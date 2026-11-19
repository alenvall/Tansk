//! Raw input, fed by the platform (DOM events or winit) and read by the
//! game once a frame. Keys are `KeyboardEvent.code` names ("KeyW",
//! "ShiftLeft", "Tab").

use std::collections::HashSet;

#[derive(Default)]
pub struct Controls {
    held: HashSet<String>,
    /// Keys pressed since the last frame: a tap shorter than a frame counts.
    tapped: HashSet<String>,
    /// Left, right, middle.
    pub buttons: [bool; 3],
    /// Pressed since the last frame (a click shorter than a frame still counts).
    pub pressed: [bool; 3],
    /// Mouse movement since the last frame, pixels.
    pub mouse: (f32, f32),
    /// The pointer is locked to the canvas (or `mouse=free` debugging).
    pub captured: bool,
}

impl Controls {
    pub fn key_down(&mut self, code: &str) {
        self.held.insert(code.to_string());
        self.tapped.insert(code.to_string());
    }

    pub fn key_up(&mut self, code: &str) {
        self.held.remove(code);
    }

    pub fn button(&mut self, b: usize, down: bool) {
        if b < 3 {
            self.buttons[b] = down;
            self.pressed[b] |= down;
        }
    }

    pub fn mouse_move(&mut self, dx: f32, dy: f32) {
        if self.captured {
            self.mouse.0 += dx;
            self.mouse.1 += dy;
        }
    }

    /// Focus lost: let go of everything.
    pub fn release_all(&mut self) {
        self.held.clear();
        self.tapped.clear();
        self.buttons = [false; 3];
    }

    /// Down now, or tapped since the last frame.
    pub fn held(&self, code: &str) -> bool {
        self.held.contains(code) || self.tapped.contains(code)
    }

    pub fn any(&self, codes: &[&str]) -> bool {
        codes.iter().any(|c| self.held(c))
    }

    pub fn axis(&self, neg: &[&str], pos: &[&str]) -> f32 {
        (self.any(pos) as i32 - self.any(neg) as i32) as f32
    }

    pub fn end_frame(&mut self) {
        self.mouse = (0.0, 0.0);
        self.pressed = [false; 3];
        self.tapped.clear();
    }
}

/// One gamepad in the standard mapping (the browser's Gamepad API).
#[derive(Clone, Copy, Debug, Default)]
pub struct Pad {
    /// Left x, left y, right x, right y; -1..1, y down.
    pub axes: [f32; 4],
    /// Button values 0..1 (A B X Y LB RB LT RT Back Start LS RS Up Down Left Right).
    pub buttons: [f32; 16],
}

pub mod pad {
    pub const A: usize = 0;
    pub const B: usize = 1;
    pub const Y: usize = 3;
    pub const LB: usize = 4;
    pub const RB: usize = 5;
    pub const LT: usize = 6;
    pub const RT: usize = 7;
    pub const BACK: usize = 8;
    pub const START: usize = 9;
    pub const LS: usize = 10;
    pub const UP: usize = 12;
    pub const LEFT: usize = 14;
    pub const RIGHT: usize = 15;
}

impl Pad {
    pub fn down(&self, b: usize) -> bool {
        self.buttons.get(b).is_some_and(|v| *v > 0.5)
    }

    /// A stick axis with a radial-ish deadzone and the rest rescaled.
    pub fn axis(&self, i: usize) -> f32 {
        let v = self.axes[i];
        let dz = 0.15;
        if v.abs() < dz {
            0.0
        } else {
            (v.abs() - dz) / (1.0 - dz) * v.signum()
        }
    }

    pub fn active(&self) -> bool {
        self.axes.iter().any(|a| a.abs() > 0.3) || self.buttons.iter().any(|b| *b > 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tap_inside_one_frame_counts_once() {
        let mut c = Controls::default();
        c.key_down("KeyF");
        c.key_up("KeyF");
        assert!(c.held("KeyF") && c.any(&["KeyE", "KeyF"]));
        c.end_frame();
        assert!(!c.held("KeyF"));
        c.button(0, true);
        c.button(0, false);
        assert!(c.pressed[0] && !c.buttons[0]);
        c.end_frame();
        assert!(!c.pressed[0]);
    }
}
