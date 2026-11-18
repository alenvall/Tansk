//! The 3D arena: a fixed heightmap over the 2D game's 1024x768 footprint,
//! with a hill in the middle (the king-of-the-hill zone), flat pads at the
//! four spawn points, a rising rim, and the 2D map's walls as blocks.
//! Pure functions, identical on server and client.

use tansk_sim::map::{self, MapEntity};

use crate::proto::P3;

pub const ARENA_W: f32 = 1024.0;
pub const ARENA_D: f32 = 768.0;
pub const WALL_HEIGHT: f32 = 40.0;
pub const ZONE_CENTER: (f32, f32) = (512.0, 384.0);
pub const ZONE_RADIUS: f32 = 60.0;

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn rolling(x: f32, z: f32) -> f32 {
    20.0 + 14.0 * (x * 0.0105).sin() * (z * 0.0125).cos()
        + 9.0 * ((x + z) * 0.019).sin()
        + 6.0 * (x * 0.031 - z * 0.023).cos()
}

/// Spawn pads: (x, z, rotation as in the map file).
pub fn spawn_points() -> Vec<(f32, f32, f32)> {
    map::standard()
        .into_iter()
        .filter_map(|e| match e {
            MapEntity::SpawnPoint { pos, rotation } => Some((pos.x, pos.y, rotation as f32)),
            _ => None,
        })
        .collect()
}

/// Ground height at (x, z). Outside the arena it keeps rising (the rim).
pub fn height(x: f32, z: f32) -> f32 {
    let cx = x.clamp(0.0, ARENA_W);
    let cz = z.clamp(0.0, ARENA_D);
    let mut h = rolling(cx, cz);

    // The central hill.
    let dx = cx - ZONE_CENTER.0;
    let dz = cz - ZONE_CENTER.1;
    let d2 = dx * dx + dz * dz;
    h += 48.0 * (-d2 / (2.0 * 120.0 * 120.0)).exp();
    // Its top is a flat plateau the zone sits on.
    let top = 20.0 + 48.0 * (-(ZONE_RADIUS * ZONE_RADIUS) / (2.0 * 120.0 * 120.0)).exp() + 6.0;
    let plateau = 1.0 - smoothstep(ZONE_RADIUS * 0.8, ZONE_RADIUS * 1.4, d2.sqrt());
    h = h * (1.0 - plateau) + top.max(h.min(top)) * plateau;

    // Flat spawn pads.
    for (sx, sz, _) in spawn_points() {
        let d = ((cx - sx).powi(2) + (cz - sz).powi(2)).sqrt();
        let w = 1.0 - smoothstep(45.0, 85.0, d);
        h = h * (1.0 - w) + rolling(sx, sz) * w;
    }

    // The rim: the ground climbs steeply at the edges.
    let edge = cx.min(ARENA_W - cx).min(cz).min(ARENA_D - cz);
    h += 70.0 * (1.0 - smoothstep(0.0, 36.0, edge));
    let out = (x - cx).abs().max((z - cz).abs());
    h + out * 1.5
}

/// Unit surface normal at (x, z).
pub fn normal(x: f32, z: f32) -> P3 {
    let e = 2.0;
    let hx = height(x + e, z) - height(x - e, z);
    let hz = height(x, z + e) - height(x, z - e);
    let n = [-hx, 2.0 * e, -hz];
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    [n[0] / l, n[1] / l, n[2] / l]
}

/// A wall block: axis-aligned box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wall3d {
    pub min: P3,
    pub max: P3,
}

impl Wall3d {
    pub fn contains(&self, p: P3) -> bool {
        (0..3).all(|i| p[i] >= self.min[i] && p[i] <= self.max[i])
    }
}

/// The 2D map's walls, standing on the terrain: from below the ground to
/// WALL_HEIGHT above the highest ground under them.
pub fn walls() -> Vec<Wall3d> {
    map::standard()
        .into_iter()
        .filter_map(|e| match e {
            MapEntity::Wall(r) => {
                let mut top: f32 = 0.0;
                let mut low: f32 = f32::MAX;
                for i in 0..=4 {
                    for j in 0..=4 {
                        let h = height(r.x + r.w * i as f32 / 4.0, r.y + r.h * j as f32 / 4.0);
                        top = top.max(h);
                        low = low.min(h);
                    }
                }
                Some(Wall3d { min: [r.x, low - 10.0, r.y], max: [r.x + r.w, top + WALL_HEIGHT, r.y + r.h] })
            }
            _ => None,
        })
        .collect()
}

/// The king-of-the-hill zone's centre, on the ground.
pub fn zone_center() -> P3 {
    [ZONE_CENTER.0, height(ZONE_CENTER.0, ZONE_CENTER.1), ZONE_CENTER.1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sane_heights() {
        for x in (0..=1024).step_by(16) {
            for z in (0..=768).step_by(16) {
                let h = height(x as f32, z as f32);
                assert!(h.is_finite() && h > -5.0 && h < 200.0, "{x},{z}: {h}");
            }
        }
        assert!(zone_center()[1] > height(300.0, 384.0));
        assert_eq!(walls().len(), 16);
        assert_eq!(spawn_points().len(), 4);
    }
}
