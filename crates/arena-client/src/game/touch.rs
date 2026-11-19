//! Touch controls for phones and tablets: a floating stick on the left
//! drives (throttle and steer, like WASD), a drag anywhere on the right
//! turns the camera, and thumb buttons fire, boost and use the special.
//! The FIRE and ALT buttons aim too: drag off them and the camera follows.
//!
//! The platform (web.rs) feeds each touch with the control it landed on
//! (the button's `data-touch`), in CSS pixels; the game reads the state
//! once a frame. Nothing here touches the DOM.

/// What a touch landed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// The left zone: the floating drive stick.
    Stick,
    /// The right zone: drag to aim.
    Look,
    Fire,
    Alt,
    Boost,
    Special,
    /// Taps (on release, if the finger stayed on the button).
    Pause,
    Board,
    Chat,
}

impl Control {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "stick" => Control::Stick,
            "look" => Control::Look,
            "fire" => Control::Fire,
            "alt" => Control::Alt,
            "boost" => Control::Boost,
            "special" => Control::Special,
            "pause" => Control::Pause,
            "board" => Control::Board,
            "chat" => Control::Chat,
            _ => return None,
        })
    }

    /// Dragging this one turns the camera.
    fn aims(self) -> bool {
        matches!(self, Control::Look | Control::Fire | Control::Alt)
    }

    fn tap(self) -> bool {
        matches!(self, Control::Pause | Control::Board | Control::Chat)
    }
}

/// A one-shot press for the game (a tap on a small button).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tap {
    Pause,
    Board,
    Chat,
}

#[derive(Clone, Copy, Debug)]
struct Finger {
    id: i32,
    control: Control,
    /// The stick's base (the stick), or where the touch started.
    origin: (f32, f32),
    last: (f32, f32),
}

/// The stick's reach, CSS pixels: a full push at this distance.
pub const STICK_RADIUS: f32 = 56.0;
const STICK_DEADZONE: f32 = 0.14;

#[derive(Default)]
pub struct Touch {
    /// The touch scheme is on (a coarse pointer, or a touch was seen).
    pub enabled: bool,
    fingers: Vec<Finger>,
    /// Aim drag since the last frame, CSS pixels.
    look: (f32, f32),
    taps: Vec<Tap>,
    /// The scoreboard button toggles the board.
    pub board: bool,
    /// The page's size, CSS pixels (aim speed is per screen height).
    pub viewport: (f32, f32),
}

impl Touch {
    /// A finger came down on `control` at `(x, y)`.
    pub fn start(&mut self, id: i32, control: Control, x: f32, y: f32) {
        self.enabled = true;
        self.fingers.retain(|f| f.id != id);
        // One stick and one aiming finger at a time; a second finger on the
        // stick zone takes it over.
        if control == Control::Stick {
            self.fingers.retain(|f| f.control != Control::Stick);
        }
        self.fingers.push(Finger { id, control, origin: (x, y), last: (x, y) });
    }

    pub fn moved(&mut self, id: i32, x: f32, y: f32) {
        let Some(f) = self.fingers.iter_mut().find(|f| f.id == id) else { return };
        if f.control.aims() {
            self.look.0 += x - f.last.0;
            self.look.1 += y - f.last.1;
        }
        if f.control == Control::Stick {
            // Past the rim the base follows the thumb (no dead push).
            let (dx, dy) = (x - f.origin.0, y - f.origin.1);
            let d = (dx * dx + dy * dy).sqrt();
            if d > STICK_RADIUS {
                let k = (d - STICK_RADIUS) / d;
                f.origin.0 += dx * k;
                f.origin.1 += dy * k;
            }
        }
        f.last = (x, y);
    }

    /// The finger lifted (`cancelled`: the browser took it, no tap).
    pub fn end(&mut self, id: i32, cancelled: bool) {
        let Some(i) = self.fingers.iter().position(|f| f.id == id) else { return };
        let f = self.fingers.remove(i);
        if cancelled || !f.control.tap() {
            return;
        }
        let tap = match f.control {
            Control::Pause => Tap::Pause,
            Control::Board => {
                self.board = !self.board;
                Tap::Board
            }
            _ => Tap::Chat,
        };
        self.taps.push(tap);
    }

