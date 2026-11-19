//! "Play of the match": a rolling record of the last minute as drawn (tanks,
//! shells, the events that make the sparks), the best moment picked from
//! it at the whistle, and its replay under a slow orbit camera before the
//! podium.
//!
//! Memory stays small: ten frames a second of compact tank poses, part
//! lists shared between frames until they change, and only the events
//! that draw something.

use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use arena_core::{ChassisClass, Event, PartKind, PlayerId, VehicleKind};
use glam::{Quat, Vec3};

use crate::camera::{Camera, CameraCollider};
use crate::world::{RenderWorld, ShellView, TankView};

/// Seconds kept.
pub const KEEP_SECS: f32 = 60.0;
/// Frames a second recorded.
const RATE: f32 = 10.0;
/// Kills this close together count as one multi-kill, s.
const MULTI_WINDOW: f32 = 8.0;
/// A damage burst is summed over this long, s.
const BURST_WINDOW: f32 = 4.0;
/// The replay's length bounds, s (of match time).
const MIN_LEN: f32 = 4.0;
const MAX_LEN: f32 = 6.0;
/// Replay speed: a touch slower than life, not a slow-mo gimmick.
pub const SPEED: f32 = 0.85;

#[derive(Clone)]
struct RTank {
    player: PlayerId,
    colour: u32,
    pos: Vec3,
    rot: Quat,
    turret_yaw: f32,
    gun_pitch: f32,
    gun: PartKind,
    parts: Rc<Vec<PartKind>>,
    alive: bool,
    boosting: bool,
    burning: bool,
    class: ChassisClass,
    speed: f32,
    throttle: f32,
    health: u8,
}

#[derive(Clone)]
struct Frame {
    t: f32,
    tanks: Vec<RTank>,
    shells: Vec<ShellView>,
    vehicles: Vec<(PlayerId, VehicleKind, Vec<f32>)>,
}

/// The rolling record.
#[derive(Default)]
pub struct Recorder {
    frames: VecDeque<Frame>,
    events: VecDeque<(f32, Event)>,
    parts: HashMap<PlayerId, Rc<Vec<PartKind>>>,
}

/// Events worth replaying (they draw something).
fn keep(e: &Event) -> bool {
    matches!(
        e,
        Event::Fired { .. }
            | Event::Tracer { .. }
            | Event::Hit { .. }
            | Event::Impact { .. }
            | Event::Explosion { .. }
            | Event::TeslaArc { .. }
            | Event::PartBrokeOff { .. }
            | Event::TrackBlown { .. }
            | Event::Destroyed { .. }
    )
}

impl Recorder {
    pub fn clear(&mut self) {
        self.frames.clear();
        self.events.clear();
        self.parts.clear();
    }

    /// The world as drawn at match time `t` (kept at [`RATE`]).
    pub fn push(&mut self, t: f32, world: &RenderWorld) {
        if self.frames.back().is_some_and(|f| t - f.t < 0.9 / RATE) {
            return;
        }
        let health = |p: PlayerId| world.health.iter().find(|h| h.0 == p).map_or(255, |h| (h.1.clamp(0.0, 1.0) * 255.0) as u8);
        let tanks = world
            .tanks
            .iter()
            .map(|v| {
                let parts = match self.parts.get(&v.player) {
                    Some(p) if **p == v.parts => p.clone(),
                    _ => {
                        let p = Rc::new(v.parts.clone());
                        self.parts.insert(v.player, p.clone());
                        p
                    }
                };
                RTank {
                    player: v.player,
                    colour: v.colour,
                    pos: v.pos,
                    rot: v.rot,
                    turret_yaw: v.turret_yaw,
                    gun_pitch: v.gun_pitch,
                    gun: v.gun,
                    parts,
                    alive: v.alive,
                    boosting: v.boosting,
                    burning: v.burning,
                    class: v.class,
                    speed: v.speed,
                    throttle: v.throttle,
                    health: health(v.player),
                }
            })
            .collect();
        self.frames.push_back(Frame { t, tanks, shells: world.shells.clone(), vehicles: world.vehicles.clone() });
        while self.frames.front().is_some_and(|f| f.t < t - KEEP_SECS) {
            self.frames.pop_front();
        }
    }

