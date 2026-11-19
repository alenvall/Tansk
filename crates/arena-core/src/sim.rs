//! The match: players, tanks, weapons, pickups, props, Scrap Brawl rules.

use std::sync::Arc;

use glam::{Quat, Vec2, Vec3};
use rapier3d::prelude::*;
use serde::{Deserialize, Serialize};

use crate::bots::{self, Brain, NavGrid};
use crate::events::{Event, HitLocation, PickupKind, RepairKind, Weapon};
use crate::map::{Map, PropKindSpawn, ScrapKind};
use crate::parts::{branch_options, AltKind, Frame, GunKind, PartKind, Track};
use crate::physics::{debris_groups, pose, prop_groups, shot_query, Physics};
use crate::rng::Rng;
use crate::snapshot::*;
use crate::tank::{gun_stats, HullPose, Repair, Tank, TrackState, WheelState};
use crate::tuning::Tuning;
use crate::{Input, PlayerId, DT};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PropKind {
    OilDrum,
    /// A part that was shot off, tumbling.
    Debris(PartKind),
    /// What's left of a destroyed tank.
    Wreck,
    /// A chunk of destroyed cover (barrier, sandbags, car, crates, pipe, silo, fuel tank).
    Rubble(crate::map::ObstacleKind),
    /// A shipping container knocked loose from its stack (12.2 x 2.6 x 2.44 m).
    Container,
    /// The crane's wrecking ball (radius `WorldTuning::crane_ball_radius`); its rope
    /// runs up to `Map::cranes[..].anchor`.
    WreckingBall,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Playing,
    Ended { winner: Option<PlayerId> },
    /// Raider Waves lost (the outpost fell or every defender was down at once).
    Defeated { waves: u16 },
    /// Team Brawl over: the winning team (None on a draw).
    TeamWon { team: Option<u8> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub id: PlayerId,
    pub name: String,
    pub is_bot: bool,
    /// None in free-for-all; Some(0) defenders, Some(1) raiders.
    pub team: Option<u8>,
    /// Garage pick (chassis, paint, emblem, horn).
    pub loadout: crate::garage::Loadout,
}

#[derive(Clone)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub input: Input,
    pub tank: Tank,
    pub kills: u32,
    pub deaths: u32,
    pub brain: Option<Brain>,
    pub ram_cd: f32,
    /// None in free-for-all; Some(0) defenders, Some(1) raiders in Raider Waves.
    pub team: Option<u8>,
    /// A human or a harness stand-in for one (statistics only: Raider Waves is lost when
    /// every defender, bots included, is down at once).
    pub counts_as_human: bool,
    /// Garage pick: chassis class and cosmetics.
    pub loadout: crate::garage::Loadout,
}

#[derive(Clone, Debug)]
pub struct Projectile {
    pub id: u16,
    pub owner: PlayerId,
    pub gun: GunKind,
    pub pos: Vec3,
    pub vel: Vec3,
    pub age: f32,
    pub travelled: f32,
    pub shot: ShotKind,
    /// Rockets: who they home on.
    pub target: Option<PlayerId>,
}

#[derive(Clone, Debug)]
pub struct Pickup {
    pub id: u16,
    pub kind: PickupKind,
    pub pos: Vec3,
    pub vel: Vec3,
    pub age: f32,
    pub ttl: Option<f32>,
    pub site: Option<usize>,
    pub settled: bool,
}

#[derive(Clone, Debug)]
pub struct Site {
    pub pos: Vec3,
    pub kind: PickupKind,
    pub timer: f32,
    pub live: Option<u16>,
}

#[derive(Clone, Debug)]
pub struct Prop {
    pub id: u16,
    pub kind: PropKind,
    pub body: RigidBodyHandle,
    pub ttl: Option<f32>,
    /// Oil drums blow up when this runs out.
    pub health: Option<f32>,
}

/// What a trace hit.
#[derive(Clone, Copy, Debug)]
pub struct TraceHit {
    pub dist: f32,
    pub pos: Vec3,
    pub normal: Vec3,
    /// (player index, location).
    pub tank: Option<(usize, HitLocation)>,
    pub collider: Option<ColliderHandle>,
}

#[derive(Clone)]
pub struct Sim {
    pub tick: u32,
    pub tuning: Tuning,
    pub map: Arc<Map>,
    pub phys: Physics,
    pub players: Vec<Player>,
    pub projectiles: Vec<Projectile>,
    pub pickups: Vec<Pickup>,
    pub sites: Vec<Site>,
    pub props: Vec<Prop>,
    pub phase: Phase,
    pub time_left: f32,
    /// Ticks played in the current match.
    pub match_ticks: u32,
    pub nav: Arc<NavGrid>,
    pub rng: Rng,
    pub(crate) events: Vec<Event>,
    pub(crate) next_id: u16,
    // --- Wave 2.
    pub smokes: Vec<crate::specials::Smoke>,
    pub mines: Vec<crate::specials::Mine>,
    pub strikes: Vec<crate::specials::Strike>,
    pub tethers: Vec<crate::specials::Tether>,
    pub hazards: Vec<crate::specials::Hazard>,
    /// Health of each destructible obstacle (None = indestructible).
    pub obstacle_hp: Vec<Option<f32>>,
    /// Removed obstacles (box 255 = all of it) and loose container boxes.
    pub removed: Vec<RemovedSnap>,
    // --- Wave 3.
    pub mode: crate::raid::ModeKind,
    pub raid: Option<crate::raid::RaidState>,
    pub(crate) pending_team: Option<u8>,
    /// The sandstorm (brawl modes).
    pub storm: crate::storm::StormState,
    // --- Wave 7.
    /// Convoy: the route, the truck and the rounds (None in other modes).
    pub convoy: Option<crate::convoy::ConvoyState>,
    // --- Wave 8.
    /// Wellhead: the derricks and the fuel tanks (None in other modes).
    pub wellhead: Option<crate::wellhead::WellheadState>,
    /// Last Crew Rolling: the sandstorm wall and who's out (None in other modes).
    pub last_crew: Option<crate::lastcrew::LastCrewState>,
}

fn yaw_of(rot: Quat) -> f32 {
    let f = rot * Vec3::X;
    (-f.z).atan2(f.x)
}

impl Sim {
    pub fn new(seed: u64, map: Map, tuning: Tuning) -> Sim {
        Sim::with_mode(seed, map, tuning, crate::raid::ModeKind::ScrapBrawl)
    }

    /// A match of the given mode. `ModeKind::RaiderWaves` = `Sim::new_raid`.
    pub fn with_mode(seed: u64, map: Map, tuning: Tuning, mode: crate::raid::ModeKind) -> Sim {
        let phys = Physics::new(&map);
        let nav = NavGrid::build(&map, tuning.bots.nav_cell);
        let mut sim = Sim {
            tick: 0,
            time_left: tuning.mode.time_limit,
            match_ticks: 0,
            tuning,
            phys,
            players: Vec::new(),
            projectiles: Vec::new(),
            pickups: Vec::new(),
            sites: Vec::new(),
            props: Vec::new(),
            phase: Phase::Playing,
            nav: Arc::new(nav),
            rng: Rng::new(seed),
            events: Vec::new(),
            next_id: 1,
            map: Arc::new(map),
            smokes: Vec::new(),
            mines: Vec::new(),
            strikes: Vec::new(),
            tethers: Vec::new(),
            hazards: Vec::new(),
            obstacle_hp: Vec::new(),
            removed: Vec::new(),
            mode,
            raid: None,
            pending_team: None,
            storm: crate::storm::StormState::default(),
            convoy: None,
            wellhead: None,
            last_crew: None,
        };
        for s in sim.map.scrap_sites.clone() {
            sim.sites.push(Site { pos: s.pos, kind: PickupKind::Scrap(s.kind), timer: 0.0, live: None });
        }
        for p in sim.map.repair_sites.clone() {
            sim.sites.push(Site { pos: p, kind: PickupKind::Repair, timer: 0.0, live: None });
        }
        for ps in sim.map.props.clone() {
            match ps.kind {
                PropKindSpawn::OilDrum => {
                    let body = sim.phys.world.insert_body(RigidBodyBuilder::dynamic().translation(ps.pos));
                    sim.phys.world.insert_collider(
                        ColliderBuilder::cylinder(0.45, 0.45).mass(200.0).collision_groups(prop_groups()),
                        Some(body),
                    );
                    let id = sim.alloc_id();
                    let hp = sim.tuning.world.oil_drum_health;
                    sim.props.push(Prop { id, kind: PropKind::OilDrum, body, ttl: None, health: Some(hp) });
                }
            }
        }
        sim.init_world();
        if mode == crate::raid::ModeKind::RaiderWaves {
            sim.init_raid();
        } else {
            sim.schedule_storm();
        }
        if mode == crate::raid::ModeKind::Convoy {
            sim.init_convoy();
        }
        if mode == crate::raid::ModeKind::Wellhead {
            sim.init_wellhead();
        }
        if mode == crate::raid::ModeKind::LastCrew {
            sim.init_last_crew();
        }
        sim.update_sites(0.0);
        sim
    }