    /// Lets go of everything (focus lost, the pause menu, a screen change).
    pub fn release_all(&mut self) {
        self.fingers.clear();
        self.look = (0.0, 0.0);
    }

    pub fn held(&self, c: Control) -> bool {
        self.fingers.iter().any(|f| f.control == c)
    }

    /// A finger is steering the camera (the view should not drift back).
    pub fn aiming(&self) -> bool {
        self.fingers.iter().any(|f| f.control.aims())
    }

    /// The stick as (steer, throttle), -1..1, throttle up = forward.
    pub fn stick(&self) -> (f32, f32) {
        let Some(f) = self.fingers.iter().find(|f| f.control == Control::Stick) else { return (0.0, 0.0) };
        let (dx, dy) = ((f.last.0 - f.origin.0) / STICK_RADIUS, (f.last.1 - f.origin.1) / STICK_RADIUS);
        let len = (dx * dx + dy * dy).sqrt();
        if len < STICK_DEADZONE {
            return (0.0, 0.0);
        }
        let k = ((len - STICK_DEADZONE) / (1.0 - STICK_DEADZONE)).min(1.0) / len;
        (dx * k, -dy * k)
    }

    /// Where to draw the stick: (base, knob) in CSS pixels, while held.
    pub fn stick_view(&self) -> Option<((f32, f32), (f32, f32))> {
        let f = self.fingers.iter().find(|f| f.control == Control::Stick)?;
        let (dx, dy) = (f.last.0 - f.origin.0, f.last.1 - f.origin.1);
        let d = (dx * dx + dy * dy).sqrt().max(1e-3);
        let k = d.min(STICK_RADIUS) / d;
        Some((f.origin, (dx * k, dy * k)))
    }

    /// The aim drag since the last call, CSS pixels.
    pub fn take_look(&mut self) -> (f32, f32) {
        std::mem::take(&mut self.look)
    }

    pub fn take_taps(&mut self) -> Vec<Tap> {
        std::mem::take(&mut self.taps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stick_drives_and_follows_the_thumb() {
        let mut t = Touch::default();
        t.start(1, Control::Stick, 100.0, 300.0);
        assert!(t.enabled);
        assert_eq!(t.stick(), (0.0, 0.0));
        // Straight up, full push: forward.
        t.moved(1, 100.0, 300.0 - STICK_RADIUS);
        let (s, th) = t.stick();
        assert!(s.abs() < 1e-4 && (th - 1.0).abs() < 1e-4);
        // Well past the rim: still a full push, and the base came along.
        t.moved(1, 100.0 + 3.0 * STICK_RADIUS, 300.0 - STICK_RADIUS);
        let (s, th) = t.stick();
        assert!((s * s + th * th).sqrt() <= 1.0 + 1e-4 && s > 0.5);
        let ((bx, _), _) = t.stick_view().unwrap();
        assert!(bx > 100.0);
        // A small wobble is the deadzone.
        t.end(1, false);
        t.start(2, Control::Stick, 50.0, 50.0);
        t.moved(2, 52.0, 51.0);
        assert_eq!(t.stick(), (0.0, 0.0));
    }

    #[test]
    fn fire_aims_and_taps_fire_once() {
        let mut t = Touch::default();
        t.start(3, Control::Fire, 600.0, 300.0);
        t.moved(3, 620.0, 290.0);
        assert!(t.held(Control::Fire) && t.aiming());
        assert_eq!(t.take_look(), (20.0, -10.0));
        assert_eq!(t.take_look(), (0.0, 0.0));
        t.end(3, false);
        assert!(!t.held(Control::Fire) && t.take_taps().is_empty());
        t.start(4, Control::Board, 10.0, 10.0);
        t.end(4, false);
        assert!(t.board);
        assert_eq!(t.take_taps(), vec![Tap::Board]);
        t.start(5, Control::Pause, 10.0, 10.0);
        t.end(5, true);
        assert!(t.take_taps().is_empty());
    }
}
