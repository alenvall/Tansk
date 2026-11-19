//! Camera feel on top of the chase rig, all client-side and visual only:
//! recoil kick, boost FOV kick, landing dip, a sway that trails the turret
//! (so its traverse reads as heavy), and hit-stop on your kills.

use arena_core::GunKind;
use glam::Vec3;

use crate::camera::Camera;

#[derive(Default)]
pub struct Juice {
    /// Recoil pitch, radians (decays).
    recoil: f32,
    /// Extra FOV, degrees (eased toward the boost target).
    fov: f32,
    /// Landing dip, metres (decays).
    dip: f32,
    prev_vy: f32,
    /// Sideways shift trailing the turret, metres (eased).
    sway: f32,
    /// Seconds of hit-stop left.
    hitstop: f32,
}

impl Juice {
    /// Your main gun went off: kicks the view; returns the muzzle-blast
    /// shake (trauma) for the gun's size.
    pub fn fire(&mut self, gun: GunKind) -> f32 {
        let (kick, shake) = match gun {
            GunKind::Stub => (0.018, 0.16),
            GunKind::HeavyCannon => (0.03, 0.24),
            GunKind::TwinBarrel => (0.04, 0.32),
            GunKind::LongGun => (0.026, 0.22),
            GunKind::BigBore => (0.055, 0.42),
            #[allow(unreachable_patterns)]
            _ => (0.03, 0.3),
        };
        self.recoil = (self.recoil + kick).min(0.12);
        shake
    }

    /// The machine gun and friends: a tremble, no real kick.
    pub fn small_fire(&mut self) {
        self.recoil = (self.recoil + 0.004).min(0.12);
    }

    /// Your kill landed: a beat of slow motion.
    pub fn kill(&mut self) {
        self.hitstop = 0.055;
    }

    /// Scale for this frame's effect time (hit-stop slows the world's look,
    /// never the sim).
    pub fn time_scale(&mut self, dt: f32) -> f32 {
        if self.hitstop > 0.0 {
            self.hitstop -= dt;
            0.08
        } else {
            1.0
        }
    }

    /// Feed your tank's vertical speed each frame; a hard landing (launch
    /// plates, dune jumps) dips the camera.
    pub fn track_landing(&mut self, vy: f32) {
        if self.prev_vy < -5.0 && vy > -1.5 {
            self.dip = (-self.prev_vy * 0.06).min(0.8);
        }
        self.prev_vy = vy;
    }

    /// Applies everything to the rig's camera. `turret_lag` is how far the
    /// turret is behind the camera's yaw (radians, + = turret to the left).
    pub fn apply(&mut self, mut cam: Camera, dt: f32, boosting: bool, turret_lag: f32, zoom: f32) -> Camera {
        let ease = |k: f32| 1.0 - (-dt * k).exp();
        self.recoil -= self.recoil * ease(9.0);
        self.dip -= self.dip * ease(5.0);
        let fov_want = if boosting { 7.0 } else { 0.0 };
        self.fov += (fov_want - self.fov) * ease(if boosting { 5.0 } else { 3.0 });
        let sway_want = (-turret_lag * 1.3).clamp(-0.9, 0.9) * (1.0 - zoom);
        self.sway += (sway_want - self.sway) * ease(4.0);

        let fwd = (cam.target - cam.eye).normalize_or(Vec3::X);
        let right = fwd.cross(Vec3::Y).normalize_or(Vec3::Z);
        let dist = cam.target.distance(cam.eye);
        cam.eye += right * self.sway - fwd * self.recoil * 3.0 - Vec3::Y * self.dip;
        cam.target += right * self.sway + Vec3::Y * (self.recoil.tan() * dist - self.dip * 0.4);
        cam.fov_y += self.fov.to_radians() * (1.0 - zoom);
        cam
    }
}
