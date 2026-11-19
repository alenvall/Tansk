//! Where a sound is heard from: the pan, the level and the dullness of a
//! source relative to the listener. Pure maths, shared by the one-shots and
//! the engines, and tested natively.
//!
//! World convention: Y is up; the ground is the XZ plane. Only the ground
//! plane decides the pan; height still counts toward distance.

/// The ears: usually the player's tank (not the camera), facing where the
/// camera looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Listener {
    pub pos: [f32; 3],
    /// Need not be normalised; its Y is ignored for panning.
    pub forward: [f32; 3],
}

impl Default for Listener {
    fn default() -> Self {
        Self { pos: [0.0; 3], forward: [0.0, 0.0, -1.0] }
    }
}

/// How one source is heard.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spatial {
    /// `0..1`, from distance.
    pub gain: f32,
    /// `-1` left .. `1` right.
    pub pan: f32,
    /// Low-pass cutoff, Hz: air and distance take the top end.
    pub cutoff_hz: f32,
    pub distance: f32,
}

/// Full level inside this, metres.
pub const REF_DISTANCE: f32 = 8.0;
/// Silent beyond this, metres.
pub const MAX_DISTANCE: f32 = 260.0;
/// How quickly level falls past the reference distance.
pub const ROLLOFF: f32 = 0.9;

/// An unpositioned sound: centred, full level, undulled.
pub const CENTRE: Spatial = Spatial { gain: 1.0, pan: 0.0, cutoff_hz: 20_000.0, distance: 0.0 };

#[must_use]
pub fn spatialize(listener: &Listener, pos: [f32; 3]) -> Spatial {
    let d = [pos[0] - listener.pos[0], pos[1] - listener.pos[1], pos[2] - listener.pos[2]];
    let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if !dist.is_finite() {
        return Spatial { gain: 0.0, ..CENTRE };
    }
    // Inverse distance, then a fade to nothing at the edge of hearing.
    let inv = REF_DISTANCE / (REF_DISTANCE + ROLLOFF * (dist - REF_DISTANCE).max(0.0));
    let edge = (1.0 - (dist / MAX_DISTANCE).powi(2)).clamp(0.0, 1.0);
    let gain = inv * edge;

    // Pan from the ground-plane bearing. Right = forward × up.
    let (fx, fz) = norm2(listener.forward[0], listener.forward[2]).unwrap_or((0.0, -1.0));
    let (rx, rz) = (-fz, fx);
    let (pan, behind) = match norm2(d[0], d[2]) {
        Some((dx, dz)) => {
            let side = dx * rx + dz * rz;
            let front = dx * fx + dz * fz;
            // Close sources are less hard-panned: they are big, and we are
            // inside the tank.
            let near = (dist / 6.0).min(1.0);
            (side * 0.85 * near, (-front).max(0.0))
        }
        None => (0.0, 0.0),
    };
    // Air absorption: 20 kHz close, a few kHz far; a little duller behind.
    let far = (dist / MAX_DISTANCE).clamp(0.0, 1.0);
    let cutoff = 18_000.0 * (1.0 - far).powi(3) + 1_800.0;
    let cutoff = cutoff * (1.0 - 0.35 * behind);
    Spatial { gain, pan: pan.clamp(-1.0, 1.0), cutoff_hz: cutoff, distance: dist }
}

/// Equal-power gains for `pan` (`-1..1`): `(left, right)`.
#[must_use]
pub fn pan_gains(pan: f32) -> (f32, f32) {
    let a = (pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
    (a.cos(), a.sin())
}

fn norm2(x: f32, z: f32) -> Option<(f32, f32)> {
    let l = (x * x + z * z).sqrt();
    (l > 1e-4 && l.is_finite()).then(|| (x / l, z / l))
}
