//! Timed power-ups (like the 2013 TANSK): rare glowing pickups at a few set
//! spots per map. Drive over one for a buff that runs for some seconds.
//!
//! API for the client (VFX, HUD, radio):
//! - [`PowerUpKind`]: what it is. A pickup of it is `PickupKind::PowerUp(kind)`
//!   in `Snapshot::pickups`.
//! - Events: `Event::PowerUpSpawned { kind, pos }` (announce it on the radio),
//!   `Event::PickedUp { kind: PickupKind::PowerUp(_), .. }`, then
//!   `Event::PowerUpStarted { player, kind, seconds }` and
//!   `Event::PowerUpEnded { player, kind }` (timed out, bubble popped, or wrecked).
//! - `TankSnap::powerups`: the buffs a tank has now ([`PowerUpSnap`]: kind,
//!   seconds left, strength 0..255; for the Bubble the strength is what's left
//!   of its soak, which is what the shield bubble should show).
//!
//! The sim side: [`PowerUps`] holds the live buffs (`Sim::powerups`); the
//! hooks are `powerup_pre` / `powerup_post` round each tank's tick, the
//! damage multiplier and the bubble soak in `Sim::damage`, and the spawn spots
//! as ordinary pickup `Site`s.

use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

use crate::events::{Event, PickupKind, Weapon};
use crate::map::MapId;
use crate::sim::{Site, Sim};
use crate::PlayerId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PowerUpKind {
    /// Double damage.
    Overcharge,
    /// Faster, and the boost never runs dry.
    Nitro,
    /// A shield bubble that soaks damage until it pops or runs out.
    Bubble,
    /// Rapid fire: the main gun reloads in a fraction of the time.
    Gatling,
    /// Pulls loose scrap in from round about.
    Magnet,
}

impl PowerUpKind {
    pub const ALL: [PowerUpKind; 5] = [PowerUpKind::Overcharge, PowerUpKind::Nitro, PowerUpKind::Bubble, PowerUpKind::Gatling, PowerUpKind::Magnet];

    pub fn name(self) -> &'static str {
        match self {
            PowerUpKind::Overcharge => "Overcharge",
            PowerUpKind::Nitro => "Nitro",
            PowerUpKind::Bubble => "Bubble",
            PowerUpKind::Gatling => "Gatling",
            PowerUpKind::Magnet => "Magnet",
        }
    }

    /// How long it lasts, seconds.
    pub fn duration(self, t: &PowerUpTuning) -> f32 {
        match self {
            PowerUpKind::Overcharge => t.overcharge_secs,
            PowerUpKind::Nitro => t.nitro_secs,
            PowerUpKind::Bubble => t.bubble_secs,
            PowerUpKind::Gatling => t.gatling_secs,
            PowerUpKind::Magnet => t.magnet_secs,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PowerUpTuning {
    /// Spots per map (the map's middle, then round it).
    pub spots: usize,
    /// Seconds into a match before the first ones appear.
    pub first_spawn: f32,
    /// Seconds before a spot offers another, after it's taken.
    pub respawn_min: f32,
    pub respawn_max: f32,
    pub overcharge_secs: f32,
    pub overcharge_damage_mult: f32,
    pub nitro_secs: f32,
    pub nitro_speed_mult: f32,
    pub bubble_secs: f32,
    /// Damage the bubble soaks before it pops.
    pub bubble_hp: f32,
    pub gatling_secs: f32,
    /// Main-gun reload multiplier.
    pub gatling_reload_mult: f32,
    pub magnet_secs: f32,
    /// Loose scrap inside this radius drifts in at this speed (m/s).
    pub magnet_radius: f32,
    pub magnet_pull: f32,
}

impl Default for PowerUpTuning {
    fn default() -> Self {
        Self {
            spots: 3,
            first_spawn: 40.0,
            respawn_min: 45.0,
            respawn_max: 70.0,
            overcharge_secs: 12.0,
            overcharge_damage_mult: 2.0,
            nitro_secs: 12.0,
            nitro_speed_mult: 1.3,
            bubble_secs: 15.0,
            bubble_hp: 150.0,
            gatling_secs: 10.0,
            gatling_reload_mult: 0.3,
            magnet_secs: 15.0,
            magnet_radius: 35.0,
            magnet_pull: 16.0,
        }
    }
}

/// One live buff.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Buff {
    pub player: PlayerId,
    pub kind: PowerUpKind,
    pub left: f32,
    /// The Bubble: damage it can still soak.
    pub hp: f32,
}

/// The live buffs of a match.
#[derive(Clone, Debug, Default)]
pub struct PowerUps {
    pub buffs: Vec<Buff>,
}

impl PowerUps {
    pub fn has(&self, player: PlayerId, kind: PowerUpKind) -> bool {
        self.buffs.iter().any(|b| b.player == player && b.kind == kind)
    }

    /// The buffs `player` has, for its snapshot.
    pub fn snap(&self, player: PlayerId, t: &PowerUpTuning) -> Vec<PowerUpSnap> {
        self.buffs
            .iter()
            .filter(|b| b.player == player)
            .map(|b| PowerUpSnap {
                kind: b.kind,
                seconds_left: b.left,
                strength: if b.kind == PowerUpKind::Bubble {
                    (b.hp / t.bubble_hp.max(1.0) * 255.0).clamp(0.0, 255.0) as u8
                } else {
                    (b.left / b.kind.duration(t).max(0.1) * 255.0).clamp(0.0, 255.0) as u8
                },
            })
            .collect()
    }
}

/// A buff on a tank, in its snapshot (`TankSnap::powerups`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PowerUpSnap {
    pub kind: PowerUpKind,
    pub seconds_left: f32,
    /// 0..255: the Bubble's soak left, otherwise the time left of the full run.
    pub strength: u8,
}

