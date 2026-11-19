//! Self-running views used until the game loop is wired: a Practice match
//! of bots on The Sink with a spectating chase camera (live on the web and
//! natively, and the `--shot gameplay` / `showcase` compositions), and the
//! `lineup` of every prop and tank part in the same light.

use arena_core::map::Map;
use arena_core::{PartKind, PlayerId, Sim, DT};
use glam::{Mat4, Quat, Vec3};

use crate::camera::{Camera, CameraCollider, ChaseCam};
use crate::meshkit::MeshBuilder;
use crate::parts;
use crate::props;
use crate::renderer::Instance;
use crate::tank;
use crate::terrain;
use crate::world::{paint_for, RenderWorld, TankView, WorldRenderer};

/// Camera collision against the map's terrain and obstacle boxes.
pub struct MapCollider<'a>(pub &'a Map);

impl CameraCollider for MapCollider<'_> {
    fn free_fraction(&self, from: Vec3, to: Vec3) -> f32 {
        let map = self.0;
        let d = to - from;
        let steps = 24;
        let mut best: f32 = 1.0;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let p = from + d * t;
            if p.y < map.height_at(p.x, p.z) + 0.6 {
                best = best.min((t - 1.0 / steps as f32).max(0.0));
                break;
            }
        }
        let len = d.length().max(1e-4);
        let dir = d / len;
        for o in &map.obstacles {
            if o.cover == arena_core::Cover::Flat || o.pos.distance(from) > o.radius + len + 2.0 {
                continue;
            }
            for (c, rot, half) in o.world_boxes() {
                let inv = rot.inverse();
                let ro = inv * (from - c);
                let rd = inv * dir;
                let half = half + Vec3::splat(0.4);
                let (mut t0, mut t1) = (0.0f32, len);
                let mut hit = true;
                for k in 0..3 {
                    if rd[k].abs() < 1e-6 {
                        if ro[k].abs() > half[k] {
                            hit = false;
                            break;
                        }
                    } else {
                        let a = (-half[k] - ro[k]) / rd[k];
                        let b = (half[k] - ro[k]) / rd[k];
                        t0 = t0.max(a.min(b));
                        t1 = t1.min(a.max(b));
                        if t0 > t1 {
                            hit = false;
                            break;
                        }
                    }
                }
                if hit && t0 > 0.0 {
                    best = best.min(t0 / len);
                }
            }
        }
        best
    }
}

/// A Practice match playing itself, watched by a chase camera that follows
/// a tank in the thick of it.
pub struct Demo {
    pub sim: Sim,
    pub map: Map,
    pub me: PlayerId,
    pub chase: ChaseCam,
    pub follow: Option<PlayerId>,
    pub time: f32,
    accumulator: f32,
    switch_in: f32,
    pub world: RenderWorld,
}

impl Demo {
    pub fn new(seed: u64, bots: usize) -> Self {
        let (sim, me) = Sim::practice(seed, bots, 0.7);
        Self {
            sim,
            map: Map::the_sink(seed),
            me,
            chase: ChaseCam::default(),
            follow: None,
            time: 0.0,
            accumulator: 0.0,
            switch_in: 0.0,
            world: RenderWorld::default(),
        }
    }

    /// The bot with the most company: the nearest pair of live tanks.
    fn busiest(&self) -> Option<PlayerId> {
        let tanks: Vec<&TankView> = self.world.tanks.iter().filter(|t| t.alive && t.player != self.me).collect();
        let mut best = None;
        let mut best_d = f32::MAX;
        for a in &tanks {
            for b in &tanks {
                if a.player != b.player {
                    let d = a.pos.distance(b.pos);
                    if d < best_d {
                        best_d = d;
                        best = Some(a.player);
                    }
                }
            }
        }
        best.or_else(|| tanks.first().map(|t| t.player))
    }

