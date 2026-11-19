//! Raider Waves: co-op PvE. Hold the outpost near the middle of the map
//! against escalating waves of raider machines from the edges; every 5th
//! wave brings THE WAR RIG, a boss whose modules are shot off one by one.
//!
//! Raiders are ordinary players on team 1 driven by bot brains, on their own
//! chassis (`VehicleKind`). Defenders are team 0. No friendly fire.

use glam::{Quat, Vec2, Vec3};
use serde::{Deserialize, Serialize};

use crate::bots::Brain;
use crate::events::{Event, HitLocation, PickupKind, Weapon};
use crate::map::ScrapKind;
use crate::parts::{PartKind, VehicleKind, BOSS_MODULES, RIG_PLATES, SHIP_MODULES, SHIP_PLATES};
use crate::physics::pose;
use crate::sim::{Phase, Player, Sim};
use crate::snapshot::q01;
use crate::tank::{HullPose, Tank};
use crate::tuning::GunStats;
use crate::{Input, PlayerId};

/// Which game a `Sim` runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ModeKind {
    /// Free-for-all, most kills (the default).
    #[default]
    ScrapBrawl,
    /// Co-op: hold the outpost against raider waves.
    RaiderWaves,
    /// Rustbacks (team 0) vs Dust Saints (team 1), team kills, no friendly fire.
    TeamBrawl,
    /// Team PvP over a fuel tanker: one team escorts it along the map's route,
    /// the other stops it; sides swap for round 2. See [`crate::convoy`].
    Convoy,
}

pub const TEAM_DEFENDERS: u8 = 0;
pub const TEAM_RAIDERS: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaidPhase {
    /// Counting down to the next wave (`timer`).
    Breather,
    Fighting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BossKind {
    WarRig,
    /// A beached ship hull on giant tracks: deck guns, mortars, broadside MGs,
    /// and a stern hangar that drops buggies. Boss waves alternate Rig, Land-Ship.
    LandShip,
}

impl BossKind {
    /// The chassis this boss is built on.
    pub fn vehicle(self) -> VehicleKind {
        match self {
            BossKind::WarRig => VehicleKind::WarRig,
            BossKind::LandShip => VehicleKind::LandShip,
        }
    }

    /// The boss on a chassis, if it is one.
    pub fn of(v: VehicleKind) -> Option<BossKind> {
        match v {
            VehicleKind::WarRig => Some(BossKind::WarRig),
            VehicleKind::LandShip => Some(BossKind::LandShip),
            _ => None,
        }
    }

    /// Its modules, in `BossSnap::modules` order.
    pub fn modules(self) -> &'static [PartKind] {
        match self {
            BossKind::WarRig => &BOSS_MODULES,
            BossKind::LandShip => &SHIP_MODULES,
        }
    }

    /// The plates that shield its core.
    pub fn plates(self) -> &'static [PartKind] {
        match self {
            BossKind::WarRig => &RIG_PLATES,
            BossKind::LandShip => &SHIP_PLATES,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LossReason {
    OutpostFell,
    AllDefendersDown,
}

#[derive(Clone, Debug)]
pub struct RaidState {
    pub outpost: Vec3,
    pub radius: f32,
    pub health: f32,
    pub max_health: f32,
    /// The current (or last) wave number; 0 before the first.
    pub wave: u32,
    pub phase: RaidPhase,
    pub timer: f32,
    pub queue: Vec<VehicleKind>,
    pub spawn_timer: f32,
    pub waves_survived: u32,
    pub lost: Option<LossReason>,
    pub bosses_spawned: u32,
    pub bosses_killed: u32,
    /// Seconds each defeated boss lived.
    pub boss_lifetimes: Vec<f32>,
    /// Living bosses: (player, spawn time in match seconds).
    pub bosses: Vec<(PlayerId, f32)>,
    /// Every boss that spawned, in order (statistics: per-kind counts).
    pub boss_spawn_log: Vec<BossKind>,
    /// Every boss defeated: (kind, seconds it lived).
    pub boss_defeat_log: Vec<(BossKind, f32)>,
    /// Living Land-Ships' gunnery (turret aim, reloads, hangar clock).
    pub ships: Vec<crate::landship::ShipState>,
    pending_hit: f32,
    /// Per raider: (id, best distance to the outpost, seconds without progress).
    progress: Vec<(PlayerId, f32, f32)>,
}

/// Raid state for the client (outpost bar, wave banner, countdown).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RaidSnap {
    pub outpost: [f32; 3],
    pub radius: f32,
    pub health: u16,
    pub max_health: u16,
    pub wave: u16,
    pub phase: RaidPhase,
    /// Ticks to the next wave during a breather.
    pub ticks_left: u32,
    pub raiders_alive: u16,
    pub raiders_queued: u16,
    pub waves_survived: u16,
}

/// A boss and its modules, for the boss bar and drawing (modules use `part_def`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BossSnap {
    pub player: PlayerId,
    pub kind: BossKind,
    pub health: u16,
    pub max_health: u16,
    /// True once all plates are gone and the core takes full damage.
    pub core_exposed: bool,
    pub modules: Vec<ModuleSnap>,
    /// Added in wave 7: independently aimed turrets (the Land-Ship's deck guns;
    /// empty for the War Rig, whose cannon is its turret).
    pub turrets: Vec<TurretSnap>,
}