/// Hand-placed spots per map (ground plane x, z). Maps without any get
/// spots worked out from their nav grid (the middle and a ring round it).
fn hand_spots(_id: MapId) -> &'static [(f32, f32)] {
    &[]
}

/// What a tank's tick needs put back afterwards.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PreTick {
    speed: Option<(f32, f32)>,
    can_boost: Option<bool>,
    reload: f32,
}

impl Sim {
    /// The power-up spots: pickup sites whose kind is rolled at each spawn.
    pub(crate) fn init_powerups(&mut self) {
        let t = self.tuning.powerups.clone();
        let mut spots: Vec<Vec2> = hand_spots(self.map.id).iter().map(|(x, z)| Vec2::new(*x, *z)).collect();
        if spots.is_empty() && t.spots > 0 {
            let half = self.map.play_half;
            let turn = (self.map.id as u32 as f32) * 0.7 + 0.4;
            let mut cands = vec![Vec2::ZERO];
            for k in 0..t.spots.saturating_sub(1) {
                let a = turn + k as f32 * std::f32::consts::TAU / (t.spots - 1) as f32;
                cands.push(Vec2::new(a.cos(), a.sin()) * half * 0.5);
            }
            for c in cands {
                let p = self.nav.nearest_free_point(c);
                if !self.nav.is_blocked(p) && spots.iter().all(|s| s.distance(p) > 30.0) {
                    spots.push(p);
                }
            }
        }
        for s in spots.into_iter().take(t.spots.max(1)) {
            let y = self.map.height_at(s.x, s.y);
            let timer = t.first_spawn + self.rng.range(0.0, 15.0);
            self.sites.push(Site { pos: Vec3::new(s.x, y, s.y), kind: PickupKind::PowerUp(PowerUpKind::Overcharge), timer, live: None });
        }
    }

    /// A power-up site is about to spawn: roll its kind and call it on the
    /// radio. Returns false to hold it back (too early in the match).
    pub(crate) fn powerup_site_spawn(&mut self, si: usize) -> bool {
        let early = self.tuning.powerups.first_spawn - self.match_ticks as f32 / crate::TICK_HZ as f32;
        if early > 0.0 {
            self.sites[si].timer = early;
            return false;
        }
        let kind = PowerUpKind::ALL[self.rng.below(PowerUpKind::ALL.len())];
        self.sites[si].kind = PickupKind::PowerUp(kind);
        self.events.push(Event::PowerUpSpawned { kind, pos: self.sites[si].pos });
        true
    }

    pub(crate) fn powerup_respawn_time(&mut self) -> f32 {
        let t = &self.tuning.powerups;
        let (a, b) = (t.respawn_min, t.respawn_max);
        self.rng.range(a, b)
    }

    /// Player index `j` drove over a power-up. Taking one it already has
    /// starts it over.
    pub(crate) fn start_powerup(&mut self, j: usize, kind: PowerUpKind) {
        let t = &self.tuning.powerups;
        let player = self.players[j].id;
        let left = kind.duration(t);
        let hp = if kind == PowerUpKind::Bubble { t.bubble_hp } else { 0.0 };
        self.powerups.buffs.retain(|b| !(b.player == player && b.kind == kind));
        self.powerups.buffs.push(Buff { player, kind, left, hp });
        self.events.push(Event::PowerUpStarted { player, kind, seconds: left });
    }

    /// Before player index `i`'s tick: Nitro's speed and endless boost.
    pub(crate) fn powerup_pre(&mut self, i: usize) -> PreTick {
        let mut pre = PreTick { reload: self.players[i].tank.reload, ..Default::default() };
        let id = self.players[i].id;
        if self.powerups.buffs.is_empty() || !self.powerups.has(id, PowerUpKind::Nitro) {
            return pre;
        }
        let m = self.tuning.powerups.nitro_speed_mult;
        let tank = &mut self.players[i].tank;
        let was = tank.stats.speed_mult;
        tank.stats.speed_mult = was * m;
        pre.speed = Some((was, was * m));
        pre.can_boost = Some(tank.stats.can_boost);
        tank.stats.can_boost = true;
        tank.fuel = tank.stats.fuel_max;
        pre
    }

