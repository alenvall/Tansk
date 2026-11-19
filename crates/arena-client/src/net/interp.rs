//! Snapshot interpolation: other tanks, projectiles and props are drawn
//! [`DELAY`] behind the newest server state, blended between the two
//! snapshots around that time.

use std::collections::VecDeque;

use arena_core::snapshot::{dq_rot, q_rot, ProjectileSnap, PropSnap, Snapshot, TankSnap};
use arena_core::{Quat, Vec3, TICK_HZ};

/// How far behind the newest snapshot others are drawn, seconds (three
/// snapshots at 30 Hz: one lost or late snapshot doesn't stall anything).
pub const DELAY: f64 = 0.1;
/// Beyond the newest snapshot, tanks coast on their velocity for at most this long.
const MAX_EXTRAPOLATE: f64 = 0.1;

#[derive(Default)]
pub struct Interp {
    buf: VecDeque<Snapshot>,
    /// Server time minus local time, seconds (smoothed).
    offset: Option<f64>,
}

fn ticks_to_s(t: u32) -> f64 {
    t as f64 / TICK_HZ as f64
}

impl Interp {
    pub fn clear(&mut self) {
        self.buf.clear();
        self.offset = None;
    }

    pub fn push(&mut self, snap: Snapshot, now: f64) {
        if self.buf.back().is_some_and(|b| b.tick >= snap.tick) {
            // A match restart or a new room restarts the clock.
            if self.buf.back().is_some_and(|b| b.tick > snap.tick + 600) {
                self.clear();
            } else {
                return;
            }
        }
        let sample = ticks_to_s(snap.tick) - now;
        self.offset = Some(match self.offset {
            None => sample,
            Some(o) if (sample - o).abs() > 0.25 => sample,
            // Late packets pull the clock back slowly, early ones quickly.
            Some(o) if sample > o => o + (sample - o) * 0.2,
            Some(o) => o + (sample - o) * 0.02,
        });
        self.buf.push_back(snap);
        while self.buf.len() > 32 {
            self.buf.pop_front();
        }
    }

    pub fn latest(&self) -> Option<&Snapshot> {
        self.buf.back()
    }

    /// The server tick being drawn now (fractional).
    pub fn render_tick(&self, now: f64) -> Option<f64> {
        Some((now + self.offset? - DELAY) * TICK_HZ as f64)
    }

    /// The world at `now - DELAY` in server time.
    pub fn sample(&self, now: f64) -> Option<Snapshot> {
        let newest = self.buf.back()?;
        let rt = self.render_tick(now)?;
        let i = self.buf.iter().position(|s| s.tick as f64 > rt);
        Some(match i {
            Some(0) => self.buf[0].clone(),
            Some(i) => {
                let (a, b) = (&self.buf[i - 1], &self.buf[i]);
                let t = ((rt - a.tick as f64) / (b.tick - a.tick) as f64) as f32;
                lerp_snapshot(a, b, t)
            }
            None => {
                let ahead = ((rt - newest.tick as f64) / TICK_HZ as f64).min(MAX_EXTRAPOLATE) as f32;
                extrapolate(newest, ahead)
            }
        })
    }
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    Vec3::from(a).lerp(Vec3::from(b), t).to_array()
}

fn lerp_rot(a: [i16; 4], b: [i16; 4], t: f32) -> [i16; 4] {
    q_rot(dq_rot(a).slerp(dq_rot(b), t))
}

fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    let d = (b - a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    a + d * t
}

pub fn lerp_tank(a: &TankSnap, b: &TankSnap, t: f32) -> TankSnap {
    let mut out = b.clone();
    // A respawn is a jump, not a slide.
    if a.alive != b.alive || Vec3::from(a.pos).distance(Vec3::from(b.pos)) > 15.0 {
        return out;
    }
    out.pos = lerp3(a.pos, b.pos, t);
    out.rot = lerp_rot(a.rot, b.rot, t);
    out.linvel = lerp3(a.linvel, b.linvel, t);
    out.angvel = lerp3(a.angvel, b.angvel, t);
    out.turret_yaw = lerp_angle(a.turret_yaw, b.turret_yaw, t);
    out.gun_pitch = a.gun_pitch + (b.gun_pitch - a.gun_pitch) * t;
    out
}

fn lerp_projectile(a: &ProjectileSnap, b: &ProjectileSnap, t: f32) -> ProjectileSnap {
    let mut out = b.clone();
    out.pos = lerp3(a.pos, b.pos, t);
    out.vel = lerp3(a.vel, b.vel, t);
    out
}

fn lerp_prop(a: &PropSnap, b: &PropSnap, t: f32) -> PropSnap {
    let mut out = b.clone();
    out.pos = lerp3(a.pos, b.pos, t);
    out.rot = lerp_rot(a.rot, b.rot, t);
    out
}

/// `b` with tanks, projectiles and props blended from `a` by `t` (0 = a, 1 = b).
/// Everything else (pickups, scores, mode, ...) is `b`'s.
pub fn lerp_snapshot(a: &Snapshot, b: &Snapshot, t: f32) -> Snapshot {
    let t = t.clamp(0.0, 1.0);
    let mut out = b.clone();
    let dt = (b.tick.saturating_sub(a.tick)) as f32 / TICK_HZ as f32;
    out.tanks = b
        .tanks
        .iter()
        .map(|tb| match a.tank(tb.player) {
            Some(ta) => lerp_tank(ta, tb, t),
            None => tb.clone(),
        })
        .collect();
    out.projectiles = b
        .projectiles
        .iter()
        .map(|pb| match a.projectiles.iter().find(|p| p.id == pb.id) {
            Some(pa) => lerp_projectile(pa, pb, t),
            // New since `a`: back it up along its velocity.
            None => {
                let mut p = pb.clone();
                p.pos = (Vec3::from(pb.pos) - Vec3::from(pb.vel) * (1.0 - t) * dt).to_array();
                p
            }
        })
        .collect();
    out.props = b
        .props
        .iter()
        .map(|pb| match a.props.iter().find(|p| p.id == pb.id) {
            Some(pa) => lerp_prop(pa, pb, t),
            None => pb.clone(),
        })
        .collect();
    // The storm wall slides smoothly between snapshots.
    if let (Some(ca), Some(cb)) = (&a.storm_circle, &mut out.storm_circle) {
        cb.radius = ca.radius + (cb.radius - ca.radius) * t;
        for k in 0..2 {
            cb.center[k] = ca.center[k] + (cb.center[k] - ca.center[k]) * t;
        }
    }
    out
}

/// `s` moved `ahead` seconds along tank and projectile velocities.
fn extrapolate(s: &Snapshot, ahead: f32) -> Snapshot {
    let mut out = s.clone();
    if ahead <= 0.0 {
        return out;
    }
    for t in &mut out.tanks {
        if t.alive {
            t.pos = (Vec3::from(t.pos) + Vec3::from(t.linvel) * ahead).to_array();
            let w = Vec3::from(t.angvel);
            if w.length_squared() > 1e-6 {
                let r = Quat::from_scaled_axis(w * ahead) * dq_rot(t.rot);
                t.rot = q_rot(r);
            }
        }
    }
    for p in &mut out.projectiles {
        p.pos = (Vec3::from(p.pos) + Vec3::from(p.vel) * ahead).to_array();
    }
    out
}