/// One boss turret's aim, relative to the boss hull (yaw + = left, pitch + = up).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TurretSnap {
    pub part: PartKind,
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleSnap {
    pub kind: PartKind,
    /// 0..255 of max; 0 when lost.
    pub health: u8,
    pub alive: bool,
}

fn blast(splash: f32, radius: f32, knockback: f32) -> GunStats {
    GunStats { damage: 0.0, splash_damage: splash, splash_radius: radius, knockback, ..GunStats::default() }
}

impl Sim {
    /// A Raider Waves match. Add 1-4 humans with `add_player` (and optional
    /// bot allies with `add_bot`); raiders are spawned by the sim.
    pub fn new_raid(seed: u64, map: crate::map::Map, tuning: crate::tuning::Tuning) -> Sim {
        Sim::with_mode(seed, map, tuning, ModeKind::RaiderWaves)
    }

    /// Offline Raider Waves practice: you plus `allies` bot allies.
    pub fn practice_raid(seed: u64, allies: usize, difficulty: f32) -> (Sim, PlayerId) {
        Sim::practice_raid_on(crate::map::MapId::Sink, seed, allies, difficulty)
    }

    /// Raider Waves practice on map `id`.
    pub fn practice_raid_on(id: crate::map::MapId, seed: u64, allies: usize, difficulty: f32) -> (Sim, PlayerId) {
        let mut sim = Sim::new_raid(seed, crate::map::Map::by_id(id, seed), crate::tuning::Tuning::default());
        let me = sim.add_player("You");
        for i in 0..allies {
            sim.add_bot(crate::bots::BOT_NAMES[i % crate::bots::BOT_NAMES.len()], difficulty);
        }
        (sim, me)
    }

