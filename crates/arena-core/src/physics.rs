//! The rapier world: the terrain heightfield, obstacle colliders, invisible
//! walls, and helpers. Cloneable (for client prediction / rollback): every
//! piece of rapier state is cloned except the pipeline scratch, which is
//! rebuilt.

use glam::{Quat, Vec3};
use rapier3d::prelude::*;

use crate::map::{Map, ObstacleKind};
use crate::DT;

/// Terrain and obstacles.
pub const G_STATIC: Group = Group::GROUP_1;
pub const G_TANK: Group = Group::GROUP_2;
pub const G_PROP: Group = Group::GROUP_3;
pub const G_DEBRIS: Group = Group::GROUP_4;

pub fn static_groups() -> InteractionGroups {
    InteractionGroups::new(G_STATIC, Group::ALL, InteractionTestMode::And)
}
pub fn tank_groups() -> InteractionGroups {
    InteractionGroups::new(G_TANK, G_STATIC | G_TANK | G_PROP, InteractionTestMode::And)
}
pub fn prop_groups() -> InteractionGroups {
    InteractionGroups::new(G_PROP, Group::ALL, InteractionTestMode::And)
}
pub fn debris_groups() -> InteractionGroups {
    InteractionGroups::new(G_DEBRIS, G_STATIC | G_PROP | G_DEBRIS, InteractionTestMode::And)
}
/// Query groups: what suspension rays stand on.
pub fn ground_query() -> InteractionGroups {
    InteractionGroups::new(Group::ALL, G_STATIC | G_PROP | G_TANK, InteractionTestMode::And)
}
/// Query groups: static world only (sight lines, splash occlusion).
pub fn static_query() -> InteractionGroups {
    InteractionGroups::new(Group::ALL, G_STATIC, InteractionTestMode::And)
}
/// Query groups: what shells and bullets hit in rapier (tanks use hit boxes).
pub fn shot_query() -> InteractionGroups {
    InteractionGroups::new(Group::ALL, G_STATIC | G_PROP, InteractionTestMode::And)
}

pub struct Physics {
    pub world: PhysicsWorld,
    /// Collider of each box of each map obstacle (`Map::obstacles[i].boxes[j]`).
    /// Their `user_data` is `OBSTACLE_TAG | i << 8 | j`.
    pub obstacle_colliders: Vec<Vec<ColliderHandle>>,
}

pub const OBSTACLE_TAG: u128 = 1 << 64;

/// (obstacle index, box index) if a collider's user data marks a map obstacle box.
pub fn obstacle_of(user_data: u128) -> Option<(usize, usize)> {
    if user_data & OBSTACLE_TAG != 0 {
        Some((((user_data >> 8) & 0xFFFF) as usize, (user_data & 0xFF) as usize))
    } else {
        None
    }
}

impl Clone for Physics {
    fn clone(&self) -> Self {
        let w = &self.world;
        Physics {
            world: PhysicsWorld {
                gravity: w.gravity,
                integration_parameters: w.integration_parameters,
                physics_pipeline: PhysicsPipeline::new(),
                islands: w.islands.clone(),
                broad_phase: w.broad_phase.clone(),
                narrow_phase: w.narrow_phase.clone(),
                bodies: w.bodies.clone(),
                colliders: w.colliders.clone(),
                impulse_joints: w.impulse_joints.clone(),
                multibody_joints: w.multibody_joints.clone(),
                ccd_solver: CCDSolver::new(),
            },
            obstacle_colliders: self.obstacle_colliders.clone(),
        }
    }
}

pub fn pose(pos: Vec3, rot: Quat) -> Pose {
    Pose::from_parts(pos, rot)
}

impl Physics {
    pub fn new(map: &Map) -> Physics {
        let mut world = PhysicsWorld::new();
        world.gravity = Vector::new(0.0, -9.81, 0.0);
        world.integration_parameters.dt = DT;
        let n = map.resolution;
        let heights = Array2::from_fn(n, n, |i, j| map.heights[i * n + j]);
        world.insert_collider(
            ColliderBuilder::heightfield_with_flags(
                heights,
                Vector::new(map.size, 1.0, map.size),
                HeightFieldFlags::FIX_INTERNAL_EDGES,
            )
            .friction(0.8)
            .collision_groups(static_groups()),
            None,
        );
        let mut obstacle_colliders = Vec::new();
        for (oi, o) in map.obstacles.iter().enumerate() {
            let mut handles = Vec::new();
            for (bi, (c, r, h)) in o.world_boxes().enumerate() {
                let builder = if o.kind == ObstacleKind::Silo {
                    ColliderBuilder::cylinder(h.y, h.x)
                } else {
                    ColliderBuilder::cuboid(h.x, h.y, h.z)
                };
                let handle = world.insert_collider(
                    builder
                        .position(pose(c, r))
                        .friction(0.6)
                        .user_data(OBSTACLE_TAG | (oi as u128) << 8 | bi as u128)
                        .collision_groups(static_groups()),
                    None,
                );
                handles.push(handle);
            }
            obstacle_colliders.push(handles);
        }
        // Invisible walls.
        let ph = map.play_half;
        for (c, h) in [
            (Vec3::new(ph + 2.0, 20.0, 0.0), Vec3::new(2.0, 40.0, ph + 4.0)),
            (Vec3::new(-ph - 2.0, 20.0, 0.0), Vec3::new(2.0, 40.0, ph + 4.0)),
            (Vec3::new(0.0, 20.0, ph + 2.0), Vec3::new(ph + 4.0, 40.0, 2.0)),
            (Vec3::new(0.0, 20.0, -ph - 2.0), Vec3::new(ph + 4.0, 40.0, 2.0)),
        ] {
            world.insert_collider(
                ColliderBuilder::cuboid(h.x, h.y, h.z).translation(c).collision_groups(static_groups()),
                None,
            );
        }
        let mut p = Physics { world, obstacle_colliders };
        // Build the broad phase so queries work before the first step.
        p.world.step();
        p
    }

    pub fn step(&mut self) {
        self.world.step();
    }

    /// First hit along a ray: (collider, distance, normal).
    pub fn ray(
        &self,
        origin: Vec3,
        dir: Vec3,
        max: f32,
        groups: InteractionGroups,
        exclude: Option<RigidBodyHandle>,
    ) -> Option<(ColliderHandle, f32, Vec3)> {
        let mut filter = QueryFilter::new().groups(groups);
        if let Some(b) = exclude {
            filter = filter.exclude_rigid_body(b);
        }
        let ray = Ray::new(origin, dir);
        self.world
            .query_pipeline_with_filter(filter)
            .cast_ray_and_get_normal(&ray, max, true)
            .map(|(h, hit)| (h, hit.time_of_impact, hit.normal))
    }

    /// Is the segment a→b clear of static geometry?
    pub fn clear_line(&self, a: Vec3, b: Vec3) -> bool {
        let d = b - a;
        let len = d.length();
        if len < 1e-3 {
            return true;
        }
        self.ray(a, d / len, len, static_query(), None).is_none()
    }

    pub fn body(&self, h: RigidBodyHandle) -> &RigidBody {
        &self.world.bodies[h]
    }

    pub fn body_mut(&mut self, h: RigidBodyHandle) -> &mut RigidBody {
        &mut self.world.bodies[h]
    }
}