    pub fn event(&mut self, t: f32, e: &Event) {
        if keep(e) {
            self.events.push_back((t, e.clone()));
        }
        while self.events.front().is_some_and(|(et, _)| *et < t - KEEP_SECS) {
            self.events.pop_front();
        }
    }

    /// The best moment: the biggest multi-kill, else the biggest damage
    /// burst. `counts` says whose moments count (not raiders).
    pub fn highlight(&self, counts: &dyn Fn(PlayerId) -> bool) -> Option<Highlight> {
        let (t0, t1) = (self.frames.front()?.t, self.frames.back()?.t);
        let kills: Vec<(f32, PlayerId, PlayerId)> = self
            .events
            .iter()
            .filter_map(|(t, e)| match e {
                Event::Destroyed { victim, killer: Some(k), .. } if k != victim && counts(*k) => Some((*t, *k, *victim)),
                _ => None,
            })
            .collect();
        let hits: Vec<(f32, PlayerId, f32)> = self
            .events
            .iter()
            .filter_map(|(t, e)| match e {
                Event::Hit { attacker: Some(a), target, damage, .. } if a != target && counts(*a) => Some((*t, *a, *damage)),
                _ => None,
            })
            .collect();
        let mut best: Option<(f32, Highlight)> = None;
        let mut consider = |score: f32, h: Highlight| {
            if best.as_ref().is_none_or(|(s, _)| score > *s) {
                best = Some((score, h));
            }
        };
        // Multi-kills (a single wreck counts too, with its damage).
        for &(t, k, _) in &kills {
            let run: Vec<f32> = kills.iter().filter(|(kt, kk, _)| *kk == k && *kt >= t && *kt <= t + MULTI_WINDOW).map(|x| x.0).collect();
            let last = run.iter().copied().fold(t, f32::max);
            let damage: f32 = hits.iter().filter(|(ht, a, _)| *a == k && *ht >= t - 2.0 && *ht <= last).map(|h| h.2).sum();
            let victims = kills.iter().filter(|(kt, kk, _)| *kk == k && *kt >= t && *kt <= last).map(|x| x.2).collect();
            consider(run.len() as f32 * 1000.0 + damage, Highlight { focus: k, start: t - 2.0, end: last + 1.5, kills: run.len() as u32, damage, victims });
        }
        // Damage bursts.
        for &(t, a, _) in &hits {
            let in_burst: Vec<&(f32, PlayerId, f32)> = hits.iter().filter(|(ht, aa, _)| *aa == a && *ht >= t && *ht <= t + BURST_WINDOW).collect();
            let damage: f32 = in_burst.iter().map(|h| h.2).sum();
            let last = in_burst.iter().map(|h| h.0).fold(t, f32::max);
            if damage >= 150.0 {
                consider(damage, Highlight { focus: a, start: t - 1.0, end: last + 1.5, kills: 0, damage, victims: Vec::new() });
            }
        }
        let (_, mut h) = best?;
        // 4..6 s, inside what was recorded.
        let len = (h.end - h.start).clamp(MIN_LEN, MAX_LEN);
        let mid = (h.start + h.end) * 0.5;
        h.start = (mid - len * 0.5).max(t0);
        h.end = (h.start + len).min(t1);
        h.start = (h.end - len).max(t0);
        (h.end - h.start >= 2.0).then_some(h)
    }

    /// The replay of `h`: its frames and events, copied out.
    pub fn playback(&self, h: Highlight) -> Playback {
        let frames: Vec<Frame> = self.frames.iter().filter(|f| f.t >= h.start - 0.2 && f.t <= h.end + 0.2).cloned().collect();
        let events = self.events.iter().filter(|(t, _)| *t >= h.start && *t <= h.end).cloned().collect();
        Playback { t: h.start, angle: 0.0, next_event: 0, frames, events, target: None, h }
    }
}

/// The picked moment.
#[derive(Clone, Debug)]
pub struct Highlight {
    pub focus: PlayerId,
    /// Match time, s.
    pub start: f32,
    pub end: f32,
    pub kills: u32,
    pub damage: f32,
    pub victims: Vec<PlayerId>,
}