    /// Practice on The Sink with `bots` bots (difficulty 0..1) and one human; returns (sim, human id).
    pub fn practice(seed: u64, bots: usize, difficulty: f32) -> (Sim, PlayerId) {
        Sim::practice_on(crate::map::MapId::Sink, seed, bots, difficulty)
    }

    /// Practice on map `id`; see [`Sim::practice`].
    pub fn practice_on(id: crate::map::MapId, seed: u64, bots: usize, difficulty: f32) -> (Sim, PlayerId) {
        let mut sim = Sim::new(seed, Map::by_id(id, seed), Tuning::default());
        let me = sim.add_player("You");
        for i in 0..bots {
            sim.add_bot(bots::BOT_NAMES[i % bots::BOT_NAMES.len()], difficulty);
        }
        (sim, me)
    }

    pub(crate) fn alloc_id(&mut self) -> u16 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        id
    }

    pub(crate) fn idx(&self, id: PlayerId) -> Option<usize> {
        self.players.iter().position(|p| p.id == id)
    }

    pub fn add_player(&mut self, name: &str) -> PlayerId {
        self.add(name, None, crate::garage::Loadout::default())
    }

    /// A human with their Garage pick (chassis class and cosmetics).
    pub fn add_player_with(&mut self, name: &str, loadout: crate::garage::Loadout) -> PlayerId {
        self.add(name, None, loadout)
    }

    /// Change a player's Garage pick; the chassis applies at their next respawn.
    pub fn set_loadout(&mut self, id: PlayerId, loadout: crate::garage::Loadout) {
        if let Some(i) = self.idx(id) {
            self.players[i].loadout = loadout;
        }
    }

    /// A bot crew; `difficulty` 0 (sloppy) ..= 1 (sharp).
    pub fn add_bot(&mut self, name: &str, difficulty: f32) -> PlayerId {
        let seed = self.rng.next_u64();
        let brain = Brain::new(seed, difficulty);
        let loadout = crate::garage::Loadout {
            chassis: brain.style().chassis(),
            paint: (seed % 8) as u8,
            emblem: ((seed >> 8) % 8) as u8,
            horn: ((seed >> 16) % 4) as u8,
        };
        self.add(name, Some(brain), loadout)
    }

    fn add(&mut self, name: &str, brain: Option<Brain>, loadout: crate::garage::Loadout) -> PlayerId {
        let team = if self.raid.is_some() {
            Some(crate::raid::TEAM_DEFENDERS)
        } else if self.mode.is_team() {
            // Fill the smaller team (ties go to the Rustbacks). The Convoy truck isn't a crew.
            let crew = |p: &&Player| p.tank.vehicle != crate::parts::VehicleKind::FuelTanker;
            let n0 = self.players.iter().filter(crew).filter(|p| p.team == Some(0)).count();
            let n1 = self.players.iter().filter(crew).filter(|p| p.team == Some(1)).count();
            Some(if n1 < n0 { 1 } else { 0 })
        } else {
            None
        };
        self.pending_team = team;
        let counts_as_human = brain.is_none();
        let id = (0..=255u8).find(|i| self.players.iter().all(|p| p.id != *i)).expect("too many players");
        let (pos, yaw) = self.pick_spawn(None);
        let mut tank = Tank::new(&mut self.phys, &self.tuning, pos, yaw);
        let t = self.tuning.clone();
        tank.set_class(&mut self.phys, &t, loadout.chassis);
        self.players.push(Player {
            id,
            name: name.to_string(),
            input: Input::default(),
            tank,
            kills: 0,
            deaths: 0,
            brain,
            ram_cd: 0.0,
            team,
            counts_as_human,
            loadout,
        });
        self.events.push(Event::Respawned { player: id, pos });
        let j = self.players.len() - 1;
        self.last_crew_late_join(j);
        id
    }

    pub fn remove_player(&mut self, id: PlayerId) {
        if let Some(i) = self.idx(id) {
            let p = self.players.remove(i);
            self.phys.world.remove_body(p.tank.body);
        }
    }

    pub fn set_input(&mut self, id: PlayerId, input: Input) {
        if let Some(i) = self.idx(id) {
            self.players[i].input = input;
        }
    }

    pub fn players(&self) -> Vec<PlayerInfo> {
        self.players
            .iter()
            .map(|p| PlayerInfo {
                id: p.id,
                name: p.name.clone(),
                // The Convoy truck has no crew: list it as a bot.
                is_bot: p.brain.is_some() || p.tank.vehicle == crate::parts::VehicleKind::FuelTanker,
                team: p.team,
                loadout: p.loadout,
            })
            .collect()
    }

    pub fn tank(&self, id: PlayerId) -> Option<&Tank> {
        self.idx(id).map(|i| &self.players[i].tank)
    }

    pub fn tank_pose(&self, id: PlayerId) -> Option<(Vec3, Quat)> {
        self.tank(id).map(|t| {
            let hp = HullPose::of(&self.phys, t.body);
            (hp.pos, hp.rot)
        })
    }

    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Restart the match on the same map: scores, tiers and pickups reset.
    pub fn reset_match(&mut self) {
        self.phase = Phase::Playing;
        // Raider Waves: raiders leave, the raid starts over from wave 0.
        if self.raid.is_some() {
            let mut k = 0;
            while k < self.players.len() {
                if self.players[k].team == Some(crate::raid::TEAM_RAIDERS) {
                    let p = self.players.remove(k);
                    self.phys.world.remove_body(p.tank.body);
                } else {
                    k += 1;
                }
            }
            self.init_raid();
        }
        self.time_left = self.tuning.mode.time_limit;
        self.match_ticks = 0;
        // Convoy: back to round 1, the truck to the start (respawned below with everyone).
        if self.convoy.is_some() {
            self.reset_convoy();
        }
        self.reset_wellhead();
        self.reset_last_crew();
        let ids: Vec<PlayerId> = self.players.iter().map(|p| p.id).collect();
        for id in ids {
            let i = self.idx(id).unwrap();
            let p = &mut self.players[i];
            p.kills = 0;
            p.deaths = 0;
            p.tank.tiers = [0; 4];
            p.tank.scrap = [0; 4];
            p.tank.branch = [None; 4];
            p.tank.choice = None;
            for k in ScrapKind::ALL {
                self.players[i].tank.rebuild_track(&mut self.phys, &self.tuning, k);
            }
            self.respawn(i);
        }
        for p in std::mem::take(&mut self.pickups) {
            let _ = p;
        }
        for s in &mut self.sites {
            s.live = None;
            s.timer = 0.0;
        }
        self.projectiles.clear();
        self.reset_world();
        if self.raid.is_none() {
            self.schedule_storm();
        }
        self.update_sites(0.0);
    }

    // ----------------------------------------------------------------- step

    pub fn step(&mut self) {
        let dt = DT;
        self.tick += 1;
        let t = self.tuning.clone();

        // Bots decide.
        for i in 0..self.players.len() {
            if let Some(mut brain) = self.players[i].brain.take() {
                let input = bots::think(self, i, &mut brain, dt);
                self.players[i].input = input;
                self.players[i].brain = Some(brain);
            }
        }
        // Machines the sim drives on top of (or instead of) a brain.
        self.ship_inputs();
        self.convoy_drive(dt);

        let mut pre_vel = Vec::with_capacity(self.players.len());
        for i in 0..self.players.len() {
            self.tick_player(i, &t, dt);
            let b = self.phys.body(self.players[i].tank.body);
            pre_vel.push(b.linvel());
        }

        self.phys.step();

        self.step_projectiles(&t, dt);
        self.check_rams(&t, &pre_vel, dt);
        self.step_pickups(&t, dt);
        self.update_sites(dt);
        self.step_props(dt);
        self.boss_weapons(dt);
        self.step_world(dt);
        self.safety_net();
        self.step_raid(dt);
        self.step_convoy(dt);
        self.step_storm(dt);
        self.step_mode(dt);
        self.step_wellhead(dt);
        self.step_last_crew(dt);
        self.cull_raiders();
    }

    fn tick_player(&mut self, i: usize, t: &Tuning, dt: f32) {
        let id = self.players[i].id;
        self.players[i].ram_cd = (self.players[i].ram_cd - dt).max(0.0);
        if self.raid.is_some() && !self.players[i].tank.alive && self.players[i].team == Some(crate::raid::TEAM_RAIDERS) {
            return;
        }
        if !self.players[i].tank.alive {
            self.players[i].tank.respawn_in -= dt;
            if self.players[i].tank.respawn_in <= 0.0 {
                self.respawn(i);
            }
            return;
        }
        let input = self.players[i].input;
        let ended = matches!(self.phase, Phase::Ended { .. });
        let input = if ended { Input { aim_point: input.aim_point, ..Input::default() } } else { input };

        // Timers.
        {
            let tank = &mut self.players[i].tank;
            tank.reload = (tank.reload - dt).max(0.0);
            tank.alt_cooldown = (tank.alt_cooldown - dt).max(0.0);
            tank.protect = (tank.protect - dt).max(0.0);
        }
        self.tick_tracks(i, t, dt);
        self.tick_repair(i, dt);
        self.tick_choice(i, &input, t, dt);
        self.tick_special_passive(i, dt);
        if input.horn && !self.players[i].tank.prev_horn {
            let horn = self.players[i].loadout.horn;
            self.events.push(Event::Horn { player: id, horn });
        }
        self.players[i].tank.prev_horn = input.horn;

        // Drive and aim.
        let report = {
            let p = &mut self.players[i];
            p.tank.drive(&mut self.phys, t, &input, dt)
        };
        if report.boost_started {
            self.events.push(Event::BoostStarted { player: id });
        }
        if report.righted {
            self.events.push(Event::Righted { player: id });
        }
        let hp = HullPose::of(&self.phys, self.players[i].tank.body);
        let turned = self.players[i].tank.aim(&hp, t, input.aim_point, dt);

        // Bloom: moving, turning the hull and slewing the turret open it up; it
        // settles over ~bloom_settle seconds once still.
        {
            let tank = &mut self.players[i].tank;
            let target = (report.speed / t.tank.max_speed
                + report.yaw_rate.abs() / t.tank.yaw_rate_in_place * 0.6
                + turned / (t.turret.yaw_rate * dt) * 0.25)
                .clamp(0.0, 1.0);
            if target > tank.bloom {
                tank.bloom += (target - tank.bloom) * (dt / t.guns.bloom_rise_tau).min(1.0);
            } else {
                let fall = dt / (t.guns.bloom_settle / 3.0);
                tank.bloom += (target - tank.bloom) * fall.min(1.0);
            }
        }

        if ended {
            return;
        }
        if input.fire && self.players[i].tank.reload <= 0.0 {
            self.fire_gun(i, t);
        }
        self.alt_weapon(i, &input, t, dt);
        self.special_input(i, &input, dt);
        self.players[i].tank.prev_alt = input.alt_fire;
        self.players[i].tank.prev_special = input.special;
    }

    fn tick_tracks(&mut self, i: usize, t: &Tuning, dt: f32) {
        let id = self.players[i].id;
        for side in 0..2u8 {
            let tank = &mut self.players[i].tank;
            let s = side as usize;
            if tank.tracks[s].blown <= 0.0 {
                continue;
            }
            let before = tank.tracks[s].blown;
            tank.tracks[s].blown -= dt;
            let patch = t.pickups.track_patch_duration;
            if before > patch && tank.tracks[s].blown <= patch && tank.repair.is_none() {
                tank.repair = Some(Repair { kind: RepairKind::Track(side), elapsed: 0.0, duration: patch, heal_rate: 0.0 });
                self.events.push(Event::RepairStarted { player: id, kind: RepairKind::Track(side) });
            }
            if tank.tracks[s].blown <= 0.0 {
                tank.tracks[s] = TrackState { health: t.damage.track_health, blown: 0.0 };
                self.events.push(Event::TrackRepaired { player: id, side });
            }
        }
    }

    fn tick_repair(&mut self, i: usize, dt: f32) {
        let id = self.players[i].id;
        let tank = &mut self.players[i].tank;
        let Some(mut r) = tank.repair else { return };
        r.elapsed += dt;
        if r.kind == RepairKind::Kit {
            tank.health = (tank.health + r.heal_rate * dt).min(tank.stats.max_health);
            for p in &mut tank.parts {
                if p.max_health.is_finite() {
                    p.health = (p.health + p.max_health * 0.5 / r.duration * dt).min(p.max_health);
                }
            }
        }
        if r.elapsed >= r.duration {
            if r.kind == RepairKind::Kit {
                for s in &mut tank.tracks {
                    s.blown = s.blown.min(0.01);
                }
            }
            tank.repair = None;
            self.events.push(Event::RepairFinished { player: id, kind: r.kind });
        } else {
            tank.repair = Some(r);
        }
    }

    fn tick_choice(&mut self, i: usize, input: &Input, t: &Tuning, dt: f32) {
        let id = self.players[i].id;
        let Some(mut c) = self.players[i].tank.choice.clone() else { return };
        if let Some(pick) = input.choice {
            if let Some(&kind) = c.options.get(pick as usize) {
                let tank = &mut self.players[i].tank;
                tank.branch[c.track.index()] = Some(kind);
                tank.choice = None;
                tank.rebuild_track(&mut self.phys, t, c.track);
                self.events.push(Event::BranchChosen { player: id, track: c.track, part: kind });
                return;
            }
        }
        c.time_left -= dt;
        self.players[i].tank.choice = if c.time_left <= 0.0 { None } else { Some(c) };
    }

    // ------------------------------------------------------------- weapons

    fn fire_gun(&mut self, i: usize, t: &Tuning) {
        let id = self.players[i].id;
        let hp = HullPose::of(&self.phys, self.players[i].tank.body);
        let g = gun_stats(t, &self.players[i].tank.stats);
        let gun = self.players[i].tank.stats.gun;
        let dir0 = self.players[i].tank.barrel_dir(&hp, t);
        let spread = (g.spread_deg + self.players[i].tank.bloom * t.guns.bloom_max_deg + self.storm.intensity * t.storm.spread_deg).to_radians();
        let twin = gun == GunKind::TwinBarrel;
        let mut muzzle_avg = Vec3::ZERO;
        for k in 0..g.shells_per_shot.max(1) {
            let off = if twin { if k % 2 == 0 { -0.28 } else { 0.28 } } else { 0.0 };
            let muzzle = self.players[i].tank.muzzle(&hp, t, off);
            muzzle_avg += muzzle;
            let dir = self.rng.cone(dir0, spread);
            let pid = self.alloc_id();
            self.projectiles.push(Projectile {
                id: pid,
                owner: id,
                gun,
                pos: muzzle,
                vel: dir * g.shell_speed,
                age: 0.0,
                travelled: 0.0,
                shot: ShotKind::Shell,
                target: None,
            });
        }
        muzzle_avg /= g.shells_per_shot.max(1) as f32;
        {
            let tank = &mut self.players[i].tank;
            tank.reload = g.reload;
            tank.protect = 0.0;
            tank.bloom = (tank.bloom + t.guns.bloom_per_shot).min(1.0);
            tank.twin_side = !tank.twin_side;
        }
        let pivot = self.players[i].tank.frame(&hp, t, Frame::Gun).0;
        self.phys.body_mut(self.players[i].tank.body).apply_impulse_at_point(-dir0 * g.recoil, pivot, true);
        self.events.push(Event::Fired { player: id, weapon: Weapon::Gun(gun), pos: muzzle_avg, dir: dir0 });
    }

    /// Hitscan direction: toward the aim point if it's within the cone around
    /// the barrel's heading, else along the barrel.
    pub(crate) fn hitscan_dir(&self, i: usize, t: &Tuning, origin: Vec3) -> Vec3 {
        let hp = HullPose::of(&self.phys, self.players[i].tank.body);
        let barrel = self.players[i].tank.barrel_dir(&hp, t);
        let to_aim = (self.players[i].input.aim_point - origin).normalize_or(barrel);
        let flat_b = Vec2::new(barrel.x, barrel.z).normalize_or_zero();
        let flat_a = Vec2::new(to_aim.x, to_aim.z).normalize_or_zero();
        if flat_b.dot(flat_a) > t.alt.hitscan_cone_deg.to_radians().cos() {
            to_aim
        } else {
            barrel
        }
    }

    fn alt_weapon(&mut self, i: usize, input: &Input, t: &Tuning, dt: f32) {
        let id = self.players[i].id;
        let hp = HullPose::of(&self.phys, self.players[i].tank.body);
        let kind = self.players[i].tank.stats.alt;
        let mk = if self.players[i].tank.stats.alt_mk >= 2 { t.alt.mk2_damage } else { 1.0 };
        match kind {
            AltKind::Flamethrower => self.flamethrower(i, input, dt),
            AltKind::Rockets => self.rocket_rack(i, input),
            AltKind::Mg => {
                if !input.alt_fire || self.players[i].tank.alt_cooldown > 0.0 {
                    return;
                }
                self.players[i].tank.alt_cooldown = 1.0 / t.alt.mg_rate;
                let (o, r) = self.players[i].tank.frame(&hp, t, Frame::Gun);
                let origin = o + r * Vec3::new(1.2, 0.15, 0.3);
                let dir = self.hitscan_dir(i, t, origin);
                let spread = (t.alt.mg_spread_deg + self.players[i].tank.bloom * t.guns.bloom_max_deg * 0.5).to_radians();
                let dir = self.rng.cone(dir, spread);
                let hit = self.trace(origin, dir, t.alt.mg_range, Some(i));
                let end = hit.map(|h| h.pos).unwrap_or(origin + dir * t.alt.mg_range);
                let w = Weapon::Alt(AltKind::Mg);
                self.events.push(Event::Tracer { player: id, weapon: w, from: origin, to: end });
                if let Some(h) = hit {
                    if let Some((vi, loc)) = h.tank {
                        self.damage(vi, Some(id), loc, t.alt.mg_damage, h.pos, dir, w);
                    } else {
                        self.events.push(Event::Impact { pos: h.pos, normal: h.normal, weapon: w });
                        self.hit_collider(h.collider, t.alt.mg_damage, 0.0, h.pos, dir, Some(id));
                    }
                }
                self.players[i].tank.protect = 0.0;
            }
            AltKind::Coilgun => {
                let tank = &mut self.players[i].tank;
                if input.alt_fire && tank.alt_cooldown <= 0.0 {
                    if tank.charge == 0.0 {
                        self.events.push(Event::CoilCharging { player: id });
                    }
                    tank.charge = (tank.charge + dt / t.alt.coil_charge_time).min(1.0);
                    return;
                }
                if input.alt_fire || tank.charge <= 0.0 {
                    return;
                }
                let charge = tank.charge;
                tank.charge = 0.0;
                if charge < t.alt.coil_min_charge {
                    return;
                }
                tank.alt_cooldown = t.alt.coil_cooldown;
                tank.protect = 0.0;
                let (o, r) = tank.frame(&hp, t, Frame::Turret);
                let origin = o + r * Vec3::new(1.8, 0.55, 1.15);
                let dir = self.hitscan_dir(i, t, origin);
                let w = Weapon::Alt(AltKind::Coilgun);
                // Pierce: stops at the static world, passes through tanks.
                let stop = self
                    .phys
                    .ray(origin, dir, t.alt.coil_range, shot_query(), None)
                    .map(|(_, d, _)| d)
                    .unwrap_or(t.alt.coil_range);
                let mut hits: Vec<(f32, usize, HitLocation)> = Vec::new();
                for (j, p) in self.players.iter().enumerate() {
                    if j == i || !p.tank.alive {
                        continue;
                    }
                    let php = HullPose::of(&self.phys, p.tank.body);
                    if let Some((d, loc, _)) = p.tank.ray_test(&php, t, origin, dir, stop) {
                        hits.push((d, j, loc));
                    }
                }
                hits.sort_by(|a, b| a.0.total_cmp(&b.0));
                hits.truncate(t.alt.coil_pierce as usize);
                self.events.push(Event::Fired { player: id, weapon: w, pos: origin, dir });
                self.events.push(Event::Tracer { player: id, weapon: w, from: origin, to: origin + dir * stop });
                for (d, j, loc) in hits {
                    let pos = origin + dir * d;
                    self.damage(j, Some(id), loc, t.alt.coil_damage * charge * mk, pos, dir, w);
                    if self.players[j].tank.alive {
                        let b = self.players[j].tank.body;
                        self.phys.body_mut(b).apply_impulse_at_point(dir * t.alt.coil_knockback * charge, pos, true);
                    }
                }
            }
            AltKind::Tesla => {
                if !input.alt_fire || self.players[i].tank.alt_cooldown > 0.0 {
                    return;
                }
                let from = hp.point(Vec3::new(-2.4, 2.4, 1.1));
                let mut hit: Vec<usize> = Vec::new();
                let mut cur = from;
                let mut range = t.alt.tesla_range;
                let mut dmg = t.alt.tesla_damage * mk;
                let mut arcs = Vec::new();
                for _ in 0..t.alt.tesla_chain_count.max(1) {
                    let mut best: Option<(f32, usize, Vec3)> = None;
                    for (j, p) in self.players.iter().enumerate() {
                        if j == i || !p.tank.alive || hit.contains(&j) {
                            continue;
                        }
                        let c = self.phys.body(p.tank.body).translation() + Vec3::Y * 0.5;
                        let d = c.distance(cur);
                        if d < range && best.is_none_or(|b| d < b.0) && self.phys.clear_line(cur, c) {
                            best = Some((d, j, c));
                        }
                    }
                    let Some((_, j, c)) = best else { break };
                    hit.push(j);
                    arcs.push((self.players[j].id, c));
                    let dir = (c - cur).normalize_or(Vec3::X);
                    self.damage(j, Some(id), HitLocation::Hull, dmg, c, dir, Weapon::Alt(AltKind::Tesla));
                    cur = c;
                    range = t.alt.tesla_chain_range;
                    dmg *= t.alt.tesla_chain_falloff;
                }
                if !arcs.is_empty() {
                    let tank = &mut self.players[i].tank;
                    tank.alt_cooldown = t.alt.tesla_cooldown;
                    tank.protect = 0.0;
                    self.events.push(Event::Fired { player: id, weapon: Weapon::Alt(AltKind::Tesla), pos: from, dir: Vec3::Y });
                    self.events.push(Event::TeslaArc { player: id, from, targets: arcs });
                }
            }
        }
    }

    /// First thing along a ray: static world / props via rapier, tanks via hit boxes.
    pub fn trace(&self, origin: Vec3, dir: Vec3, max: f32, ignore: Option<usize>) -> Option<TraceHit> {
        let mut best: Option<TraceHit> = self.phys.ray(origin, dir, max, shot_query(), None).map(|(h, d, n)| TraceHit {
            dist: d,
            pos: origin + dir * d,
            normal: n,
            tank: None,
            collider: Some(h),
        });
        let limit = best.map(|b| b.dist).unwrap_or(max);
        for (j, p) in self.players.iter().enumerate() {
            if Some(j) == ignore || !p.tank.alive {
                continue;
            }
            let hp = HullPose::of(&self.phys, p.tank.body);
            if let Some((d, loc, n)) = p.tank.ray_test(&hp, &self.tuning, origin, dir, limit) {
                if best.is_none_or(|b| d < b.dist) {
                    best = Some(TraceHit { dist: d, pos: origin + dir * d, normal: n, tank: Some((j, loc)), collider: None });
                }
            }
        }
        best
    }

    fn step_projectiles(&mut self, t: &Tuning, dt: f32) {
        let mut projectiles = std::mem::take(&mut self.projectiles);
        let mut keep = Vec::with_capacity(projectiles.len());
        for mut pr in projectiles.drain(..) {
            let owner = self.idx(pr.owner);
            if pr.shot != ShotKind::Shell {
                if let Some(p) = self.step_special_projectile(pr, dt) {
                    keep.push(p);
                }
                continue;
            }
            let g = match pr.gun {
                GunKind::Stub => &t.guns.stub,
                GunKind::HeavyCannon => &t.guns.heavy_cannon,
                GunKind::BigBore => &t.guns.big_bore,
                GunKind::TwinBarrel => &t.guns.twin_barrel,
                GunKind::LongGun => &t.guns.long_gun,
                GunKind::RigCannons => &t.raid.rig_cannon,
                GunKind::DeckGun => &t.raid.deck_gun,
            }
            .clone();
            // Owner's Mk II multiplier (if they still have it).
            let mk = owner
                .map(|o| {
                    let s = &self.players[o].tank.stats;
                    if s.gun == pr.gun && s.gun_mk >= 2 {
                        t.guns.mk2_damage
                    } else {
                        1.0
                    }
                })
                .unwrap_or(1.0);
            pr.age += dt;
            if pr.age > g.lifetime {
                continue;
            }
            pr.vel.y -= 9.81 * g.gravity_scale * dt;
            let seg = pr.vel * dt;
            let len = seg.length();
            let dir = seg / len.max(1e-6);
            let w = Weapon::Gun(pr.gun);
            if let Some(h) = self.trace(pr.pos, dir, len, owner) {
                let travelled = pr.travelled + h.dist;
                let fall = if travelled <= g.falloff_start {
                    1.0
                } else {
                    let f = ((travelled - g.falloff_start) / (g.falloff_end - g.falloff_start).max(1.0)).min(1.0);
                    1.0 - f * (1.0 - g.falloff_min)
                };
                let mut direct = None;
                if let Some((vi, loc)) = h.tank {
                    self.damage(vi, Some(pr.owner), loc, g.damage * fall * mk, h.pos, dir, w);
                    direct = Some(vi);
                } else {
                    self.hit_collider(h.collider, g.damage * fall * mk, g.knockback, h.pos, dir, Some(pr.owner));
                    self.events.push(Event::Impact { pos: h.pos, normal: h.normal, weapon: w });
                }
                self.explode(h.pos + h.normal * 0.2, Some(pr.owner), &g, mk, direct, w);
                continue;
            }
            pr.pos += seg;
            pr.travelled += len;
            if pr.pos.y < -100.0 {
                continue;
            }
            keep.push(pr);
        }
        self.projectiles = keep;
    }

    pub(crate) fn explode(&mut self, pos: Vec3, owner: Option<PlayerId>, g: &crate::tuning::GunStats, mk: f32, direct: Option<usize>, w: Weapon) {
        let t = self.tuning.clone();
        let r = g.splash_radius;
        if r <= 0.0 {
            return;
        }
        self.events.push(Event::Explosion { pos, radius: r, weapon: w });
        for j in 0..self.players.len() {
            if !self.players[j].tank.alive {
                continue;
            }
            let body = self.players[j].tank.body;
            let c = self.phys.body(body).translation();
            let d = (c.distance(pos) - 2.0).max(0.0);
            if d >= r {
                continue;
            }
            if !self.phys.clear_line(pos, c + Vec3::Y * 0.5) && d > 0.5 {
                continue;
            }
            let f = 1.0 - d / r;
            let is_self = owner == Some(self.players[j].id);
            let mut dmg = g.splash_damage * mk * f * self.players[j].tank.stats.splash_mult;
            if is_self {
                dmg *= t.damage.self_splash;
            }
            if direct == Some(j) {
                dmg *= 0.5;
            }
            let dir = (c - pos).normalize_or(Vec3::Y);
            // Parts near the blast take a share directly.
            let hp = HullPose::of(&self.phys, body);
            let centers = self.players[j].tank.location_centers(&hp, &t);
            for (loc, lc) in centers {
                if let HitLocation::Part(_) = loc {
                    let pd = lc.distance(pos);
                    if pd < r {
                        let pf = 1.0 - pd / r;
                        self.damage_part_only(j, owner, loc, g.splash_damage * mk * pf * t.damage.splash_part_fraction, lc, w);
                    }
                }
            }
            self.damage(j, owner, HitLocation::Hull, dmg, c - dir * 1.5, dir, w);
            if self.players[j].tank.alive {
                let imp = (dir + Vec3::Y * 0.5).normalize() * g.knockback * f;
                let at = c - dir * 1.0;
                self.phys.body_mut(body).apply_impulse_at_point(imp, at, true);
            }
        }
        for k in 0..self.props.len() {
            let b = self.props[k].body;
            let c = self.phys.body(b).translation();
            let d = c.distance(pos);
            if d < r * 1.3 {
                let f = 1.0 - d / (r * 1.3);
                let dir = (c - pos + Vec3::Y * 0.6).normalize_or(Vec3::Y);
                let m = self.phys.body(b).mass();
                let dv = (g.knockback * f / 6000.0).min(15.0);
                self.phys.body_mut(b).apply_impulse_at_point(dir * m * dv, c - dir * 0.2, true);
            }
        }
        self.explode_world(pos, r, g.splash_damage * mk, g.knockback, owner);
        self.raid_explosion(pos, owner, g.splash_damage * mk, r);
    }

    // ------------------------------------------------------------- damage

    pub(crate) fn facing_mult(&self, j: usize, dir: Vec3) -> f32 {
        let t = &self.tuning.damage;
        let hp = HullPose::of(&self.phys, self.players[j].tank.body);
        let l = hp.rot.inverse() * -dir;
        let v = Vec2::new(l.x, l.z).normalize_or_zero();
        if v.x > 0.6 {
            t.facing_front
        } else if v.x < -0.6 {
            t.facing_rear
        } else {
            t.facing_side
        }
    }

    pub(crate) fn can_damage(&self, j: usize) -> bool {
        self.players[j].tank.alive && self.players[j].tank.protect <= 0.0 && self.phase == Phase::Playing
    }

    fn damage_part_only(&mut self, j: usize, attacker: Option<PlayerId>, loc: HitLocation, amount: f32, pos: Vec3, w: Weapon) {
        if !self.can_damage(j) {
            return;
        }
        let HitLocation::Part(kind) = loc else { return };
        if self.players[j].tank.shield > 0.0 {
            return;
        }
        let Some(p) = self.players[j].tank.parts.iter_mut().find(|p| p.kind == kind) else { return };
        p.health -= amount;
        let broke = p.health <= 0.0;
        let target = self.players[j].id;
        self.events.push(Event::Hit { target, attacker, location: loc, pos, damage: amount, weapon: w });
        if broke {
            self.break_part(j, kind);
        }
    }

    /// Apply damage to a hit location. `dir` is the direction the shot travelled.
    #[allow(clippy::too_many_arguments)]
    pub fn damage(&mut self, j: usize, attacker: Option<PlayerId>, loc: HitLocation, amount: f32, pos: Vec3, dir: Vec3, w: Weapon) {
        if !self.can_damage(j) || amount <= 0.0 {
            return;
        }
        let loc = self.ship_location(j, loc);
        let amount = amount * self.truck_armour(j, w);
        let amount = if self.last_crew.is_some() { amount * self.tuning.last_crew.damage_taken } else { amount };

        let Some(amount) = self.absorb(j, amount, pos, w) else { return };
        let t = self.tuning.clone();
        let target = self.players[j].id;
        if let Some(a) = attacker {
            if a != target && self.same_team(a, target) {
                return;
            }
        }
        if attacker.is_some() && attacker != Some(target) {
            self.players[j].tank.last_attacker = attacker;
        }
        let facing = self.facing_mult(j, dir);
        let hull;
        let mut broke = None;
        match loc {
            HitLocation::Hull | HitLocation::Turret => hull = amount * facing,
            HitLocation::Track(side) => {
                let tank = &mut self.players[j].tank;
                let tr = &mut tank.tracks[side as usize];
                hull = amount * t.damage.track_passthrough * facing;
                if tr.blown <= 0.0 {
                    tr.health -= amount * facing;
                    if tr.health <= 0.0 {
                        tr.health = 0.0;
                        tr.blown = t.damage.track_repair_time;
                        self.events.push(Event::TrackBlown { player: target, side });
                    }
                }
            }
            HitLocation::Part(kind) => {
                let tank = &mut self.players[j].tank;
                if let Some(p) = tank.parts.iter_mut().find(|p| p.kind == kind) {
                    p.health -= amount;
                    if p.health <= 0.0 {
                        broke = Some(kind);
                    }
                }
                hull = amount * t.damage.part_passthrough;
            }
        }
        self.events.push(Event::Hit { target, attacker, location: loc, pos, damage: amount, weapon: w });
        if let Some(kind) = broke {
            self.break_part(j, kind);
        }
        let hull = hull * self.core_mult(j, loc);
        let tank = &mut self.players[j].tank;
        tank.health -= hull;
        if tank.health <= 0.0 {
            let killer = attacker.filter(|a| *a != target).or(tank.last_attacker);
            self.kill(j, killer, w);
        }
    }

    fn break_part(&mut self, j: usize, kind: PartKind) {
        let t = self.tuning.clone();
        let id = self.players[j].id;
        let Some(part) = self.players[j].tank.parts.iter().find(|p| p.kind == kind).cloned() else { return };
        let track = part.track;
        let hp = HullPose::of(&self.phys, self.players[j].tank.body);
        let def = part.def();
        let (fo, fr) = self.players[j].tank.frame(&hp, &t, def.frame);
        let hb = def.hitboxes[0];
        let wpos = fo + fr * hb.center;
        let tank = &mut self.players[j].tank;
        let mut lost = 0;
        if part.tier == 0 {
            // A combo or rare part: no tier lost, it just comes off.
            tank.remove_special(&mut self.phys, &t, kind);
        } else {
            let old_tier = tank.tiers[track.index()];
            let new_tier = part.tier.saturating_sub(1);
            tank.tiers[track.index()] = new_tier;
            if new_tier < 2 {
                tank.branch[track.index()] = None;
                if tank.choice.as_ref().is_some_and(|c| c.track == track) {
                    tank.choice = None;
                }
            }
            tank.rebuild_track(&mut self.phys, &t, track);
            lost = old_tier.saturating_sub(new_tier) as u32;
        }
        let vel0 = self.phys.body(self.players[j].tank.body).linvel();
        for _ in 0..lost * t.upgrades.break_scrap_per_tier {
            self.spill(PickupKind::Scrap(track), wpos, vel0);
        }
        // The part itself tumbles away as debris.
        let body = self.phys.world.insert_body(
            RigidBodyBuilder::dynamic()
                .pose(pose(wpos, fr))
                .linvel(vel0 + Vec3::new(self.rng.range(-3.0, 3.0), self.rng.range(4.0, 7.0), self.rng.range(-3.0, 3.0)))
                .angvel(Vec3::new(self.rng.range(-4.0, 4.0), self.rng.range(-4.0, 4.0), self.rng.range(-4.0, 4.0))),
        );
        let h = hb.half.max(Vec3::splat(0.12));
        self.phys.world.insert_collider(
            ColliderBuilder::cuboid(h.x, h.y, h.z).mass(part.mass.clamp(100.0, 2000.0)).collision_groups(debris_groups()),
            Some(body),
        );
        let pid = self.alloc_id();
        self.props.push(Prop { id: pid, kind: PropKind::Debris(kind), body, ttl: Some(t.mode.debris_lifetime), health: None });
        self.events.push(Event::PartBrokeOff { player: id, part: kind, pos: wpos, prop_id: Some(pid) });
        self.on_part_lost(j, kind, wpos);
    }

    pub(crate) fn kill(&mut self, j: usize, killer: Option<PlayerId>, w: Weapon) {
        let t = self.tuning.clone();
        let victim = self.players[j].id;
        let body = self.players[j].tank.body;
        let hp = HullPose::of(&self.phys, body);
        let vel = self.phys.body(body).linvel();
        self.players[j].deaths += 1;
        if let Some(k) = killer.and_then(|k| self.idx(k)) {
            if k != j {
                self.players[k].kills += 1;
            }
        }
        self.events.push(Event::Destroyed { victim, killer, pos: hp.pos, weapon: w });

        // Spill scrap.
        let keep = t.upgrades.respawn_keep_tier;
        let mut spills = Vec::new();
        {
            let tank = &mut self.players[j].tank;
            for k in ScrapKind::ALL {
                let tier = tank.tiers[k.index()];
                let lost = tier.saturating_sub(keep) as u32;
                let n = lost * t.upgrades.death_scrap_per_tier + tank.scrap[k.index()] as u32;
                for _ in 0..n {
                    spills.push(k);
                }
                tank.tiers[k.index()] = tier.min(keep);
                tank.scrap[k.index()] = 0;
                tank.branch[k.index()] = None;
            }
            tank.choice = None;
            tank.repair = None;
            tank.alive = false;
            tank.respawn_in = t.mode.respawn_delay;
        }
        let bonus = t.upgrades.death_bonus_scrap + if self.last_crew.is_some() { t.last_crew.death_bonus_scrap } else { 0 };
        for _ in 0..bonus {
            spills.push(ScrapKind::from_index(self.rng.below(4)));
        }
        for k in spills {
            self.spill(PickupKind::Scrap(k), hp.pos + Vec3::Y, vel);
        }
        self.death_specials(j, hp.pos, vel);
        self.on_raid_death(j, killer, hp.pos, vel);
        self.on_convoy_death(j, killer, hp.pos);
        self.on_last_crew_death(j, killer, hp.pos);
        for k in ScrapKind::ALL {
            self.players[j].tank.rebuild_track(&mut self.phys, &t, k);
        }
        {
            let b = self.phys.body_mut(body);
            b.set_enabled(false);
        }
        // The wreck.
        let h = self.players[j].tank.chassis(&t).hull_half;
        let wb = self.phys.world.insert_body(
            RigidBodyBuilder::dynamic()
                .pose(pose(hp.pos, hp.rot))
                .linvel(vel + Vec3::Y * 3.0)
                .angvel(Vec3::new(self.rng.range(-1.0, 1.0), self.rng.range(-0.5, 0.5), self.rng.range(-1.0, 1.0))),
        );
        self.phys.world.insert_collider(
            ColliderBuilder::cuboid(h[0], h[1] + 0.3, h[2]).mass(18_000.0).collision_groups(prop_groups()),
            Some(wb),
        );
        let pid = self.alloc_id();
        self.props.push(Prop { id: pid, kind: PropKind::Wreck, body: wb, ttl: Some(t.mode.wreck_lifetime), health: None });
    }

    fn pick_spawn(&mut self, except: Option<usize>) -> (Vec3, f32) {
        if let Some(p) = self.raid_spawn() {
            return p;
        }
        if self.convoy.is_some() {
            if let Some(p) = self.convoy_spawn(except) {
                return p;
            }
        }
        let team = match except {
            Some(j) => self.players[j].team,
            None => self.pending_team,
        };
        let team_side = if self.mode.is_team() { team } else { None };
        let split = self.wellhead_split();
        let mut best = (f32::MIN, 0usize);
        for (si, s) in self.map.spawns.iter().enumerate() {
            // Team Brawl: Rustbacks spawn on the west half, Dust Saints on the
            // east. Wellhead: each team on the side of its own derrick.
            if let Some(t) = team_side {
                let west = match split {
                    Some((o, axis)) => (Vec2::new(s.pos.x, s.pos.z) - o).dot(axis) < 0.0,
                    None => s.pos.x < 0.0,
                };
                if west != (t == 0) {
                    continue;
                }
            }
            let mut nearest = 1e6f32;
            for (j, p) in self.players.iter().enumerate() {
                if Some(j) == except || !p.tank.alive {
                    continue;
                }
                let c = self.phys.body(p.tank.body).translation();
                nearest = nearest.min(c.distance(s.pos));
            }
            let score = nearest + self.rng.range(0.0, 30.0);
            if score > best.0 {
                best = (score, si);
            }
        }
        let s = self.map.spawns[best.1];
        (s.pos + Vec3::Y * 1.3, s.yaw)
    }

    pub(crate) fn respawn(&mut self, j: usize) {
        let t = self.tuning.clone();
        let class = self.players[j].loadout.chassis;
        if self.players[j].tank.vehicle == crate::parts::VehicleKind::Tank && self.players[j].tank.class != class {
            self.players[j].tank.set_class(&mut self.phys, &t, class);
        }
        let (pos, yaw) = self.pick_spawn(Some(j));
        let id = self.players[j].id;
        let tank = &mut self.players[j].tank;
        let b = self.phys.body_mut(tank.body);
        b.set_enabled(true);
        b.set_position(pose(pos, Quat::from_rotation_y(yaw)), true);
        b.set_linvel(Vec3::ZERO, true);
        b.set_angvel(Vec3::ZERO, true);
        tank.alive = true;
        tank.health = tank.stats.max_health;
        tank.tracks = [TrackState { health: t.damage.track_health, blown: 0.0 }; 2];
        tank.fuel = tank.stats.fuel_max;
        tank.protect = t.damage.spawn_protection;
        tank.reload = 0.5;
        tank.charge = 0.0;
        tank.bloom = 0.0;
        tank.turret_yaw = 0.0;
        tank.gun_pitch = 0.0;
        tank.last_attacker = None;
        tank.tipped_time = 0.0;
        for w in &mut tank.wheels {
            *w = WheelState::default();
        }
        for p in &mut tank.parts {
            p.health = p.max_health;
        }
        self.events.push(Event::Respawned { player: id, pos });
    }

    // ------------------------------------------------------------- rams

    fn check_rams(&mut self, t: &Tuning, pre_vel: &[Vec3], _dt: f32) {
        let n = self.players.len();
        for a in 0..n {
            for b in (a + 1)..n {
                if !self.players[a].tank.alive || !self.players[b].tank.alive {
                    continue;
                }
                if self.players[a].ram_cd > 0.0 || self.players[b].ram_cd > 0.0 {
                    continue;
                }
                if self.same_team(self.players[a].id, self.players[b].id) {
                    continue;
                }
                let ba = self.players[a].tank.body;
                let bb = self.players[b].tank.body;
                let pa = self.phys.body(ba).translation();
                let pb = self.phys.body(bb).translation();
                if pa.distance(pb) > 9.0 {
                    continue;
                }
                let ca = self.phys.body(ba).colliders().to_vec();
                let cb = self.phys.body(bb).colliders().to_vec();
                let touching = ca.iter().any(|x| {
                    cb.iter().any(|y| {
                        self.phys.world.narrow_phase.contact_pair(*x, *y).is_some_and(|p| p.has_any_active_contact())
                    })
                });
                if !touching {
                    continue;
                }
                let nrm = (pb - pa).normalize_or(Vec3::X);
                let va = pre_vel[a].dot(nrm);
                let vb = pre_vel[b].dot(nrm);
                let closing = va - vb;
                let spiked = self.players[a].tank.stats.spiked || self.players[b].tank.stats.spiked;
                let min_speed = if spiked { t.special.ram_spike_min_speed } else { t.tank.ram_min_speed };
                if closing < min_speed {
                    continue;
                }
                let base = (closing - min_speed) * t.tank.ram_damage_per_ms;
                let spike = t.special.ram_spike_per_ms;
                let mult = |p: &Player| {
                    p.tank.stats.ram_mult
                        * if p.tank.boosting { p.tank.stats.boost_ram_mult } else { 1.0 }
                        * if p.tank.stats.spiked { 1.0 + spike * closing } else { 1.0 }
                };
                // The faster one is the rammer: it deals more and takes less.
                let (ram, vic) = if va.abs() >= vb.abs() { (a, b) } else { (b, a) };
                let m_ram = mult(&self.players[ram]);
                let m_vic = mult(&self.players[vic]);
                let id_r = self.players[ram].id;
                let id_v = self.players[vic].id;
                let mid = (pa + pb) * 0.5;
                self.events.push(Event::Ram { a: id_r, b: id_v, pos: mid, speed: closing });
                let d_rv = if ram == a { nrm } else { -nrm };
                self.damage(vic, Some(id_r), HitLocation::Hull, base * m_ram, mid, d_rv, Weapon::Ram);
                self.damage(ram, Some(id_v), HitLocation::Hull, base * 0.35 * m_vic / m_ram.max(1.0), mid, -d_rv, Weapon::Ram);
                self.players[a].ram_cd = 0.6;
                self.players[b].ram_cd = 0.6;
            }
        }
    }

    // ------------------------------------------------------------- pickups

    pub(crate) fn spill(&mut self, kind: PickupKind, pos: Vec3, base_vel: Vec3) {
        let s = self.tuning.pickups.spill_speed;
        let a = self.rng.range(0.0, std::f32::consts::TAU);
        let r = self.rng.range(0.4, 1.0) * s;
        let vel = base_vel * 0.5 + Vec3::new(a.cos() * r, self.rng.range(0.6, 1.0) * s, a.sin() * r);
        let id = self.alloc_id();
        self.pickups.push(Pickup {
            id,
            kind,
            pos,
            vel,
            age: 0.0,
            ttl: Some(self.tuning.pickups.dropped_lifetime),
            site: None,
            settled: false,
        });
    }

    fn update_sites(&mut self, dt: f32) {
        for si in 0..self.sites.len() {
            if self.sites[si].live.is_some() {
                continue;
            }
            let storm = if matches!(self.sites[si].kind, PickupKind::Scrap(_)) {
                (1.0 + (self.tuning.storm.scrap_respawn_mult - 1.0) * self.storm.intensity) * self.scrap_boost()
            } else {
                1.0
            };
            self.sites[si].timer -= dt * storm;
            if self.sites[si].timer <= 0.0 {
                let id = self.alloc_id();
                if matches!(self.sites[si].kind, PickupKind::Rare(_)) {
                    let r = crate::parts::RARE_PARTS[self.rng.below(crate::parts::RARE_PARTS.len())];
                    self.sites[si].kind = PickupKind::Rare(r);
                }
                let s = &mut self.sites[si];
                s.live = Some(id);
                self.pickups.push(Pickup {
                    id,
                    kind: s.kind,
                    pos: s.pos + Vec3::Y * 0.4,
                    vel: Vec3::ZERO,
                    age: 10.0,
                    ttl: None,
                    site: Some(si),
                    settled: true,
                });
            }
        }
    }

    fn step_pickups(&mut self, t: &Tuning, dt: f32) {
        let ph = self.map.play_half - 3.0;
        let mut k = 0;
        while k < self.pickups.len() {
            let mut remove = false;
            {
                let p = &mut self.pickups[k];
                p.age += dt;
                if let Some(ttl) = p.ttl.as_mut() {
                    *ttl -= dt;
                    if *ttl <= 0.0 {
                        remove = true;
                    }
                }
                if !p.settled {
                    p.vel.y -= 9.81 * dt;
                    p.pos += p.vel * dt;
                    p.pos.x = p.pos.x.clamp(-ph, ph);
                    p.pos.z = p.pos.z.clamp(-ph, ph);
                    let g = self.map.height_at(p.pos.x, p.pos.z) + 0.3;
                    if p.pos.y < g {
                        p.pos.y = g;
                        p.vel.y = -p.vel.y * 0.3;
                        p.vel.x *= 0.5;
                        p.vel.z *= 0.5;
                        if p.vel.length() < 0.6 {
                            p.settled = true;
                        }
                    }
                }
            }
            if remove {
                self.pickups.swap_remove(k);
                continue;
            }
            // Collection.
            let p = self.pickups[k].clone();
            let mut taker = None;
            if p.age >= t.pickups.pickup_delay {
                for j in 0..self.players.len() {
                    let tank = &self.players[j].tank;
                    if !tank.alive {
                        continue;
                    }
                    let c = self.phys.body(tank.body).translation();
                    let flat = Vec2::new(c.x - p.pos.x, c.z - p.pos.z).length();
                    if flat < t.pickups.radius && (c.y - p.pos.y).abs() < 3.5 {
                        let useful = match p.kind {
                            PickupKind::Scrap(_) => true,
                            PickupKind::Rare(kind) => !tank.parts.iter().any(|q| q.kind == kind),
                            PickupKind::AirStrike => !tank.air_strike,
                            PickupKind::Repair => {
                                tank.repair.is_none()
                                    && (tank.health < tank.stats.max_health * 0.98
                                        || tank.tracks.iter().any(|s| s.blown > 0.0)
                                        || tank.parts.iter().any(|q| q.health < q.max_health * 0.9))
                            }
                        };
                        if useful {
                            taker = Some(j);
                            break;
                        }
                    }
                }
            }
            if let Some(j) = taker {
                self.pickups.swap_remove(k);
                if let Some(si) = p.site {
                    self.sites[si].live = None;
                    self.sites[si].timer = match p.kind {
                        PickupKind::Repair => t.pickups.repair_respawn,
                        PickupKind::Rare(_) => self.rng.range(t.special.rare_respawn_min, t.special.rare_respawn_max),
                        PickupKind::AirStrike => t.special.strike_respawn,
                        _ => t.pickups.site_respawn,
                    };
                }
                let id = self.players[j].id;
                self.events.push(Event::PickedUp { player: id, kind: p.kind, pos: p.pos });
                match p.kind {
                    PickupKind::Scrap(kind) => self.add_scrap(j, kind),
                    PickupKind::Repair => self.start_kit_repair(j),
                    PickupKind::Rare(kind) => self.attach_special(j, kind),
                    PickupKind::AirStrike => self.players[j].tank.air_strike = true,
                }
                continue;
            }
            k += 1;
        }
    }

    fn start_kit_repair(&mut self, j: usize) {
        let t = &self.tuning.pickups;
        let id = self.players[j].id;
        let tank = &mut self.players[j].tank;
        let heal = tank.stats.max_health * t.repair_heal;
        tank.repair = Some(Repair { kind: RepairKind::Kit, elapsed: 0.0, duration: t.repair_duration, heal_rate: heal / t.repair_duration });
        self.events.push(Event::RepairStarted { player: id, kind: RepairKind::Kit });
    }

    /// Give a tank one scrap; the third of a kind applies the next tier at once.
    pub fn add_scrap(&mut self, j: usize, kind: ScrapKind) {
        let t = self.tuning.clone();
        let id = self.players[j].id;
        let k = kind.index();
        let tank = &mut self.players[j].tank;
        if tank.tiers[k] >= t.upgrades.max_tier {
            tank.health = (tank.health + t.upgrades.overflow_heal).min(tank.stats.max_health);
            return;
        }
        tank.scrap[k] += 1;
        let per_tier = if tank.vehicle == crate::parts::VehicleKind::Tank { tank.class_stat(&t).scrap_per_tier } else { t.upgrades.scrap_per_tier };
        if tank.scrap[k] < per_tier {
            return;
        }
        tank.scrap[k] = 0;
        tank.tiers[k] += 1;
        let tier = tank.tiers[k];
        let class = tank.class;
        let opts: Vec<PartKind> = branch_options(kind).iter().copied().filter(|o| class.allows(crate::parts::part_def(*o).slot)).collect();
        if tier == 2 && !opts.is_empty() {
            tank.branch[k] = Some(opts[0]);
            tank.choice = Some(crate::tank::PendingChoice {
                track: kind,
                options: opts.to_vec(),
                time_left: t.upgrades.branch_choice_time,
            });
            self.events.push(Event::BranchOffered { player: id, track: kind, options: opts.to_vec(), default: opts[0] });
        }
        tank.rebuild_track(&mut self.phys, &t, kind);
        let part = tank.parts.iter().filter(|p| p.track == kind).max_by_key(|p| p.tier).map(|p| p.kind);
        self.events.push(Event::UpgradeApplied { player: id, track: kind, tier, part });
        // Two tracks at tier 2+: the combo bolts on with this upgrade.
        for combo in crate::parts::combos_unlocked(&self.players[j].tank.tiers) {
            self.attach_special(j, combo);
        }
    }

    // ------------------------------------------------------------- props, safety, mode

    fn step_props(&mut self, dt: f32) {
        let mut k = 0;
        while k < self.props.len() {
            let expired = match self.props[k].ttl.as_mut() {
                Some(ttl) => {
                    *ttl -= dt;
                    *ttl <= 0.0
                }
                None => false,
            };
            let fell = self.phys.body(self.props[k].body).translation().y < -50.0;
            if expired || fell {
                let p = self.props.swap_remove(k);
                self.phys.world.remove_body(p.body);
                continue;
            }
            k += 1;
        }
    }

    fn safety_net(&mut self) {
        for p in &mut self.players {
            if !p.tank.alive {
                continue;
            }
            let b = self.phys.world.bodies.get_mut(p.tank.body).unwrap();
            let c = b.translation();
            let g = self.map.height_at(c.x, c.z);
            if c.y < g - 1.0 || !c.is_finite() {
                let fwd = *b.rotation() * Vec3::X;
                let yaw = (-fwd.z).atan2(fwd.x);
                let c = if c.is_finite() { c } else { Vec3::ZERO };
                b.set_position(pose(Vec3::new(c.x, g + 2.0, c.z), Quat::from_rotation_y(if yaw.is_finite() { yaw } else { 0.0 })), true);
                b.set_linvel(Vec3::ZERO, true);
                b.set_angvel(Vec3::ZERO, true);
                p.tank.rescues += 1;
            }
        }
    }

    /// Last Crew Rolling: scrap respawns faster early so tanks grow fast.
    fn scrap_boost(&self) -> f32 {
        if self.last_crew.is_none() {
            return 1.0;
        }
        let t = &self.tuning.last_crew;
        let s = self.match_ticks as f32 / crate::TICK_HZ as f32;
        let f = ((s - t.scrap_boost_full) / (t.scrap_boost_end - t.scrap_boost_full).max(1.0)).clamp(0.0, 1.0);
        t.scrap_boost + (1.0 - t.scrap_boost) * f
    }

    fn step_mode(&mut self, dt: f32) {
        // Raider Waves and Convoy run their own clocks.
        if self.raid.is_some() || self.convoy.is_some() {
            return;
        }
        if self.phase != Phase::Playing {
            return;
        }
        let _ = dt;
        self.match_ticks += 1;
        // Wellhead and Last Crew judge the end themselves (`step_wellhead`, `step_last_crew`).
        if self.wellhead.is_some() || self.last_crew.is_some() {
            return;
        }
        self.time_left = self.tuning.mode.time_limit - self.match_ticks as f32 / crate::TICK_HZ as f32;
        if self.mode == crate::raid::ModeKind::TeamBrawl {
            self.step_team_mode();
            return;
        }
        let limit = self.tuning.mode.kill_limit;
        let top = self.players.iter().map(|p| p.kills).max().unwrap_or(0);
        if self.time_left <= 0.0 || (limit > 0 && top >= limit) {
            self.time_left = self.time_left.max(0.0);
            let leaders: Vec<&Player> = self.players.iter().filter(|p| p.kills == top).collect();
            let winner = if top == 0 {
                None
            } else if leaders.len() == 1 {
                Some(leaders[0].id)
            } else {
                // Tie: fewest deaths, else nobody.
                let least = leaders.iter().map(|p| p.deaths).min().unwrap_or(0);
                let l2: Vec<_> = leaders.iter().filter(|p| p.deaths == least).collect();
                if l2.len() == 1 {
                    Some(l2[0].id)
                } else {
                    None
                }
            };
            self.phase = Phase::Ended { winner };
            self.events.push(Event::MatchEnded { winner });
        }
    }

    // ------------------------------------------------------------- snapshot

    pub fn snapshot(&self) -> Snapshot {
        let t = &self.tuning;
        let tanks = self
            .players
            .iter()
            .map(|p| {
                let tank = &p.tank;
                let b = self.phys.body(tank.body);
                let g = gun_stats(t, &tank.stats);
                TankSnap {
                    player: p.id,
                    alive: tank.alive,
                    pos: b.translation().to_array(),
                    rot: q_rot(*b.rotation()),
                    linvel: b.linvel().to_array(),
                    angvel: b.angvel().to_array(),
                    turret_yaw: tank.turret_yaw,
                    gun_pitch: tank.gun_pitch,
                    wheels: tank.wheels.iter().map(|w| q01(w.compression / (tank.chassis(t).suspension_travel + tank.chassis(t).wheel_radius))).collect(),
                    track_speed: tank.track_speed.map(|s| (s * 100.0).clamp(-32000.0, 32000.0) as i16),
                    health: tank.health.max(0.0).round() as u16,
                    max_health: tank.stats.max_health.round() as u16,
                    tracks: tank.tracks.map(|s| TrackSnap { health: q01(s.health / t.damage.track_health), blown: s.blown > 0.0 }),
                    parts: tank
                        .parts
                        .iter()
                        .map(|q| PartSnap { kind: q.kind, mk: q.mk, health: q01(q.health / q.max_health) })
                        .collect(),
                    tiers: tank.tiers,
                    scrap: tank.scrap,
                    gun: tank.stats.gun,
                    alt: tank.stats.alt,
                    fuel: q01(tank.fuel / tank.stats.fuel_max),
                    boosting: tank.boosting,
                    bloom: q01(tank.bloom),
                    charge: q01(tank.charge),
                    reload: q01(tank.reload / g.reload),
                    repair: tank.repair.map(|r| RepairSnap { kind: r.kind, progress: q01(r.elapsed / r.duration) }),
                    choice: tank.choice.as_ref().map(|c| ChoiceSnap {
                        track: c.track,
                        options: c.options.clone(),
                        current: tank.branch[c.track.index()].unwrap_or(c.options[0]),
                        ticks_left: (c.time_left * 60.0).max(0.0) as u16,
                    }),
                    respawn_in: (tank.respawn_in.max(0.0) * 60.0) as u16,
                    protected: tank.protect > 0.0,
                    mass: tank.stats.mass,
                    shield: if tank.stats.shield_max > 0.0 { q01(tank.shield / tank.stats.shield_max) } else { 0 },
                    burning: tank.burning > 0.0,
                    flaming: tank.flaming,
                    lock: tank.lock,
                    lock_progress: q01(tank.lock_progress),
                    rare_ammo: tank.rare_ammo,
                    air_strike: tank.air_strike,
                    team: p.team.unwrap_or(255),
                    chassis: tank.class,
                    paint: p.loadout.paint,
                    emblem: p.loadout.emblem,
                    vehicle: tank.vehicle,
                }
            })
            .collect();
        Snapshot {
            tick: self.tick,
            tanks,
            projectiles: self
                .projectiles
                .iter()
                .map(|p| ProjectileSnap { id: p.id, owner: p.owner, gun: p.gun, pos: p.pos.to_array(), vel: p.vel.to_array(), shot: p.shot })
                .collect(),
            pickups: self
                .pickups
                .iter()
                .map(|p| PickupSnap { id: p.id, kind: p.kind, pos: p.pos.to_array(), dropped: p.site.is_none() })
                .collect(),
            props: self
                .props
                .iter()
                .map(|p| {
                    let b = self.phys.body(p.body);
                    PropSnap { id: p.id, kind: p.kind, pos: b.translation().to_array(), rot: q_rot(*b.rotation()) }
                })
                .collect(),
            scores: self
                .players
                .iter()
                .map(|p| ScoreSnap { player: p.id, kills: p.kills as u16, deaths: p.deaths as u16 })
                .collect(),
            mode: ModeSnap {
                phase: self.phase,
                ticks_left: (self.time_left.max(0.0) * 60.0) as u32,
                kill_limit: t.mode.kill_limit as u16,
            },
            smokes: self.smoke_snaps(),
            mines: self.mine_snaps(),
            strikes: self.strike_snaps(),
            hazards: self.hazard_snaps(),
            removed: self.removed.clone(),
            tethers: self.tether_snaps(),
            raid: self.raid_snap(),
            bosses: self.boss_snaps(),
            teams: self.team_scores(),
            storm: self.storm.intensity,
            convoy: self.convoy_snap(),
            wellhead: self.wellhead_snap(),
            storm_circle: self.last_crew_snap(),
        }
    }

    /// Client prediction helper: snap a local tank's physical state to a
    /// server snapshot (pose, velocities, turret, wheels, loadout), before
    /// re-simulating unacknowledged inputs.
    pub fn apply_tank_snap(&mut self, s: &TankSnap) {
        let Some(j) = self.idx(s.player) else { return };
        let t = self.tuning.clone();
        let tank = &mut self.players[j].tank;
        let b = self.phys.body_mut(tank.body);
        b.set_enabled(s.alive);
        b.set_position(pose(Vec3::from(s.pos), dq_rot(s.rot)), true);
        b.set_linvel(Vec3::from(s.linvel), true);
        b.set_angvel(Vec3::from(s.angvel), true);
        tank.alive = s.alive;
        tank.turret_yaw = s.turret_yaw;
        tank.gun_pitch = s.gun_pitch;
        let ray_len = t.tank.suspension_travel + t.tank.wheel_radius;
        for (w, c) in tank.wheels.iter_mut().zip(&s.wheels) {
            w.compression = *c as f32 / 255.0 * ray_len;
        }
        tank.health = s.health as f32;
        tank.boosting = s.boosting;
        tank.fuel = s.fuel as f32 / 255.0 * tank.stats.fuel_max;
        for side in 0..2 {
            tank.tracks[side].blown = if s.tracks[side].blown { tank.tracks[side].blown.max(0.5) } else { 0.0 };
        }
        if tank.tiers != s.tiers || tank.parts.len() != s.parts.len() {
            tank.tiers = s.tiers;
            tank.scrap = s.scrap;
            for p in &s.parts {
                let def = crate::parts::part_def(p.kind);
                if def.track != Track::Powder || p.kind != PartKind::HeavyCannon {
                    if branch_options(def.track).contains(&p.kind) {
                        tank.branch[def.track.index()] = Some(p.kind);
                    }
                }
            }
            for k in ScrapKind::ALL {
                tank.rebuild_track(&mut self.phys, &t, k);
            }
        }
    }

    pub fn hull_yaw(&self, id: PlayerId) -> Option<f32> {
        self.tank_pose(id).map(|(_, r)| yaw_of(r))
    }
}

