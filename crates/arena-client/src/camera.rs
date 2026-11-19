//! Cameras: the shot orbit, and the third-person chase rig (low,
//! over-the-shoulder behind the turret, world collision, hold-to-zoom
//! gunsight) with trauma-based shake.

use glam::{Quat, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub fov_y: f32,
}

impl Camera {
    pub fn look(eye: Vec3, target: Vec3, fov_deg: f32) -> Self {
        Self { eye, target, fov_y: fov_deg.to_radians() }
    }

    /// A slow orbit round `centre`, `t` in seconds (title screen backdrop).
    #[allow(dead_code)]
    pub fn orbit(centre: Vec3, radius: f32, height: f32, t: f32, phase: f32) -> Self {
        let a = phase + t * 0.12;
        Self::look(centre + Vec3::new(a.cos() * radius, height, a.sin() * radius), centre + Vec3::Y * 1.6, 40.0)
    }
}

/// Something the camera can hit: returns the fraction (0..1] of the
/// segment `from -> to` that is free.
pub trait CameraCollider {
    fn free_fraction(&self, from: Vec3, to: Vec3) -> f32;
}

/// The over-the-shoulder chase rig.
#[derive(Clone, Debug)]
pub struct ChaseCam {
    /// Look direction yaw (+ = left, 0 looks along +X) and pitch (+ = up).
    pub yaw: f32,
    pub pitch: f32,
    /// Boom length behind the pivot, metres, and sideways shoulder offset.
    pub distance: f32,
    pub shoulder: f32,
    /// Pivot height above the hull origin (just behind and above the turret).
    pub pivot_height: f32,
    /// 0 = third person, 1 = gunsight. Eased towards `zoom_target`.
    pub zoom: f32,
    pub zoom_target: f32,
    /// Shake: 0..1, decays; shake amount is trauma squared.
    pub trauma: f32,
    /// Smoothed boom length after collision, so the camera does not pop.
    boom: f32,
}

impl Default for ChaseCam {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: -0.12,
            distance: 10.5,
            shoulder: 1.6,
            pivot_height: 2.6,
            zoom: 0.0,
            zoom_target: 0.0,
            trauma: 0.0,
            boom: 10.5,
        }
    }
}

impl ChaseCam {
    pub fn add_trauma(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }

    /// The direction the rig looks along.
    pub fn forward(&self) -> Vec3 {
        Quat::from_rotation_y(self.yaw) * Quat::from_rotation_z(self.pitch) * Vec3::X
    }

    /// Advances easing and shake and returns this frame's camera for a tank
    /// whose hull origin is at `pivot_base`. `time` drives the shake noise.
    pub fn update(&mut self, dt: f32, time: f32, pivot_base: Vec3, world: &dyn CameraCollider) -> Camera {
        self.zoom += (self.zoom_target - self.zoom) * (1.0 - (-dt * 12.0).exp());
        self.trauma = (self.trauma - dt * 0.9).max(0.0);
        let fwd = self.forward();
        let right = fwd.cross(Vec3::Y).normalize_or(Vec3::Z);
        let pivot = pivot_base + Vec3::Y * self.pivot_height;
        // Third person: boom back and over the right shoulder. Gunsight: at the pivot.
        let shoulder = pivot + right * self.shoulder * (1.0 - self.zoom);
        let want = self.distance * (1.0 - self.zoom) + 0.3;
        let free = world.free_fraction(shoulder, shoulder - fwd * want).clamp(0.05, 1.0);
        let reach = (want * free - 0.4).max(0.3);
        // Pull in fast, ease back out.
        let k = if reach < self.boom { 1.0 } else { 1.0 - (-dt * 3.0).exp() };
        self.boom += (reach - self.boom) * k;
        let mut eye = shoulder - fwd * self.boom;
        let mut target = shoulder + fwd * 30.0;
        // Shake: smooth pseudo-noise on position and aim, trauma squared.
        let s = self.trauma * self.trauma;
        if s > 0.0 {
            let n = |f: f32, o: f32| (time * f + o).sin() * 0.6 + (time * f * 2.3 + o * 1.7).sin() * 0.4;
            let jolt = Vec3::new(n(23.0, 0.0), n(19.0, 3.0), n(29.0, 7.0)) * 0.35 * s;
            eye += jolt;
            target += Vec3::new(n(17.0, 11.0), n(21.0, 5.0), n(13.0, 2.0)) * 1.6 * s;
        }
        let fov = 62.0 + (22.0 - 62.0) * self.zoom;
        Camera::look(eye, target, fov)
    }
}

/// No collision (lineups and tests).
#[allow(dead_code)]
pub struct OpenAir;

impl CameraCollider for OpenAir {
    fn free_fraction(&self, _from: Vec3, _to: Vec3) -> f32 {
        1.0
    }
}
