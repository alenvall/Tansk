//! A cached sampling of `terrain::height` (which re-parses the map on every
//! call): a 4 px grid over the arena, bilinearly interpolated. Built once per
//! process.

use std::sync::OnceLock;

use crate::proto::P3;
use crate::terrain::{self, ARENA_D, ARENA_W};

const STEP: f32 = 4.0;
const NX: usize = (ARENA_W / STEP) as usize + 1;
const NZ: usize = (ARENA_D / STEP) as usize + 1;

fn grid() -> &'static [f32] {
    static GRID: OnceLock<Vec<f32>> = OnceLock::new();
    GRID.get_or_init(|| {
        let mut g = Vec::with_capacity(NX * NZ);
        for j in 0..NZ {
            for i in 0..NX {
                g.push(terrain::height(i as f32 * STEP, j as f32 * STEP));
            }
        }
        g
    })
}

/// Ground height at (x, z), matching `terrain::height` to within a fraction of a pixel.
pub fn height(x: f32, z: f32) -> f32 {
    let cx = x.clamp(0.0, ARENA_W);
    let cz = z.clamp(0.0, ARENA_D);
    let g = grid();
    let fx = cx / STEP;
    let fz = cz / STEP;
    let i = (fx as usize).min(NX - 2);
    let j = (fz as usize).min(NZ - 2);
    let tx = fx - i as f32;
    let tz = fz - j as f32;
    let h00 = g[j * NX + i];
    let h10 = g[j * NX + i + 1];
    let h01 = g[(j + 1) * NX + i];
    let h11 = g[(j + 1) * NX + i + 1];
    let h = (h00 * (1.0 - tx) + h10 * tx) * (1.0 - tz) + (h01 * (1.0 - tx) + h11 * tx) * tz;
    let out = (x - cx).abs().max((z - cz).abs());
    h + out * 1.5
}

/// Unit surface normal at (x, z).
pub fn normal(x: f32, z: f32) -> P3 {
    let e = 3.0;
    let hx = height(x + e, z) - height(x - e, z);
    let hz = height(x, z + e) - height(x, z - e);
    let n = [-hx, 2.0 * e, -hz];
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    [n[0] / l, n[1] / l, n[2] / l]
}
