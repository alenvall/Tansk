//! A small seeded RNG (SplitMix64). No OS entropy anywhere in the sim.

use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed ^ 0x9E37_79B9_7F4A_7C15 }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform in [lo, hi).
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }

    /// Uniform integer in [0, n).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }

    /// A random unit vector inside a cone of half-angle `angle` around `dir`.
    pub fn cone(&mut self, dir: Vec3, angle: f32) -> Vec3 {
        if angle <= 0.0 {
            return dir;
        }
        let a = self.range(0.0, std::f32::consts::TAU);
        let r = angle * self.f32().sqrt();
        let any = if dir.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
        let u = dir.cross(any).normalize();
        let v = dir.cross(u);
        (dir + (u * a.cos() + v * a.sin()) * r.tan()).normalize()
    }
}

/// Deterministic hash of a 2D lattice point, in [0, 1).
pub fn hash2(seed: u64, x: i32, z: i32) -> f32 {
    let mut h = seed
        ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h = (h ^ (h >> 31)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h = (h ^ (h >> 29)).wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 32;
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// Smooth value noise in [0, 1).
pub fn value_noise(seed: u64, x: f32, z: f32) -> f32 {
    let x0 = x.floor();
    let z0 = z.floor();
    let fx = x - x0;
    let fz = z - z0;
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sz = fz * fz * (3.0 - 2.0 * fz);
    let (ix, iz) = (x0 as i32, z0 as i32);
    let a = hash2(seed, ix, iz);
    let b = hash2(seed, ix + 1, iz);
    let c = hash2(seed, ix, iz + 1);
    let d = hash2(seed, ix + 1, iz + 1);
    let ab = a + (b - a) * sx;
    let cd = c + (d - c) * sx;
    ab + (cd - ab) * sz
}