impl Highlight {
    /// The plate's line: "Triple wreck", "640 damage in 3 s".
    pub fn caption(&self) -> String {
        match self.kills {
            0 => format!("{} damage in {:.0} s", self.damage.round(), (self.end - self.start - 1.5).max(1.0)),
            1 => "The wreck of the match".into(),
            2 => "Double wreck".into(),
            3 => "Triple wreck".into(),
            4 => "Four in a row".into(),
            n => format!("{n} wrecks, one crew"),
        }
    }
}

pub struct Playback {
    pub h: Highlight,
    t: f32,
    angle: f32,
    next_event: usize,
    frames: Vec<Frame>,
    events: Vec<(f32, Event)>,
    target: Option<Vec3>,
}

impl Playback {
    pub fn done(&self) -> bool {
        self.t >= self.h.end
    }

    /// Seconds of replay left (at replay speed).
    pub fn left(&self) -> f32 {
        ((self.h.end - self.t) / SPEED).max(0.0)
    }

    /// Advances by `dt` of wall time; fills `out` (a live world: the
    /// statics and pickups stay as they are) with the moment's tanks and
    /// shells and a camera, and returns the events now due.
    pub fn frame(&mut self, dt: f32, out: &mut RenderWorld, col: &dyn CameraCollider) -> Vec<Event> {
        self.t = (self.t + dt * SPEED).min(self.h.end);
        let i = self.frames.partition_point(|f| f.t <= self.t);
        let (a, b) = match (i.checked_sub(1).and_then(|k| self.frames.get(k)), self.frames.get(i)) {
            (Some(a), Some(b)) => (a, b),
            (Some(a), None) => (a, a),
            (None, Some(b)) => (b, b),
            (None, None) => return Vec::new(),
        };
        let k = if b.t > a.t { ((self.t - a.t) / (b.t - a.t)).clamp(0.0, 1.0) } else { 0.0 };
        out.tanks = a
            .tanks
            .iter()
            .map(|ta| {
                let tb = b.tanks.iter().find(|t| t.player == ta.player).unwrap_or(ta);
                // A respawn or teleport between frames: don't sweep across the map.
                let near = ta.pos.distance(tb.pos) < 8.0;
                let lerp = |x: f32, y: f32| if near { x + (y - x) * k } else { x };
                TankView {
                    player: ta.player,
                    colour: ta.colour,
                    pos: if near { ta.pos.lerp(tb.pos, k) } else { ta.pos },
                    rot: if near { ta.rot.slerp(tb.rot, k) } else { ta.rot },
                    turret_yaw: ta.turret_yaw + super::wrap(tb.turret_yaw - ta.turret_yaw) * if near { k } else { 0.0 },
                    gun_pitch: lerp(ta.gun_pitch, tb.gun_pitch),
                    wheels: [0.15; 10],
                    track_speed: [ta.speed, ta.speed],
                    gun: ta.gun,
                    parts: (*ta.parts).clone(),
                    alive: if k < 0.5 { ta.alive } else { tb.alive },
                    boosting: ta.boosting,
                    burning: ta.burning,
                    flaming: false,
                    throttle: lerp(ta.throttle, tb.throttle),
                    speed: lerp(ta.speed, tb.speed),
                    class: ta.class,
                }
            })
            .collect();
        out.health = a.tanks.iter().map(|t| (t.player, f32::from(t.health) / 255.0)).collect();
        out.shells = if k < 0.5 { a.shells.clone() } else { b.shells.clone() };
        out.vehicles = a.vehicles.clone();
        out.flaming.clear();
        out.shields.clear();

        // The camera: a slow circle round the crew of the moment, aimed a
        // little toward whoever it's fighting.
        if let Some(f) = out.tanks.iter().find(|t| t.player == self.h.focus) {
            let foe = out
                .tanks
                .iter()
                .filter(|t| t.player != f.player && t.pos.distance(f.pos) < 60.0)
                .filter(|t| self.h.victims.contains(&t.player) || self.h.victims.is_empty())
                .min_by(|x, y| x.pos.distance(f.pos).total_cmp(&y.pos.distance(f.pos)))
                .map(|t| t.pos);
            let want = match foe {
                Some(p) => f.pos + (p - f.pos) * 0.3,
                None => f.pos,
            } + Vec3::Y * 1.3;
            let target = match self.target {
                Some(t) => t + (want - t) * (1.0 - (-dt * 3.0).exp()),
                None => want,
            };
            self.target = Some(target);
            if self.angle == 0.0 {
                // Start behind the crew's turret: the moment from its side.
                let fwd = f.turret_transform().transform_vector3(Vec3::X);
                self.angle = (-fwd.z).atan2(fwd.x) + std::f32::consts::PI * 0.8;
            }
            let r = 15.0;
            let at = |a: f32| target + Vec3::new(a.cos() * r, 4.5, -a.sin() * r);
            // Swing on faster while something's in the way.
            let blocked = col.free_fraction(target + Vec3::Y, at(self.angle)) < 0.7;
            self.angle += dt * if blocked { 1.6 } else { 0.3 };
            let eye = at(self.angle);
            let free = col.free_fraction(target + Vec3::Y, eye).clamp(0.25, 1.0);
            let eye = target + Vec3::Y + (eye - target - Vec3::Y) * free;
            out.camera = Some(Camera::look(eye, target, 50.0));
        }

        let mut due = Vec::new();
        while let Some((et, e)) = self.events.get(self.next_event) {
            if *et > self.t {
                break;
            }
            due.push(e.clone());
            self.next_event += 1;
        }
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(t: f32, x: f32) -> RenderWorld {
        let tank = |p: PlayerId, x: f32| TankView {
            player: p,
            colour: 0,
            pos: Vec3::new(x, 0.0, p as f32 * 5.0),
            rot: Quat::IDENTITY,
            turret_yaw: 0.0,
            gun_pitch: 0.0,
            wheels: [0.15; 10],
            track_speed: [0.0; 2],
            gun: PartKind::StubGun,
            parts: vec![PartKind::StubGun],
            alive: true,
            boosting: false,
            burning: false,
            flaming: false,
            throttle: 0.0,
            speed: 0.0,
            class: ChassisClass::Brawler,
        };
        let _ = t;
        RenderWorld { tanks: vec![tank(1, x), tank(2, -x), tank(3, 0.0)], ..RenderWorld::default() }
    }

    fn kill(k: PlayerId, v: PlayerId) -> Event {
        Event::Destroyed { victim: v, killer: Some(k), pos: Vec3::ZERO, weapon: arena_core::Weapon::Gun(arena_core::GunKind::Stub) }
    }

    #[test]
    fn picks_the_double_and_replays_it() {
        let mut r = Recorder::default();
        for i in 0..900 {
            let t = i as f32 / 10.0;
            r.push(t, &world(t, t));
            if i == 300 {
                r.event(t, &kill(2, 3));
            }
            if i == 700 || i == 740 {
                r.event(t, &kill(1, if i == 700 { 2 } else { 3 }));
            }
        }
        // A minute kept, ten frames a second.
        assert!(r.frames.len() <= (KEEP_SECS * RATE) as usize + 1);
        assert!(r.frames.front().unwrap().t >= 89.8 - KEEP_SECS);
        assert!(r.events.iter().all(|(t, _)| *t >= 89.8 - KEEP_SECS), "the old kill is gone");
        let h = r.highlight(&|_| true).expect("a highlight");
        assert_eq!((h.focus, h.kills), (1, 2));
        assert!(h.end - h.start >= MIN_LEN - 0.01 && h.end - h.start <= MAX_LEN + 0.01, "{h:?}");
        assert!(h.start <= 70.0 && h.end >= 74.0, "{h:?}");
        assert_eq!(h.caption(), "Double wreck");
        let mut p = r.playback(h);
        let mut out = RenderWorld::default();
        let mut seen = 0;
        let mut steps = 0;
        while !p.done() && steps < 1000 {
            seen += p.frame(1.0 / 60.0, &mut out, &crate::camera::OpenAir).len();
            steps += 1;
        }
        assert_eq!(seen, 2, "both wrecks replay");
        assert!(out.camera.is_some() && out.tanks.len() == 3);
        assert!(steps as f32 / 60.0 > MIN_LEN / SPEED - 0.1, "{steps}");
        // Nobody did anything worth showing: no replay.
        let mut quiet = Recorder::default();
        for i in 0..100 {
            quiet.push(i as f32 / 10.0, &world(0.0, 0.0));
        }
        assert!(quiet.highlight(&|_| true).is_none());
    }
}