    /// Steps the match by `dt` seconds of wall time, feeds events to the
    /// effects, and updates the camera. Returns the world to draw.
    pub fn advance(&mut self, dt: f32, wr: &mut WorldRenderer) -> &RenderWorld {
        let dt = dt.min(0.1);
        self.time += dt;
        self.accumulator += dt;
        let listener = self.world.camera.map(|c| c.eye).unwrap_or(Vec3::ZERO);
        while self.accumulator >= DT {
            self.sim.step();
            for e in self.sim.drain_events() {
                let trauma = wr.event(&e, listener);
                self.chase.add_trauma(trauma);
                if let arena_core::Event::UpgradeApplied { player, .. } = e {
                    if let Some(t) = self.world.tanks.iter().find(|t| t.player == player) {
                        wr.fx.weld(t.pos + Vec3::Y * 1.5);
                    }
                }
            }
            self.accumulator -= DT;
        }
        let snap = self.sim.snapshot();
        let mut world = RenderWorld::from_snapshot(&snap, &|p| paint_for(p));
        world.time = self.time;
        world.dt = dt;
        self.world = world;

        self.switch_in -= dt;
        let dead = self.follow.and_then(|f| self.world.tanks.iter().find(|t| t.player == f)).is_none_or(|t| !t.alive);
        if self.follow.is_none() || dead || self.switch_in <= 0.0 {
            self.follow = self.busiest();
            self.switch_in = 18.0;
        }
        if let Some(t) = self.follow.and_then(|f| self.world.tanks.iter().find(|t| t.player == f)) {
            // Look where the turret looks, a little down.
            let fwd = t.turret_transform().transform_vector3(Vec3::X);
            let yaw = (-fwd.z).atan2(fwd.x);
            let mut dy = yaw - self.chase.yaw;
            while dy > std::f32::consts::PI {
                dy -= std::f32::consts::TAU;
            }
            while dy < -std::f32::consts::PI {
                dy += std::f32::consts::TAU;
            }
            self.chase.yaw += dy * (1.0 - (-dt * 3.0).exp());
            let pos = t.pos;
            let cam = self.chase.update(dt, self.time, pos, &MapCollider(&self.map));
            self.world.camera = Some(cam);
        }
        &self.world
    }

    /// Runs `seconds` of the match quickly (effects aged with it).
    pub fn run(&mut self, seconds: f32, wr: &mut WorldRenderer) {
        let step = 1.0 / 30.0;
        let mut t = 0.0;
        while t < seconds {
            self.advance(step, wr);
            // Ages effects too, which `draw` normally does.
            let w = self.world.clone();
            wr.fx.update(step, &w);
            t += step;
        }
    }
}