    /// After player index `i`'s tick: put Nitro's stats back, and Gatling
    /// cuts the reload of a shot fired this tick.
    pub(crate) fn powerup_post(&mut self, i: usize, pre: PreTick) {
        if self.powerups.buffs.is_empty() {
            return;
        }
        let id = self.players[i].id;
        let gatling = self.powerups.has(id, PowerUpKind::Gatling);
        let mult = self.tuning.powerups.gatling_reload_mult;
        let tank = &mut self.players[i].tank;
        if let Some((was, boosted)) = pre.speed {
            // (Unless the stats were rebuilt meanwhile.)
            if tank.stats.speed_mult == boosted {
                tank.stats.speed_mult = was;
            }
        }
        if let Some(cb) = pre.can_boost {
            tank.stats.can_boost = cb;
            tank.fuel = tank.stats.fuel_max;
        }
        if gatling && tank.reload > pre.reload + 0.05 {
            tank.reload *= mult;
        }
    }

    /// Damage dealt by `attacker` is multiplied by this (Overcharge).
    pub(crate) fn powerup_damage_mult(&self, attacker: Option<PlayerId>) -> f32 {
        match attacker {
            Some(a) if !self.powerups.buffs.is_empty() && self.powerups.has(a, PowerUpKind::Overcharge) => self.tuning.powerups.overcharge_damage_mult,
            _ => 1.0,
        }
    }

    /// The Bubble soaks damage first. Returns what's left to apply.
    pub(crate) fn powerup_absorb(&mut self, j: usize, amount: f32, pos: Vec3, w: Weapon) -> Option<f32> {
        if self.powerups.buffs.is_empty() || w == Weapon::Burn {
            return Some(amount);
        }
        let id = self.players[j].id;
        let Some(b) = self.powerups.buffs.iter_mut().find(|b| b.player == id && b.kind == PowerUpKind::Bubble) else {
            return Some(amount);
        };
        let a = b.hp.min(amount);
        b.hp -= a;
        self.players[j].tank.since_damage = 0.0;
        self.events.push(Event::ShieldHit { player: id, pos, damage: a });
        let left = amount - a;
        (left > 0.001).then_some(left)
    }

    /// Whether `j` is inside a Bubble (parts can't be shot off then).
    pub(crate) fn in_bubble(&self, j: usize) -> bool {
        !self.powerups.buffs.is_empty() && self.powerups.has(self.players[j].id, PowerUpKind::Bubble)
    }

    /// Per tick: run the clocks down, end buffs (time, popped bubble, wrecked
    /// or gone), and the Magnet's pull on loose scrap.
    pub(crate) fn step_powerups(&mut self, dt: f32) {
        if self.powerups.buffs.is_empty() {
            return;
        }
        let t = self.tuning.powerups.clone();
        let mut k = 0;
        while k < self.powerups.buffs.len() {
            let b = &mut self.powerups.buffs[k];
            b.left -= dt;
            let alive = self.players.iter().any(|p| p.id == b.player && p.tank.alive);
            let popped = b.kind == PowerUpKind::Bubble && b.hp <= 0.0;
            if b.left <= 0.0 || popped || !alive || !matches!(self.phase, crate::sim::Phase::Playing) {
                let (player, kind) = (b.player, b.kind);
                self.powerups.buffs.swap_remove(k);
                if popped {
                    self.events.push(Event::ShieldBroken { player });
                }
                self.events.push(Event::PowerUpEnded { player, kind });
                continue;
            }
            k += 1;
        }
        let ph = self.map.play_half - 3.0;
        for b in self.powerups.buffs.iter().filter(|b| b.kind == PowerUpKind::Magnet) {
            let Some(p) = self.players.iter().find(|p| p.id == b.player) else { continue };
            let c = self.phys.body(p.tank.body).translation();
            for q in self.pickups.iter_mut().filter(|q| matches!(q.kind, PickupKind::Scrap(_))) {
                let to = Vec2::new(c.x - q.pos.x, c.z - q.pos.z);
                let d = to.length();
                if d < t.magnet_radius && d > 0.5 {
                    let step = (t.magnet_pull * dt).min(d);
                    let n = to / d * step;
                    q.pos.x = (q.pos.x + n.x).clamp(-ph, ph);
                    q.pos.z = (q.pos.z + n.y).clamp(-ph, ph);
                    let g = self.map.height_at(q.pos.x, q.pos.z) + 0.3;
                    q.pos.y = if q.settled { g } else { q.pos.y.max(g) };
                }
            }
        }
    }
}
