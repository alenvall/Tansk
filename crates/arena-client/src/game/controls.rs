//! Raw input, fed by the platform (DOM events or winit) and read by the
//! game once a frame. Keys are `KeyboardEvent.code` names ("KeyW",
//! "ShiftLeft", "Tab").

use std::collections::HashSet;

#[derive(Default)]
pub struct Controls {
    held: HashSet<String>,
    /// Left, right, middle.
    pub buttons: [bool; 3],
    /// Mouse movement since the last frame, pixels.
    pub mouse: (f32, f32),
    /// The pointer is locked to the canvas (or `mouse=free` debugging).
    pub captured: bool,
}

impl Controls {
    pub fn key_down(&mut self, code: &str) {
        self.held.insert(code.to_string());
    }

    pub fn key_up(&mut self, code: &str) {
        self.held.remove(code);
    }

    pub fn button(&mut self, b: usize, down: bool) {
        if b < 3 {
            self.buttons[b] = down;
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
        self.buttons = [false; 3];
    }

    pub fn held(&self, code: &str) -> bool {
        self.held.contains(code)
    }

    pub fn any(&self, codes: &[&str]) -> bool {
        codes.iter().any(|c| self.held(c))
    }

    pub fn axis(&self, neg: &[&str], pos: &[&str]) -> f32 {
        (self.any(pos) as i32 - self.any(neg) as i32) as f32
    }

    pub fn end_frame(&mut self) {
        self.mouse = (0.0, 0.0);
    }
}