    pub(crate) fn init_raid(&mut self) {
        let r = self.tuning.raid.clone();
        // The outpost: the map's compound if it has one, else open flat ground near the middle.
        let mut c = Vec2::new(self.map.outpost.x, self.map.outpost.z);
        'search: for ring in 0..60 {
            if ring == 0 && self.map.outpost != Vec3::ZERO {
                break;
            }
            for k in 0..24 {
                let a = k as f32 * std::f32::consts::TAU / 24.0;
                let p = Vec2::new(a.cos(), a.sin()) * ring as f32 * 4.0;
                if !self.map.blocked_xz(p, r.outpost_radius + 4.0) && self.map.normal_at(p.x, p.y).y > 0.97 {
                    c = p;
                    break 'search;
                }
            }
        }
        let outpost = Vec3::new(c.x, self.map.height_at(c.x, c.y), c.y);
        self.raid = Some(RaidState {
            outpost,
            radius: r.outpost_radius,
            health: r.outpost_health,
            max_health: r.outpost_health,
            wave: 0,
            phase: RaidPhase::Breather,
            timer: r.first_wave_delay,
            queue: Vec::new(),
            spawn_timer: 0.0,
            waves_survived: 0,
            lost: None,
            bosses_spawned: 0,
            bosses_killed: 0,
            boss_lifetimes: Vec::new(),
            bosses: Vec::new(),
            boss_spawn_log: Vec::new(),
            boss_defeat_log: Vec::new(),
            ships: Vec::new(),
            pending_hit: 0.0,
            progress: Vec::new(),
        });
    }

    /// Team of a player (None in free-for-all).
    pub fn team_of(&self, id: PlayerId) -> Option<u8> {
        self.idx(id).and_then(|i| self.players[i].team)
    }

    /// Are these two players on the same side?
    pub fn same_team(&self, a: PlayerId, b: PlayerId) -> bool {
        matches!((self.team_of(a), self.team_of(b)), (Some(x), Some(y)) if x == y)
    }

    /// Defender spawn points ring the outpost.
    pub(crate) fn raid_spawn(&mut self) -> Option<(Vec3, f32)> {
        let raid = self.raid.as_ref()?;
        let o = raid.outpost;
        let r0 = raid.radius + 8.0;
        for _ in 0..40 {
            let a = self.rng.range(0.0, std::f32::consts::TAU);
            let r = r0 + self.rng.range(0.0, 10.0);
            let p = Vec2::new(o.x + a.cos() * r, o.z + a.sin() * r);
            if !self.map.blocked_xz(p, 5.0) {
                let d = Vec2::new(o.x, o.z) - p;
                return Some((Vec3::new(p.x, self.map.height_at(p.x, p.y) + 1.3, p.y), (-d.y).atan2(d.x)));
            }
        }
        Some((o + Vec3::Y * 1.3, 0.0))
    }

    /// Spawn a raider machine at the map edge.
    fn spawn_raider(&mut self, kind: VehicleKind) {
        self.spawn_raider_at(kind, None);
    }

    /// Spawn a raider machine at `at` (position, yaw), or at the map edge.
    pub(crate) fn spawn_raider_at(&mut self, kind: VehicleKind, at: Option<(Vec3, f32)>) -> Option<PlayerId> {
        let t = self.tuning.clone();
        let raid = self.raid.as_ref()?;
        let wave = raid.wave;
        let outpost = raid.outpost;
        let mut pos = Vec3::ZERO;
        let mut yaw = 0.0;
        let (margin, lift) = match kind {
            VehicleKind::WarRig => (10.0, 3.0),
            VehicleKind::LandShip => (18.0, 4.6),
            _ => (4.0, 1.3),
        };
        if let Some((p, y)) = at {
            pos = p;
            yaw = y;
        } else {
            for attempt in 0..60 {
                let a = self.rng.range(0.0, std::f32::consts::TAU);
                let r = self.map.play_half - if kind == VehicleKind::LandShip { 32.0 } else { 25.0 };
                let p = Vec2::new(a.cos() * r, a.sin() * r);
                // The Land-Ship turns like a ship: it wants open ground ahead of it.
                let clear_ahead = kind != VehicleKind::LandShip || attempt >= 50 || {
                    let to = (Vec2::new(outpost.x, outpost.z) - p).normalize_or(Vec2::X);
                    (1..=16).all(|k| !self.map.blocked_xz(p + to * (k as f32 * 5.0), 10.0))
                };
                if clear_ahead && !self.map.blocked_xz(p, margin) {

                    let d = Vec2::new(outpost.x, outpost.z) - p;
                    yaw = (-d.y).atan2(d.x);
                    pos = Vec3::new(p.x, self.map.height_at(p.x, p.y) + lift, p.y);
                    break;
                }
            }
        }
        let id = (0..=255u8).find(|i| self.players.iter().all(|p| p.id != *i))?;
        let mut tank = Tank::new_vehicle(&mut self.phys, &t, pos, yaw, kind);
        tank.protect = 0.0;
        let hm = 1.0 + t.raid.health_per_wave * wave.saturating_sub(1) as f32;
        tank.health_mult = hm;
        let name = match kind {
            VehicleKind::Buggy => "Raider buggy",
            VehicleKind::Technical => "Raider technical",
            VehicleKind::Tank => "Raider tank",
            VehicleKind::WarRig => "THE WAR RIG",
            VehicleKind::LandShip => "THE LAND-SHIP",
            VehicleKind::FuelTanker => "Fuel tanker",
        };
        let seed = self.rng.next_u64();
        self.players.push(Player {
            id,
            name: name.to_string(),
            input: Input::default(),
            tank,
            kills: 0,
            deaths: 0,
            brain: Some(Brain::new(seed, 0.4 + 0.04 * wave.min(10) as f32)),
            ram_cd: 0.0,
            team: Some(TEAM_RAIDERS),
            counts_as_human: false,
            loadout: crate::garage::Loadout::default(),
        });
        let j = self.players.len() - 1;
        match kind {
            VehicleKind::Tank => {
                let points = ((wave as f32 * t.raid.tank_tiers_per_wave) as u32).min(t.raid.tank_tiers_max);
                for _ in 0..points {
                    let k = ScrapKind::from_index(self.rng.below(4));
                    for _ in 0..t.upgrades.scrap_per_tier {
                        self.add_scrap(j, k);
                    }
                }
                // Branch picks stay at random defaults; clear the choice window.
                self.players[j].tank.choice = None;
                // Raider tanks don't get combo parts (shield bubbles etc. stay rare).
                let t2 = self.tuning.clone();
                self.players[j].tank.parts.retain(|p| p.tier > 0);
                self.players[j].tank.refresh_stats(&mut self.phys, &t2);
                self.players[j].tank.shield = 0.0;
            }
            VehicleKind::Technical => self.attach_special(j, PartKind::RocketRack),
            VehicleKind::WarRig => {
                let cycle = wave / t.raid.boss_every.max(1);
                let boss_mult = 1.0 + t.raid.boss_health_per_cycle * cycle.saturating_sub(1) as f32;
                self.players[j].tank.health_mult = boss_mult;
                for m in BOSS_MODULES {
                    self.attach_special(j, m);
                }
                for p in &mut self.players[j].tank.parts {
                    p.max_health *= boss_mult;
                    p.health = p.max_health;
                }
                let tk = &mut self.players[j].tank;
                tk.refresh_stats(&mut self.phys, &t);
                tk.health = tk.stats.max_health;
                let now = self.tick as f32 / 60.0;
                let raid = self.raid.as_mut().unwrap();
                raid.bosses_spawned += 1;
                raid.bosses.push((id, now));
                raid.boss_spawn_log.push(BossKind::WarRig);
                self.events.push(Event::BossSpawned { player: id, kind: BossKind::WarRig, pos });
            }
            VehicleKind::LandShip => {
                // Its own health cycle: the first Land-Ship (wave 10) is the baseline.
                let cycle = wave / t.raid.boss_every.max(1);
                let n = if t.raid.boss_alternate { cycle / 2 } else { cycle };
                let boss_mult = 1.0 + t.raid.boss_health_per_cycle * n.saturating_sub(1) as f32;
                self.players[j].tank.health_mult = boss_mult;
                for m in SHIP_MODULES {
                    self.attach_special(j, m);
                }
                for p in &mut self.players[j].tank.parts {
                    p.max_health *= boss_mult;
                    p.health = p.max_health;
                }
                let tk = &mut self.players[j].tank;
                tk.refresh_stats(&mut self.phys, &t);
                tk.health = tk.stats.max_health;
                let now = self.tick as f32 / 60.0;
                let hangar = t.raid.ship_hangar_interval * 0.5;
                let raid = self.raid.as_mut().unwrap();
                raid.bosses_spawned += 1;
                raid.bosses.push((id, now));
                raid.boss_spawn_log.push(BossKind::LandShip);
                raid.ships.push(crate::landship::ShipState::new(id, hangar));
                self.events.push(Event::BossSpawned { player: id, kind: BossKind::LandShip, pos });
            }
            VehicleKind::Buggy | VehicleKind::FuelTanker => {}
        }
        let tk = &mut self.players[j].tank;
        tk.health = tk.stats.max_health;
        self.events.push(Event::Respawned { player: id, pos });
        Some(id)
    }

    /// The boss for wave `wave` (a boss wave): War Rig and Land-Ship alternate.
    pub fn boss_for_wave(&self, wave: u32) -> BossKind {
        let r = &self.tuning.raid;
        let cycle = wave / r.boss_every.max(1);
        if r.boss_alternate && cycle % 2 == 0 {
            BossKind::LandShip
        } else {
            BossKind::WarRig
        }
    }

    fn start_wave(&mut self) {
        let t = self.tuning.raid.clone();
        let raid = self.raid.as_mut().unwrap();
        raid.wave += 1;
        let n = raid.wave as f32;
        let boss = t.boss_every > 0 && raid.wave % t.boss_every == 0;
        let cycle = raid.wave / t.boss_every.max(1);
        let boss_kind = if t.boss_alternate && cycle % 2 == 0 { BossKind::LandShip } else { BossKind::WarRig };
        let scale = match (boss, boss_kind) {
            (false, _) => 1.0,
            (true, BossKind::WarRig) => 0.5,
            (true, BossKind::LandShip) => t.ship_escort_scale,
        };
        let buggies = ((t.buggies_base + t.buggies_per_wave * n) * scale) as usize;
        let tanks = ((t.tanks_per_wave * (n - 1.0)).max(0.0) * scale) as usize;
        let techs = ((t.technicals_per_wave * (n - 2.0)).max(0.0) * scale) as usize;
        let mut q = Vec::new();
        q.extend(std::iter::repeat_n(VehicleKind::Buggy, buggies));
        q.extend(std::iter::repeat_n(VehicleKind::Tank, tanks));
        q.extend(std::iter::repeat_n(VehicleKind::Technical, techs));
        // Interleave so each wave arrives mixed (fast buggies first-ish).
        let mut mixed = Vec::new();
        let mut k = 0;
        while !q.is_empty() {
            let i = (k * 7) % q.len();
            mixed.push(q.remove(i));
            k += 1;
        }
        if boss {
            let at = mixed.len().min(2);
            mixed.insert(at, boss_kind.vehicle());
        }
        mixed.reverse(); // popped from the back
        let count = mixed.len() as u16;
        raid.queue = mixed;
        raid.phase = RaidPhase::Fighting;
        raid.spawn_timer = 0.0;
        let wave = raid.wave as u16;
        self.events.push(Event::WaveStarted { wave, raiders: count, boss });
    }

    /// Per-tick raid logic: wave clock, spawning, sabotage, win/lose.
    pub(crate) fn step_raid(&mut self, dt: f32) {
        let Some(raid) = self.raid.as_ref() else { return };
        if raid.lost.is_some() {
            return;
        }
        let t = self.tuning.raid.clone();
        let raiders_alive = self.players.iter().filter(|p| p.team == Some(TEAM_RAIDERS) && p.tank.alive).count();
        match raid.phase {
            RaidPhase::Breather => {
                let raid = self.raid.as_mut().unwrap();
                raid.timer -= dt;
                if raid.timer <= 0.0 {
                    self.start_wave();
                }
            }
            RaidPhase::Fighting => {
                let raid = self.raid.as_mut().unwrap();
                raid.spawn_timer -= dt;
                if raid.spawn_timer <= 0.0 && !raid.queue.is_empty() && raiders_alive < t.max_alive {
                    let kind = raid.queue.pop().unwrap();
                    raid.spawn_timer = t.spawn_interval;
                    self.spawn_raider(kind);
                } else if raid.queue.is_empty() && raiders_alive == 0 {
                    // Wave cleared: breather, repairs, everyone back.
                    raid.waves_survived = raid.wave;
                    raid.phase = RaidPhase::Breather;
                    raid.timer = t.breather;
                    let heal = (raid.max_health * t.breather_repair).min(raid.max_health - raid.health);
                    raid.health += heal;
                    let wave = raid.wave as u16;
                    self.events.push(Event::WaveCleared { wave });
                    self.events.push(Event::OutpostRepaired { amount: heal });
                    for j in 0..self.players.len() {
                        if self.players[j].team == Some(TEAM_DEFENDERS) && !self.players[j].tank.alive {
                            self.players[j].tank.respawn_in = 0.0;
                        }
                    }
                }
            }
        }

        self.recycle_stuck_raiders(dt);

        // Sabotage: raiders inside the zone drain it; buggies blow themselves up on it.
        let (o, rad) = {
            let r = self.raid.as_ref().unwrap();
            (r.outpost, r.radius)
        };
        let mut drain = 0.0;
        let mut kamikaze = Vec::new();
        for (j, p) in self.players.iter().enumerate() {
            if p.team != Some(TEAM_RAIDERS) || !p.tank.alive {
                continue;
            }
            let c = self.phys.body(p.tank.body).translation();
            if Vec2::new(c.x - o.x, c.z - o.z).length() < rad {
                match p.tank.vehicle {
                    VehicleKind::Buggy => kamikaze.push(j),
                    VehicleKind::WarRig | VehicleKind::LandShip => drain += t.boss_sabotage_dps * dt,
                    _ => drain += t.sabotage_dps * dt,
                }
            }
        }
        for j in kamikaze.into_iter().rev() {
            let c = self.phys.body(self.players[j].tank.body).translation();
            drain += t.buggy_kamikaze;
            self.players[j].tank.protect = 0.0;
            let pos = c;
            let id = self.players[j].id;
            self.damage(j, Some(id), HitLocation::Hull, 1.0e6, pos, Vec3::Y, Weapon::Hazard);
            self.explode(pos, Some(id), &blast(40.0, 7.0, 60_000.0), 1.0, None, Weapon::Hazard);
        }
        self.hit_outpost(drain, o);

        // Lose conditions.
        let raid = self.raid.as_mut().unwrap();
        let ticks = self.tick;
        if raid.pending_hit > 0.0 && ticks % 30 == 0 {
            let d = raid.pending_hit;
            raid.pending_hit = 0.0;
            let h = raid.health;
            self.events.push(Event::OutpostHit { damage: d, health: h });
        }
        let raid = self.raid.as_ref().unwrap();
        let mut reason = None;
        if raid.health <= 0.0 {
            reason = Some(LossReason::OutpostFell);
        } else {
            // Every defender (humans and bot allies) down at the same moment.
            let core: Vec<&Player> = self.players.iter().filter(|p| p.team == Some(TEAM_DEFENDERS)).collect();
            if !core.is_empty() && core.iter().all(|p| !p.tank.alive) && raid.wave > 0 {
                reason = Some(LossReason::AllDefendersDown);
            }
        }
        if let Some(reason) = reason {
            let raid = self.raid.as_mut().unwrap();
            raid.lost = Some(reason);
            raid.health = raid.health.max(0.0);
            raid.queue.clear();
            let waves = raid.waves_survived as u16;
            self.phase = Phase::Defeated { waves };
            self.events.push(Event::RaidLost { waves_survived: waves, reason });
        }
    }

    pub(crate) fn hit_outpost(&mut self, amount: f32, _pos: Vec3) {
        if amount <= 0.0 {
            return;
        }
        if let Some(r) = self.raid.as_mut() {
            if r.lost.is_none() {
                r.health -= amount;
                r.pending_hit += amount;
            }
        }
    }

    /// Raider shells landing on the outpost hurt it.
    pub(crate) fn raid_explosion(&mut self, pos: Vec3, owner: Option<PlayerId>, splash: f32, radius: f32) {
        let Some(r) = self.raid.as_ref() else { return };
        let Some(o) = owner else { return };
        if self.team_of(o) != Some(TEAM_RAIDERS) {
            return;
        }
        let d = Vec2::new(pos.x - r.outpost.x, pos.z - r.outpost.z).length();
        // The whole compound counts: shells on the walls hurt the outpost too.
        if d < r.radius + 24.0 + radius {
            let mult = self.tuning.raid.outpost_splash_mult;
            self.hit_outpost(splash * mult, pos);
        }
    }

    /// Boss core: hull hits barely scratch it while any plate stands.
    pub(crate) fn core_mult(&self, j: usize, loc: HitLocation) -> f32 {
        let tank = &self.players[j].tank;
        let Some(boss) = BossKind::of(tank.vehicle) else { return 1.0 };
        if matches!(loc, HitLocation::Part(_)) {
            return 1.0;
        }
        if tank.parts.iter().any(|p| boss.plates().contains(&p.kind)) {
            self.tuning.raid.boss_core_shielded
        } else {
            1.0
        }
    }

    /// After a part breaks off: boss module events.
    pub(crate) fn on_part_lost(&mut self, j: usize, kind: PartKind, pos: Vec3) {
        let Some(boss) = BossKind::of(self.players[j].tank.vehicle) else { return };
        if !boss.modules().contains(&kind) {
            return;
        }
        let id = self.players[j].id;
        self.events.push(Event::BossModuleLost { player: id, part: kind, pos });
        let plates = boss.plates();
        if plates.contains(&kind) && !self.players[j].tank.parts.iter().any(|p| plates.contains(&p.kind)) {
            self.events.push(Event::BossCoreExposed { player: id });
        }
    }

    /// On any death in a raid: raider drops, boss bookkeeping, defender respawn timer.
    pub(crate) fn on_raid_death(&mut self, j: usize, killer: Option<PlayerId>, pos: Vec3, vel: Vec3) {
        if self.raid.is_none() {
            return;
        }
        let t = self.tuning.raid.clone();
        let id = self.players[j].id;
        let vehicle = self.players[j].tank.vehicle;
        if self.players[j].team == Some(TEAM_DEFENDERS) {
            self.players[j].tank.respawn_in = t.defender_respawn;
            return;
        }
        self.players[j].tank.respawn_in = f32::INFINITY;
        let drops = match vehicle {
            VehicleKind::Buggy => t.drop_buggy,
            VehicleKind::Technical => t.drop_technical,
            VehicleKind::Tank => t.drop_tank,
            VehicleKind::WarRig => t.drop_boss,
            VehicleKind::LandShip => t.drop_ship,
            VehicleKind::FuelTanker => 0,
        };
        // A raider killed by its own kamikaze still drops its scrap.
        for _ in 0..drops {
            let k = ScrapKind::from_index(self.rng.below(4));
            self.spill(PickupKind::Scrap(k), pos + Vec3::Y, vel);
        }
        if let Some(kind) = BossKind::of(vehicle) {
            let rares = if kind == BossKind::LandShip { 2 } else { 1 };
            for _ in 0..rares {
                let r = crate::parts::RARE_PARTS[self.rng.below(crate::parts::RARE_PARTS.len())];
                self.spill(PickupKind::Rare(r), pos + Vec3::Y * 2.0, vel);
            }
            self.spill(PickupKind::AirStrike, pos + Vec3::Y * 2.0, vel);
            let now = self.tick as f32 / 60.0;
            let raid = self.raid.as_mut().unwrap();
            let mut lived = 0.0;
            if let Some(k) = raid.bosses.iter().position(|b| b.0 == id) {
                lived = now - raid.bosses[k].1;
                raid.bosses.remove(k);
            }
            raid.bosses_killed += 1;
            raid.boss_lifetimes.push(lived);
            raid.boss_defeat_log.push((kind, lived));
            raid.ships.retain(|s| s.id != id);
            self.events.push(Event::BossDefeated { player: id, killer, seconds: lived });
        }
    }

    /// Dead raiders leave the match (their wrecks stay as props).
    pub(crate) fn cull_raiders(&mut self) {
        if self.raid.is_none() {
            return;
        }
        let mut k = 0;
        while k < self.players.len() {
            if self.players[k].team == Some(TEAM_RAIDERS) && !self.players[k].tank.alive {
                let p = self.players.remove(k);
                self.phys.world.remove_body(p.tank.body);
                let id = p.id;
                self.tethers.retain(|x| x.owner != id && x.target != crate::events::HookTarget::Tank(id));
                continue;
            }
            k += 1;
        }
    }

    /// The War Rig's front flamethrower (while the module stands).
    pub(crate) fn boss_weapons(&mut self, dt: f32) {
        if self.raid.is_none() {
            return;
        }
        self.ship_weapons(dt);
        let t = self.tuning.clone();
        for i in 0..self.players.len() {
            let p = &self.players[i];
            if p.tank.vehicle != VehicleKind::WarRig || !p.tank.alive {
                continue;
            }
            if !p.tank.parts.iter().any(|q| q.kind == PartKind::RigFlamer) {
                continue;
            }
            let id = p.id;
            let hp = HullPose::of(&self.phys, p.tank.body);
            let nozzle = hp.point(Vec3::new(8.6, 0.3, 0.0));
            let fwd = hp.rot * Vec3::X;
            let mut targets = Vec::new();
            for (j, q) in self.players.iter().enumerate() {
                if q.team == Some(TEAM_RAIDERS) || !q.tank.alive {
                    continue;
                }
                let c = self.phys.body(q.tank.body).translation();
                let v = c - nozzle;
                if v.length() < t.raid.rig_flame_range && v.angle_between(fwd) < 0.6 {
                    targets.push((j, c, v.normalize_or(fwd)));
                }
            }
            self.players[i].tank.flaming = !targets.is_empty();
            if targets.is_empty() || self.tick % 6 != 0 {
                continue;
            }
            for (j, c, dir) in targets {
                self.damage(j, Some(id), HitLocation::Hull, t.raid.rig_flame_dps * 0.1, c, dir, Weapon::Alt(crate::parts::AltKind::Flamethrower));
                if self.players[j].tank.alive {
                    if self.players[j].tank.burning <= 0.0 {
                        let pid = self.players[j].id;
                        self.events.push(Event::Ignited { player: pid, by: Some(id) });
                    }
                    self.players[j].tank.burning = t.special.burn_time;
                    self.players[j].tank.burn_attacker = Some(id);
                }
            }
            let _ = dt;
        }
    }

    pub(crate) fn raid_snap(&self) -> Option<RaidSnap> {
        let r = self.raid.as_ref()?;
        Some(RaidSnap {
            outpost: r.outpost.to_array(),
            radius: r.radius,
            health: r.health.max(0.0) as u16,
            max_health: r.max_health as u16,
            wave: r.wave as u16,
            phase: r.phase,
            ticks_left: if r.phase == RaidPhase::Breather { (r.timer.max(0.0) * 60.0) as u32 } else { 0 },
            raiders_alive: self.players.iter().filter(|p| p.team == Some(TEAM_RAIDERS) && p.tank.alive).count() as u16,
            raiders_queued: r.queue.len() as u16,
            waves_survived: r.waves_survived as u16,
        })
    }

    pub(crate) fn boss_snaps(&self) -> Vec<BossSnap> {
        self.players
            .iter()
            .filter(|p| p.tank.alive && p.team == Some(TEAM_RAIDERS))
            .filter_map(|p| BossKind::of(p.tank.vehicle).map(|kind| (p, kind)))
            .map(|(p, kind)| BossSnap {
                player: p.id,
                kind,
                health: p.tank.health.max(0.0) as u16,
                max_health: p.tank.stats.max_health as u16,
                core_exposed: !p.tank.parts.iter().any(|q| kind.plates().contains(&q.kind)),
                modules: kind
                    .modules()
                    .iter()
                    .map(|&k| match p.tank.parts.iter().find(|q| q.kind == k) {
                        Some(q) => ModuleSnap { kind: k, health: q01(q.health / q.max_health), alive: true },
                        None => ModuleSnap { kind: k, health: 0, alive: false },
                    })
                    .collect(),
                turrets: self.ship_turrets(p.id),
            })
            .collect()
    }

    /// Put a tank on the ground at `pos` (used by tests and the server's debug tools).
    pub fn teleport(&mut self, id: PlayerId, x: f32, z: f32, yaw: f32) {
        let Some(i) = self.idx(id) else { return };
        let y = self.map.height_at(x, z) + 1.1 * self.players[i].tank.hit_scale.y.max(1.0);
        let b = self.players[i].tank.body;
        let body = self.phys.body_mut(b);
        body.set_position(pose(Vec3::new(x, y, z), Quat::from_rotation_y(yaw)), true);
        body.set_linvel(Vec3::ZERO, true);
        body.set_angvel(Vec3::ZERO, true);
    }

    /// Spawn a raider of `kind` now (tests, debug). Starts wave 1 if none is running.
    pub fn spawn_raider_now(&mut self, kind: VehicleKind) -> Option<PlayerId> {
        let raid = self.raid.as_mut()?;
        if raid.wave == 0 {
            raid.wave = 1;
            raid.phase = RaidPhase::Fighting;
        }
        self.spawn_raider(kind);
        self.players.last().map(|p| p.id)
    }
}