impl Sim {
    /// Team Brawl: team score = team kills; first to `team_kill_limit` or the most at the whistle.
    fn step_team_mode(&mut self) {
        let limit = self.tuning.mode.team_kill_limit;
        let scores = self.team_scores();
        let top = scores.iter().map(|t| t.kills).max().unwrap_or(0);
        if self.time_left <= 0.0 || (limit > 0 && top as u32 >= limit) {
            self.time_left = self.time_left.max(0.0);
            let leaders: Vec<u8> = scores.iter().filter(|t| t.kills == top).map(|t| t.team).collect();
            let team = if leaders.len() == 1 { Some(leaders[0]) } else { None };
            self.phase = Phase::TeamWon { team };
            self.events.push(Event::TeamMatchEnded { team });
        }
    }

    /// Kills per team (Team Brawl and Convoy; empty otherwise).
    pub fn team_scores(&self) -> Vec<TeamScoreSnap> {
        if !self.mode.is_team() {
            return Vec::new();
        }

        (0..2u8)
            .map(|t| TeamScoreSnap {
                team: t,
                kills: self.players.iter().filter(|p| p.team == Some(t)).map(|p| p.kills).sum::<u32>() as u16,
            })
            .collect()
    }

    /// A Team Brawl match: Rustbacks (team 0) vs Dust Saints (team 1). Players and
    /// bots are put on the smaller team as they join.
    pub fn new_team_brawl(seed: u64, map: Map, tuning: Tuning) -> Sim {
        Sim::with_mode(seed, map, tuning, crate::raid::ModeKind::TeamBrawl)
    }
}