/// The lineup: every prop and every tank part, in rows on flat sand, in
/// the same light. Returns the world (tanks with parts) and a camera.
pub fn lineup(wr: &mut WorldRenderer) -> RenderWorld {
    let r = &mut wr.renderer;
    let mut statics: Vec<Instance> = Vec::new();
    let floor = r.add_mesh(&terrain::flat(80.0));
    statics.push(Instance::new(floor, Mat4::IDENTITY));
    let mut place = |m: MeshBuilder, x: f32, z: f32, yaw: f32| {
        let id = r.add_mesh(&m);
        statics.push(Instance::new(id, Mat4::from_rotation_translation(Quat::from_rotation_y(yaw), Vec3::new(x, 0.0, z))));
    };
    // Row 1 (back): the big pieces.
    let back = -22.0;
    place(props::container(Vec3::new(6.1, 1.3, 1.22), props::CONTAINER_COLOURS[0]).lifted(1.3), -30.0, back, 0.0);
    place(props::container(Vec3::new(3.03, 1.3, 1.22), props::CONTAINER_COLOURS[1]).lifted(1.3), -19.0, back, 0.2);
    place(props::container(Vec3::new(3.03, 1.3, 1.22), props::CONTAINER_COLOURS[2]).lifted(1.3), -19.0, back + 0.2, 0.2);
    place(props::silo(3.0, 4.5).lifted(4.5), -9.0, back - 2.0, 0.0);
    place(props::fuel_tank(), 0.0, back, 0.3);
    place(props::mesa_block(Vec3::new(7.0, 4.0, 5.0), 11).lifted(4.0), 13.0, back - 4.0, 0.0);
    place(props::rock_block(Vec3::new(3.0, 3.0, 2.6), 5).lifted(3.0), 25.0, back, 0.4);
    place(props::car_wreck(Vec3::new(2.2, 0.5, 0.95), 2).lifted(0.5), 33.0, back + 4.0, 0.5);
    // Row 2: barriers, sandbags, pipe, pier, highway.
    let mid = -10.0;
    place(props::barrier(Vec3::new(1.7, 0.5, 0.35), false).lifted(0.5), -32.0, mid, 0.0);
    place(props::barrier(Vec3::new(1.7, 0.5, 0.35), true).lifted(0.5), -27.5, mid, 0.0);
    place(props::sandbags(Vec3::new(1.5, 0.45, 0.55), 3).lifted(0.45), -22.0, mid, 0.0);
    place(props::pipe(Vec3::new(4.0, 0.55, 0.55)).lifted(0.55), -14.0, mid, 0.2);
    place(props::pier_ruin(Vec3::new(3.4, 0.45, 2.2), 4).lifted(0.45), -4.0, mid, 0.0);
    place(props::highway(Vec3::new(8.05, 0.3, 6.5)).lifted(0.3), 10.0, mid, 0.0);
    place(props::car_wreck(Vec3::new(2.2, 0.5, 0.95), 5).lifted(0.5), 24.0, mid, -0.3);
    place(props::power_pole(), 31.0, mid, 0.0);
    // Row 3: small dressing.
    let front = 0.0;
    let small: Vec<MeshBuilder> = vec![
        props::hedgehog(),
        props::razor_wire(3.0),
        props::tyres(1, 1),
        props::tyres(3, 2),
        props::crates(1),
        props::oil_drum(0x34587e, 0.6).lifted(props::DRUM_HALF),
        props::oil_drum(0x8c2a1e, 0.6).lifted(props::DRUM_HALF),
        props::cactus(1),
        props::cactus(2),
        props::boulder(3, 0.9),
        props::boulder(8, 0.5),
        props::pickup(0),
        props::pickup(1),
        props::pickup(2),
        props::pickup(3),
        props::pickup(4),
    ];
    for (i, m) in small.into_iter().enumerate() {
        place(m, -33.0 + i as f32 * 4.2, front, 0.4);
    }
    wr.set_statics(statics);

    // Tanks: base (orange), then one per non-gun part, guns on teal tanks.
    let mut world = RenderWorld::default();
    let make = |i: usize, x: f32, z: f32, colour: u32, gun: PartKind, parts: Vec<PartKind>| TankView {
        class: Default::default(),
        player: i as PlayerId,
        colour,
        pos: Vec3::new(x, tank::HULL_Y - 0.15 + 0.15, z),
        rot: Quat::from_rotation_y(0.5),
        turret_yaw: 0.2,
        gun_pitch: 0.03,
        wheels: [0.15; 10],
        track_speed: [0.0; 2],
        gun,
        parts,
        alive: true,
        boosting: false,
        burning: false,
        flaming: false,
        throttle: 0.0,
        speed: 0.0,
    };
    let guns = [PartKind::StubGun, PartKind::HeavyCannon, PartKind::BigBore, PartKind::TwinBarrel, PartKind::LongGun];
    for (i, g) in guns.iter().enumerate() {
        world.tanks.push(make(i, -32.0 + i as f32 * 10.0, 13.0, tank::DUST_SAINT, *g, vec![]));
    }
    let others: Vec<PartKind> = parts::ALL_PARTS.iter().copied().filter(|k| !parts::is_gun(*k)).collect();
    for (i, p) in others.iter().enumerate() {
        world.tanks.push(make(10 + i, -36.0 + i as f32 * 8.0, 27.0, tank::RUSTBACK, PartKind::HeavyCannon, vec![*p]));
    }
    world.camera = Some(Camera::look(Vec3::new(0.0, 22.0, 52.0), Vec3::new(0.0, 0.0, 2.0), 50.0));
    world
}

trait Lifted {
    fn lifted(self, dy: f32) -> Self;
}

impl Lifted for MeshBuilder {
    fn lifted(mut self, dy: f32) -> Self {
        self.transform_from(0, Mat4::from_translation(Vec3::Y * dy));
        self
    }
}