impl Sim {
    /// Raiders that make no progress toward the outpost for a while (wedged on
    /// a mesa, lost in the yard) and aren't fighting go back to the edge: small
    /// fry are re-queued, the boss is moved to a fresh edge spot. Keeps waves
    /// from stalling forever.
    fn recycle_stuck_raiders(&mut self, dt: f32) {
        let Some(raid) = self.raid.as_ref() else { return };
        let o = raid.outpost;
        let rad = raid.radius;
        let mut prog = raid.progress.clone();
        prog.retain(|(id, _, _)| self.players.iter().any(|p| p.id == *id && p.tank.alive));
        let mut recycle = Vec::new();
        for p in &self.players {
            if p.team != Some(TEAM_RAIDERS) || !p.tank.alive {
                continue;
            }
            let c = self.phys.body(p.tank.body).translation();
            let d = Vec2::new(c.x - o.x, c.z - o.z).length();
            match prog.iter_mut().find(|e| e.0 == p.id) {
                Some(e) => {
                    if d < e.1 - 5.0 {
                        e.1 = d;
                        e.2 = 0.0;
                    } else {
                        e.2 += dt;
                    }
                    // The Land-Ship parks off the walls on purpose: that's not stuck.
                    let parked = p.tank.vehicle == VehicleKind::LandShip && d < self.tuning.raid.ship_standoff + 25.0;
                    if e.2 > 25.0 && p.tank.since_damage > 10.0 && d > rad + 5.0 && !parked {
                        recycle.push(p.id);
                        e.1 = f32::MAX;
                        e.2 = 0.0;
                    }
                }
                None => prog.push((p.id, d, 0.0)),
            }
        }
        self.raid.as_mut().unwrap().progress = prog;
        for id in recycle {
            let Some(j) = self.idx(id) else { continue };
            let kind = self.players[j].tank.vehicle;
            if matches!(kind, VehicleKind::WarRig | VehicleKind::LandShip) {
                let margin = if kind == VehicleKind::LandShip { 18.0 } else { 10.0 };
                for _ in 0..30 {
                    let a = self.rng.range(0.0, std::f32::consts::TAU);
                    let r = self.map.play_half - margin - 20.0;
                    let p = Vec2::new(a.cos() * r, a.sin() * r);
                    if !self.map.blocked_xz(p, margin) {
                        let d = Vec2::new(o.x, o.z) - p;
                        self.teleport(id, p.x, p.y, (-d.y).atan2(d.x));
                        let b = self.players[j].tank.body;
                        let pos = self.phys.body(b).translation() + Vec3::Y * 2.0;
                        let rot = *self.phys.body(b).rotation();
                        self.phys.body_mut(b).set_position(pose(pos, rot), true);
                        break;
                    }
                }
            } else {
                let p = self.players.remove(j);
                self.phys.world.remove_body(p.tank.body);
                self.raid.as_mut().unwrap().queue.push(kind);
            }
        }
    }
}

impl Sim {
    /// Debug: skip to wave `n` (its breather ends now). Living raiders leave.
    pub fn debug_jump_to_wave(&mut self, n: u32) {
        if self.raid.is_none() {
            return;
        }
        let mut k = 0;
        while k < self.players.len() {
            if self.players[k].team == Some(TEAM_RAIDERS) {
                let p = self.players.remove(k);
                self.phys.world.remove_body(p.tank.body);
            } else {
                k += 1;
            }
        }
        let r = self.raid.as_mut().unwrap();
        r.wave = n.saturating_sub(1);
        r.waves_survived = r.wave;
        r.queue.clear();
        r.bosses.clear();
        r.lost = None;
        r.phase = RaidPhase::Breather;
        r.timer = 0.0;
        self.phase = Phase::Playing;
    }

    /// Debug: spawn THE WAR RIG at the map edge now.
    pub fn debug_spawn_boss(&mut self) -> Option<PlayerId> {
        self.spawn_raider_now(VehicleKind::WarRig)
    }

    /// Debug: spawn THE LAND-SHIP at the map edge now.
    pub fn debug_spawn_landship(&mut self) -> Option<PlayerId> {
        self.spawn_raider_now(VehicleKind::LandShip)
    }

    /// Debug: spawn the boss of `kind` at the map edge now.
    pub fn debug_spawn_boss_kind(&mut self, kind: BossKind) -> Option<PlayerId> {
        self.spawn_raider_now(kind.vehicle())
    }

}
