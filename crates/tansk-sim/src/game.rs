//! The game: a port of the Java original's model (`chalmers.TDA367.B17.model`,
//! `weapons`, `powerups`, `spawnpoints`, `gamemodes`), stepped at a fixed tick.
//!
//! Numbers are the original's. Where the original's behaviour was plainly a
//! bug that broke the game, it is fixed and marked `FIX:`.

use crate::geom::{circle_poly, norm_deg, poly_poly, rect_poly, Aabb, V2};
use crate::map::{self, MapEntity};
use crate::proto::{
    Event, GameView, Phase, PickupKind, PickupView, PlayerView, Power, ProjKind, ProjView, Settings,
    Sound, TankView, Weapon,
};
use crate::rng::Rng;

pub const WIDTH: f32 = 1024.0;
pub const HEIGHT: f32 = 768.0;
/// The original ran Slick2D at 30 frames per second, one logic update per frame.
pub const TICK_MS: i32 = 33;
pub const MAX_PLAYERS: usize = 4;

pub const MAX_HEALTH: f64 = 100.0;
pub const MAX_SHIELD_HEALTH: f64 = 50.0;
pub const SHIELD_RADIUS: f32 = 53.0;
pub const TANK_W: f32 = 48.0;
pub const TANK_H: f32 = 63.0;
pub const TURRET_OFFSET: f32 = 6.0;
pub const TURRET_LENGTH: f32 = 31.5;
pub const ZONE_RADIUS: f32 = 45.0;
pub const ZONE_POS: V2 = V2::new(512.0, 384.0);

const TANK_MAX_SPEED: f32 = 0.07;
const TANK_MIN_SPEED: f32 = -0.04;
const TURN_SPEED: f32 = 0.15;
const POWERUP_SPAWN_MS: i32 = 10_000;
const WEAPON_SPAWN_MS: i32 = 15_000;
const POWERUP_LIMIT: i32 = 6;
const WEAPON_LIMIT: i32 = 6;
const ROUND_DELAY_MS: i32 = 5_000;
const POWER_EFFECT_MS: i32 = 7_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Input {
    pub up: bool,
    pub left: bool,
    pub down: bool,
    pub right: bool,
    pub fire: bool,
    /// Where the mouse is, in world coordinates: the turret turns to face it.
    pub aim: V2,
}

#[derive(Clone, Debug)]
pub struct Player {
    pub id: u32,
    pub name: String,
    pub color: u8,
    pub score: i32,
    pub lives: i32,
    respawn_timer: i32,
    respawn_time: i32,
    pub active: bool,
    pub eliminated: bool,
    elimination_counted: bool,
    pub input: Input,
    pub tank: Option<u32>,
    zone_time: Option<i32>,
}

#[derive(Clone, Debug)]
struct Turret {
    kind: Weapon,
    pos: V2,
    angle: f64,
    ammo: i32,
}

impl Turret {
    fn new(kind: Weapon, pos: V2, angle: f64) -> Self {
        let ammo = match kind {
            Weapon::Default => 0,
            Weapon::Bounce => 30,
            Weapon::Flamethrower => 400,
            Weapon::Shockwave => 5,
            Weapon::Shotgun => 8,
            Weapon::Slowspeedy => 7,
        };
        Self { kind, pos, angle, ammo }
    }

    fn base_fire_rate(&self) -> i32 {
        match self.kind {
            Weapon::Default | Weapon::Bounce => 500,
            Weapon::Flamethrower => 33,
            Weapon::Shockwave => 3000,
            Weapon::Shotgun => 1000,
            Weapon::Slowspeedy => 750,
        }
    }

    fn nozzle(&self) -> V2 {
        let r = self.angle.to_radians();
        V2::new(
            self.pos.x - TURRET_LENGTH * r.sin() as f32,
            self.pos.y + TURRET_LENGTH * r.cos() as f32,
        )
    }
}

#[derive(Clone, Debug)]
struct Shield {
    health: f64,
    /// None: lasts until broken.
    duration: Option<i32>,
}

#[derive(Clone, Copy, Debug)]
struct ActivePower {
    kind: Power,
    left: i32,
}

#[derive(Clone, Debug)]
struct Tank {
    id: u32,
    player: u32,
    color: u8,
    pos: V2,
    theta: f64,
    speed: f32,
    last_pos: V2,
    last_theta: f64,
    health: f64,
    turret: Turret,
    time_since_last_shot: i32,
    shield: Option<Shield>,
    power: Option<ActivePower>,
    dead: bool,
}

impl Tank {
    fn poly(&self) -> [V2; 4] {
        rect_poly(self.pos, TANK_W, TANK_H, self.theta + 90.0)
    }

    fn speed_mult(&self) -> f32 {
        match self.power {
            Some(ActivePower { kind: Power::Speed, .. }) => 2.0,
            _ => 1.0,
        }
    }

    fn set_speed(&mut self, s: f32) {
        let m = self.speed_mult();
        self.speed = s.clamp(TANK_MIN_SPEED * m, TANK_MAX_SPEED * m);
    }

    fn accelerate(&mut self, delta: i32) {
        self.set_speed(self.speed + TANK_MAX_SPEED * 0.001 * delta as f32);
    }

    fn reverse(&mut self, delta: i32) {
        self.set_speed(self.speed - TANK_MAX_SPEED * 0.001 * delta as f32);
    }

    fn friction(&mut self, delta: i32) {
        let f = TANK_MAX_SPEED * 0.001 * delta as f32;
        if self.speed - f > 0.0 {
            self.set_speed(self.speed - f);
        } else if self.speed + f < 0.0 {
            self.set_speed(self.speed + f);
        } else {
            self.set_speed(0.0);
        }
    }

    fn turn(&mut self, delta: i32, sign: f64) {
        self.last_theta = self.theta;
        let amount = TURN_SPEED as f64 * delta as f64 * (self.speed.abs() as f64 * 0.2 + 0.7);
        self.theta = norm_deg(self.theta + sign * amount);
    }

    fn place_turret(&mut self) {
        let d = V2::from_deg(self.theta);
        self.turret.pos = self.pos.sub(d.scale(TURRET_OFFSET));
    }

    fn fire_rate(&self) -> i32 {
        let r = self.turret.base_fire_rate();
        match self.power {
            Some(ActivePower { kind: Power::FireRate, .. }) => (r as f32 * 0.5) as i32,
            _ => r,
        }
    }

    fn damage_mult(&self) -> f64 {
        match self.power {
            Some(ActivePower { kind: Power::Damage, .. }) => 2.0,
            _ => 1.0,
        }
    }
}

#[derive(Clone, Debug)]
struct Projectile {
    id: u32,
    kind: ProjKind,
    owner: u32,
    pos: V2,
    prev_pos: V2,
    theta: f64,
    speed: f32,
    damage: f64,
    timer: i32,
    w: f32,
    h: f32,
    detonated: bool,
    /// For a shockwave's ring: the shockwave it came from.
    parent: Option<u32>,
    dead: bool,
}

impl Projectile {
    fn poly(&self) -> [V2; 4] {
        rect_poly(self.pos, self.w, self.h, self.theta + 90.0)
    }

    fn max_speed(&self) -> f32 {
        if self.kind == ProjKind::Shotgun {
            50.0
        } else {
            100.0
        }
    }

    fn set_speed(&mut self, s: f32) {
        let min = if self.kind == ProjKind::Bounce { -100.0 } else { 0.0 };
        self.speed = s.clamp(min, self.max_speed());
    }
}

#[derive(Clone, Debug)]
struct Pickup {
    id: u32,
    kind: PickupKind,
    rect: Aabb,
}

#[derive(Clone, Debug)]
struct SpawnPoint {
    pos: V2,
    rotation: f64,
}

#[derive(Clone, Copy, Debug)]
struct Wall {
    rect: Aabb,
}

/// The whole game: world, players and rules.
pub struct Game {
    pub settings: Settings,
    pub players: Vec<Player>,
    tanks: Vec<Tank>,
    projectiles: Vec<Projectile>,
    pickups: Vec<Pickup>,
    walls: Vec<Wall>,
    spawn_points: Vec<SpawnPoint>,
    zone: Option<V2>,
    /// Shockwave id -> hits its ring has dealt (at most five).
    shock_hits: Vec<(u32, u32)>,
    spawn_queue: Vec<u32>,
    next_id: u32,
    rng: Rng,
    events: Vec<Event>,
    pub tick: u32,

    pub started: bool,
    game_over: bool,
    delaying: bool,
    delay_timer: i32,
    round_ended: bool,
    round_counter: i32,
    round_timer: i32,
    game_timer: i32,
    eliminated_count: i32,
    powerup_timer: i32,
    weapon_timer: i32,
    powerup_count: i32,
    weapon_count: i32,
}

impl Game {
    pub fn new(settings: Settings, seed: u64) -> Self {
        let settings = settings.clamped();
        let mut g = Game {
            players: Vec::new(),
            tanks: Vec::new(),
            projectiles: Vec::new(),
            pickups: Vec::new(),
            walls: Vec::new(),
            spawn_points: Vec::new(),
            zone: None,
            shock_hits: Vec::new(),
            spawn_queue: Vec::new(),
            next_id: 0,
            rng: Rng::new(seed),
            events: Vec::new(),
            tick: 0,
            started: false,
            game_over: false,
            delaying: false,
            delay_timer: 0,
            round_ended: false,
            round_counter: 0,
            round_timer: settings.round_time_ms,
            game_timer: settings.game_time_ms,
            eliminated_count: 0,
            powerup_timer: POWERUP_SPAWN_MS,
            weapon_timer: WEAPON_SPAWN_MS,
            powerup_count: 0,
            weapon_count: 0,
            settings,
        };
        for e in map::standard() {
            match e {
                MapEntity::Wall(rect) => g.walls.push(Wall { rect }),
                MapEntity::SpawnPoint { pos, rotation } => g.spawn_points.push(SpawnPoint { pos, rotation }),
            }
        }
        g
    }

    fn gen_id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    pub fn is_game_over(&self) -> bool {
        self.game_over
    }

    pub fn phase(&self) -> Phase {
        if self.game_over {
            Phase::GameOver
        } else if !self.started {
            Phase::Lobby
        } else if self.delaying {
            Phase::Countdown
        } else {
            Phase::Playing
        }
    }

    /// Adds a player (lobby only). Returns its id.
    pub fn add_player(&mut self, name: &str, color: u8) -> u32 {
        let id = self.gen_id();
        self.players.push(Player {
            id,
            name: name.to_string(),
            color,
            score: 0,
            lives: self.settings.player_lives(),
            respawn_timer: 0,
            respawn_time: self.settings.spawn_time_ms,
            active: true,
            eliminated: false,
            elimination_counted: false,
            input: Input::default(),
            tank: None,
            zone_time: None,
        });
        id
    }

    pub fn remove_player(&mut self, id: u32) {
        if let Some(tid) = self.player(id).and_then(|p| p.tank) {
            if let Some(t) = self.tanks.iter_mut().find(|t| t.id == tid) {
                t.dead = true;
                let pos = t.pos;
                self.events.push(Event::Explosion(pos));
                self.events.push(Event::Sound(Sound::TankDestroyed));
            }
        }
        self.tanks.retain(|t| !t.dead);
        self.spawn_queue.retain(|&p| p != id);
        self.players.retain(|p| p.id != id);
    }

    pub fn player(&self, id: u32) -> Option<&Player> {
        self.players.iter().find(|p| p.id == id)
    }

    fn player_mut(&mut self, id: u32) -> Option<&mut Player> {
        self.players.iter_mut().find(|p| p.id == id)
    }

    pub fn set_input(&mut self, id: u32, input: Input) {
        if let Some(p) = self.player_mut(id) {
            p.input = input;
        }
    }

    /// The host pressed start: a five second countdown, then the first round.
    pub fn start(&mut self) {
        self.start_with_delay(ROUND_DELAY_MS);
    }

    pub fn start_with_delay(&mut self, delay: i32) {
        if self.started {
            return;
        }
        self.started = true;
        if self.settings.koth {
            self.zone = Some(ZONE_POS);
        }
        self.new_round_delay(delay);
    }

    fn new_round_delay(&mut self, ms: i32) {
        self.delaying = true;
        self.delay_timer = ms;
    }

    // ----------------------------------------------------------------- tick

    /// One tick of `TICK_MS` milliseconds (ServerState.update / Play.update).
    pub fn step(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        if !self.started {
            return;
        }
        let delta = TICK_MS;
        self.update_tank_spawner(delta);
        self.update_spawner(delta);
        self.update_mode(delta);
        self.apply_inputs(delta);
        self.update_world(delta);
    }

    fn update_tank_spawner(&mut self, delta: i32) {
        let queue = std::mem::take(&mut self.spawn_queue);
        let mut keep = Vec::new();
        for pid in queue {
            let Some(p) = self.player_mut(pid) else { continue };
            p.respawn_timer -= delta;
            if p.respawn_timer > 0 {
                keep.push(pid);
                continue;
            }
            // FIX: the original drew a random index among the first
            // `playerCount` points and spun forever if all were blocked.
            let free: Vec<usize> = (0..self.spawn_points.len())
                .filter(|&i| self.spawn_point_free(i))
                .collect();
            if free.is_empty() {
                keep.push(pid);
                continue;
            }
            let i = free[self.rng.below(free.len() as u32) as usize];
            self.spawn_tank_at(pid, i);
        }
        keep.extend(self.spawn_queue.drain(..));
        self.spawn_queue = keep;
    }

    fn spawn_point_free(&self, i: usize) -> bool {
        let sp = &self.spawn_points[i];
        let r = Aabb::centered(sp.pos, 40.0, 55.0).poly();
        !self.tanks.iter().any(|t| !t.dead && poly_poly(&t.poly(), &r))
    }

    /// TankSpawnPoint.spawnTank
    fn spawn_tank_at(&mut self, pid: u32, sp: usize) {
        let id = self.gen_id();
        let SpawnPoint { pos, rotation } = self.spawn_points[sp].clone();
        let color = self.player(pid).map(|p| p.color).unwrap_or(0);
        let theta = norm_deg(rotation + 90.0);
        let mut tank = Tank {
            id,
            player: pid,
            color,
            pos,
            theta,
            speed: 0.0,
            last_pos: pos,
            last_theta: theta,
            health: MAX_HEALTH,
            turret: Turret::new(Weapon::Default, pos, 270.0),
            time_since_last_shot: 0,
            // Three seconds of protection after spawning.
            shield: Some(Shield { health: 9999.0, duration: Some(3000) }),
            power: None,
            dead: false,
        };
        tank.place_turret();
        self.tanks.push(tank);
        if let Some(p) = self.player_mut(pid) {
            p.tank = Some(id);
        }
    }

    /// Player.spawnTank: queue the player for a tank after their respawn time.
    fn queue_spawn(&mut self, pid: u32) {
        if let Some(tid) = self.player(pid).and_then(|p| p.tank) {
            self.tanks.retain(|t| t.id != tid);
        }
        let Some(p) = self.player_mut(pid) else { return };
        p.tank = None;
        p.active = true;
        p.respawn_timer = p.respawn_time;
        // FIX: the original could queue a player twice and give them two tanks.
        if !self.spawn_queue.contains(&pid) {
            self.spawn_queue.push(pid);
        }
    }

    /// Spawner.update: power-ups every 10 s, weapons every 15 s, six of each at most.
    fn update_spawner(&mut self, delta: i32) {
        if self.game_over {
            return;
        }
        self.powerup_timer -= delta;
        self.weapon_timer -= delta;
        if self.powerup_count < POWERUP_LIMIT && self.powerup_timer <= 0 {
            self.powerup_timer = POWERUP_SPAWN_MS;
            if let Some(p) = self.find_pickup_spot() {
                let kind = match self.rng.below(5) {
                    1 => Power::FireRate,
                    2 => Power::Speed,
                    3 => Power::Shield,
                    4 => Power::Damage,
                    _ => Power::Health,
                };
                self.add_pickup(PickupKind::Power(kind), p);
            }
        }
        if self.weapon_count < WEAPON_LIMIT && self.weapon_timer <= 0 {
            self.weapon_timer = WEAPON_SPAWN_MS;
            if let Some(p) = self.find_pickup_spot() {
                let kind = match self.rng.below(5) {
                    1 => Weapon::Flamethrower,
                    2 => Weapon::Shotgun,
                    3 => Weapon::Shockwave,
                    4 => Weapon::Slowspeedy,
                    _ => Weapon::Bounce,
                };
                self.add_pickup(PickupKind::Weapon(kind), p);
            }
        }
    }

    pub fn add_pickup(&mut self, kind: PickupKind, pos: V2) {
        let id = self.gen_id();
        let size = match kind {
            PickupKind::Weapon(_) => {
                self.weapon_count += 1;
                40.0
            }
            PickupKind::Power(_) => {
                self.powerup_count += 1;
                35.0
            }
        };
        self.pickups.push(Pickup { id, kind, rect: Aabb::centered(pos, size, size) });
    }

    /// Spawner.generateSpawnPoint: a random spot whose 75x75 square is clear.
    fn find_pickup_spot(&mut self) -> Option<V2> {
        for _ in 0..200 {
            let mut x = self.rng.below(WIDTH as u32) as f32;
            let mut y = self.rng.below(HEIGHT as u32) as f32;
            if x < 35.0 {
                x += 35.0;
            }
            if x > WIDTH - 35.0 {
                x -= 35.0;
            }
            if y < 35.0 {
                y += 35.0;
            }
            if y > HEIGHT - 35.0 {
                y -= 35.0;
            }
            let p = V2::new(x, y);
            let r = Aabb::centered(p, 75.0, 75.0).poly();
            let blocked = self.spawn_points.iter().any(|s| poly_poly(&Aabb::centered(s.pos, 40.0, 55.0).poly(), &r))
                || self.tanks.iter().any(|t| poly_poly(&t.poly(), &r))
                || self.pickups.iter().any(|k| poly_poly(&k.rect.poly(), &r))
                || self.walls.iter().any(|w| poly_poly(&w.rect.poly(), &r));
            if !blocked {
                return Some(p);
            }
        }
        None
    }

    // ------------------------------------------------------------- game mode

    /// GameConditions.update + StandardGameMode / KingOfTheHillMode.update
    fn update_mode(&mut self, delta: i32) {
        if self.settings.koth && self.game_over {
            return;
        }
        if !self.game_over && !self.delaying {
            self.game_timer -= delta;
            self.round_timer -= delta;
            if self.game_timer <= 0 {
                self.game_over = true;
            }
            let n = self.players.len() as i32;
            if (self.eliminated_count >= n - 1 && n > 1) || self.round_timer <= 0 {
                self.new_round_delay(ROUND_DELAY_MS);
            }
        }
        if self.delaying {
            self.end_round();
            self.delay_timer -= delta;
            if self.delay_timer <= 0 {
                self.new_round();
                self.delaying = false;
            }
        }

        if !self.settings.koth {
            // FIX: in the original a dead player was already inactive here, so
            // no elimination was ever counted and Standard never scored. Each
            // elimination gives every player still in the round a point.
            let newly: Vec<u32> = self
                .players
                .iter()
                .filter(|p| p.eliminated && !p.elimination_counted)
                .map(|p| p.id)
                .collect();
            for id in newly {
                if let Some(p) = self.player_mut(id) {
                    p.elimination_counted = true;
                    p.active = false;
                }
                self.eliminated_count += 1;
                for p in self.players.iter_mut().filter(|p| !p.eliminated) {
                    p.score += 1;
                }
            }
        } else if !self.game_over {
            let ids: Vec<u32> = self.players.iter().map(|p| p.id).collect();
            for id in ids {
                let (active, tank) = {
                    let p = self.player(id).unwrap();
                    (p.active, p.tank)
                };
                if !active {
                    self.queue_spawn(id);
                }
                let in_zone = match (tank.and_then(|t| self.tank(t)), self.zone) {
                    (Some(t), Some(z)) => circle_poly(z, ZONE_RADIUS, &t.poly()),
                    _ => false,
                };
                if in_zone {
                    let p = self.player_mut(id).unwrap();
                    match p.zone_time {
                        None => p.zone_time = Some(0),
                        Some(t) => {
                            let t = t + delta;
                            // A point for every second spent in the zone.
                            if t > 1000 {
                                p.score += 1;
                                p.zone_time = Some(0);
                            } else {
                                p.zone_time = Some(t);
                            }
                        }
                    }
                }
            }
            if self.players.iter().any(|p| p.score >= self.settings.score_limit) {
                self.game_over = true;
            }
        }
    }

    fn end_round(&mut self) {
        if self.round_ended {
            return;
        }
        self.powerup_count = 0;
        self.weapon_count = 0;
        self.projectiles.clear();
        self.pickups.clear();
        self.shock_hits.clear();
        for t in &mut self.tanks {
            t.shield = None;
        }
        self.round_ended = true;
        if self.players.iter().any(|p| p.score >= self.settings.score_limit) {
            self.game_over = true;
        }
        // FIX: the original never used its "Rounds" setting.
        if self.round_counter > 0 && self.round_counter >= self.settings.rounds {
            self.game_over = true;
        }
    }

    fn new_round(&mut self) {
        if self.game_over {
            return;
        }
        self.round_ended = false;
        self.round_counter += 1;
        self.round_timer = self.settings.round_time_ms;
        self.eliminated_count = 0;
        let ids: Vec<u32> = self.players.iter().map(|p| p.id).collect();
        for id in &ids {
            let p = self.player_mut(*id).unwrap();
            p.respawn_time = 100;
            p.eliminated = false;
            p.elimination_counted = false;
            p.active = true;
            self.queue_spawn(*id);
        }
        let lives = self.settings.player_lives();
        let spawn = self.settings.spawn_time_ms;
        for p in &mut self.players {
            p.respawn_time = spawn;
            p.lives = lives;
        }
    }

    fn tank(&self, id: u32) -> Option<&Tank> {
        self.tanks.iter().find(|t| t.id == id && !t.dead)
    }

    // ---------------------------------------------------------------- input

    /// ServerState.updatePlayerTanks
    fn apply_inputs(&mut self, delta: i32) {
        for pi in 0..self.players.len() {
            let input = self.players[pi].input;
            let Some(tid) = self.players[pi].tank else { continue };
            let Some(ti) = self.tanks.iter().position(|t| t.id == tid && !t.dead) else { continue };
            {
                let t = &mut self.tanks[ti];
                let tp = t.turret.pos;
                t.turret.angle = norm_deg(
                    ((tp.x - input.aim.x) as f64).atan2((tp.y - input.aim.y) as f64).to_degrees() * -1.0 + 180.0,
                );
                if input.up {
                    t.accelerate(delta);
                } else if input.down {
                    t.reverse(delta);
                } else {
                    t.friction(delta);
                }
                if input.left && !input.right {
                    t.turn(delta, if input.down { 1.0 } else { -1.0 });
                }
                if input.right && !input.left {
                    t.turn(delta, if input.down { -1.0 } else { 1.0 });
                }
            }
            if input.fire {
                self.fire(ti);
            }
        }
    }

    /// Point a player's turret somewhere without other input (the playground's dummies).
    pub fn aim(&mut self, pid: u32, at: V2) {
        if let Some(p) = self.player_mut(pid) {
            p.input.aim = at;
        }
    }

    /// AbstractTank.fireTurret + the turrets' fireWeapon.
    fn fire(&mut self, ti: usize) {
        if self.tanks[ti].time_since_last_shot > 0 {
            return;
        }
        let kind = self.tanks[ti].turret.kind;
        let angle = self.tanks[ti].turret.angle;
        match kind {
            Weapon::Default => {
                self.spawn_projectile(ti, ProjKind::Default, angle + 90.0, None);
                self.events.push(Event::Sound(Sound::DefaultFire));
            }
            Weapon::Bounce => {
                if self.tanks[ti].turret.ammo > 0 {
                    self.spawn_projectile(ti, ProjKind::Bounce, angle + 90.0, None);
                    self.tanks[ti].turret.ammo -= 1;
                    self.events.push(Event::Sound(Sound::DefaultFire));
                } else {
                    self.reset_turret(ti);
                }
            }
            Weapon::Flamethrower => {
                for _ in 0..2 {
                    if self.tanks[ti].turret.ammo > 0 {
                        self.spawn_projectile(ti, ProjKind::Flame, angle + 90.0, None);
                        self.tanks[ti].turret.ammo -= 1;
                        if self.tanks[ti].turret.ammo % 10 == 0 {
                            self.events.push(Event::Sound(Sound::Flamethrower));
                        }
                    } else {
                        self.reset_turret(ti);
                        break;
                    }
                }
            }
            Weapon::Shotgun => {
                if self.tanks[ti].turret.ammo > 0 {
                    for i in -8i32..8 {
                        let spread = i as f64 * self.rng.f64() * 2.0;
                        let a = if i % 2 == 0 { angle + 90.0 - spread } else { angle + 90.0 + spread };
                        let speed = (0.3 - i as f32 * 0.025).abs();
                        self.spawn_projectile(ti, ProjKind::Shotgun, a, Some(speed));
                    }
                    self.tanks[ti].turret.ammo -= 1;
                    self.events.push(Event::Sound(Sound::Shotgun));
                } else {
                    self.reset_turret(ti);
                }
            }
            Weapon::Slowspeedy => {
                if self.tanks[ti].turret.ammo > 0 {
                    for i in 0..5 {
                        self.spawn_projectile(ti, ProjKind::Slowspeedy, angle + 86.0 + (i * 2) as f64, None);
                    }
                    self.tanks[ti].turret.ammo -= 1;
                    self.events.push(Event::Sound(Sound::SlowspeedyFire));
                } else {
                    self.reset_turret(ti);
                }
            }
            Weapon::Shockwave => {
                if self.tanks[ti].turret.ammo > 0 {
                    // Spends two per shot, as the original did: three shots.
                    self.tanks[ti].turret.ammo -= 2;
                    self.spawn_projectile(ti, ProjKind::Shockwave, angle + 90.0, None);
                } else {
                    self.reset_turret(ti);
                }
            }
        }
        let t = &mut self.tanks[ti];
        t.time_since_last_shot = t.fire_rate();
    }

    fn reset_turret(&mut self, ti: usize) {
        let t = &mut self.tanks[ti];
        t.turret = Turret::new(Weapon::Default, t.turret.pos, t.turret.angle);
    }

    fn spawn_projectile(&mut self, ti: usize, kind: ProjKind, theta: f64, speed: Option<f32>) {
        let id = self.gen_id();
        let t = &self.tanks[ti];
        let pos = t.turret.nozzle();
        let (speed0, damage, duration, w, h) = proj_stats(kind);
        let mut p = Projectile {
            id,
            kind,
            owner: t.id,
            pos,
            prev_pos: pos,
            theta: norm_deg(theta),
            speed: speed0,
            damage: damage * t.damage_mult(),
            timer: duration,
            w,
            h,
            detonated: false,
            parent: None,
            dead: false,
        };
        if let Some(s) = speed {
            p.set_speed(s);
        }
        self.projectiles.push(p);
    }

    /// The test arena's number keys.
    pub fn give_weapon(&mut self, pid: u32, kind: Weapon) {
        let Some(tid) = self.player(pid).and_then(|p| p.tank) else { return };
        if let Some(t) = self.tanks.iter_mut().find(|t| t.id == tid) {
            t.turret = Turret::new(kind, t.turret.pos, t.turret.angle);
        }
    }

    // ---------------------------------------------------------------- world

    /// ServerState.updateWorld: update every entity, then its collisions.
    fn update_world(&mut self, delta: i32) {
        for ti in 0..self.tanks.len() {
            if self.tanks[ti].dead {
                continue;
            }
            self.update_tank(ti, delta);
        }
        self.tanks.retain(|t| !t.dead);

        let mut pi = 0;
        while pi < self.projectiles.len() {
            if !self.projectiles[pi].dead {
                self.update_projectile(pi, delta);
            }
            pi += 1;
        }
        self.projectiles.retain(|p| !p.dead);
        let live: Vec<u32> = self.projectiles.iter().filter(|p| p.kind == ProjKind::Shockwave).map(|p| p.id).collect();
        self.shock_hits.retain(|(id, _)| {
            live.contains(id) || self.projectiles.iter().any(|p| p.parent == Some(*id))
        });
    }

    fn update_tank(&mut self, ti: usize, delta: i32) {
        {
            let t = &mut self.tanks[ti];
            // MovableEntity.move moved the shape twice (setPosition, then a
            // translate), so everything travels at twice its nominal speed.
            let v = V2::from_deg(t.theta).scale(t.speed * delta as f32 * 2.0);
            t.last_pos = t.pos;
            t.pos = t.pos.add(v);
            t.time_since_last_shot -= delta;
            t.place_turret();

            if let Some(p) = &mut t.power {
                p.left -= delta;
                if p.left <= 0 {
                    t.power = None;
                    let m = t.speed_mult();
                    t.speed = t.speed.clamp(TANK_MIN_SPEED * m, TANK_MAX_SPEED * m);
                }
            }
            if let Some(s) = &mut t.shield {
                if let Some(d) = &mut s.duration {
                    *d -= delta;
                }
                if s.health <= 0.0 || s.duration.is_some_and(|d| d <= 0) {
                    t.shield = None;
                }
            }
        }
        let t = &self.tanks[ti];
        if t.pos.x < 0.0 || t.pos.x > WIDTH || t.pos.y < 0.0 || t.pos.y > HEIGHT {
            self.tanks[ti].dead = true;
            return;
        }

        // Collisions.
        let poly = t.poly();
        let hits_bounds = poly.iter().any(|v| v.x < -1.0 || v.x > WIDTH + 1.0 || v.y < -1.0 || v.y > HEIGHT);
        let hits_wall = self.walls.iter().any(|w| poly_poly(&w.rect.poly(), &poly));
        let hits_tank = self
            .tanks
            .iter()
            .enumerate()
            .any(|(j, o)| j != ti && !o.dead && poly_poly(&o.poly(), &poly));
        if hits_bounds || hits_wall || hits_tank {
            let t = &mut self.tanks[ti];
            if t.last_pos != t.pos {
                t.pos = t.last_pos;
                t.set_speed(-(t.speed * 0.5));
                t.place_turret();
            }
            if t.last_theta != t.theta {
                t.theta = t.last_theta;
            }
        }

        let poly = self.tanks[ti].poly();
        let mut k = 0;
        while k < self.pickups.len() {
            if poly_poly(&self.pickups[k].rect.poly(), &poly) {
                let pk = self.pickups.remove(k);
                self.activate_pickup(ti, pk.kind);
            } else {
                k += 1;
            }
        }
    }

    fn activate_pickup(&mut self, ti: usize, kind: PickupKind) {
        let t = &mut self.tanks[ti];
        match kind {
            PickupKind::Weapon(w) => {
                self.weapon_count -= 1;
                t.turret = Turret::new(w, t.turret.pos, t.turret.angle);
            }
            PickupKind::Power(p) => {
                self.powerup_count -= 1;
                match p {
                    Power::Health => {
                        t.health = if t.health >= 75.0 { MAX_HEALTH } else { t.health + 25.0 };
                    }
                    Power::Shield => {
                        t.shield = Some(Shield { health: MAX_SHIELD_HEALTH, duration: None });
                    }
                    Power::Speed | Power::FireRate | Power::Damage => {
                        // One timed power-up at a time; a new one ends the old.
                        t.power = Some(ActivePower { kind: p, left: POWER_EFFECT_MS });
                        let m = t.speed_mult();
                        t.speed = t.speed.clamp(TANK_MIN_SPEED * m, TANK_MAX_SPEED * m);
                    }
                }
            }
        }
    }

    fn update_projectile(&mut self, pi: usize, delta: i32) {
        let kind = self.projectiles[pi].kind;
        if kind == ProjKind::Shockwave {
            let p = &self.projectiles[pi];
            if p.timer <= 20 && !p.detonated {
                self.detonate(pi);
                return;
            }
        }
        if kind == ProjKind::Slowspeedy {
            let p = &mut self.projectiles[pi];
            if p.speed < 0.1 {
                p.set_speed(p.speed * 1.02);
            } else if p.speed < 0.35 {
                p.set_speed(0.55);
                self.events.push(Event::Sound(Sound::SlowspeedySecondary));
            }
        }
        let wobble = match kind {
            ProjKind::Flame => (self.rng.f64() * 80.0 - 40.0).trunc(),
            ProjKind::Shotgun => (self.rng.f64() * 10.0 - 5.0).trunc(),
            _ => 0.0,
        };
        {
            let p = &mut self.projectiles[pi];
            p.timer -= delta;
            if p.timer < 0 {
                p.dead = true;
                return;
            }
            let v = V2::from_deg(p.theta + wobble).scale(p.speed * delta as f32 * 2.0);
            p.prev_pos = p.pos;
            p.pos = p.pos.add(v);
            if p.pos.x < 0.0 || p.pos.x > WIDTH || p.pos.y < 0.0 || p.pos.y > HEIGHT {
                p.dead = true;
                return;
            }
        }
        self.projectile_collisions(pi);
    }

    fn projectile_collisions(&mut self, pi: usize) {
        let (kind, owner, poly) = {
            let p = &self.projectiles[pi];
            (p.kind, p.owner, p.poly())
        };

        // Other tanks' shields stop projectiles (Shield.didCollideWith).
        for ti in 0..self.tanks.len() {
            let t = &self.tanks[ti];
            if t.dead || t.id == owner || t.shield.is_none() {
                continue;
            }
            if circle_poly(t.pos, SHIELD_RADIUS, &poly) {
                if kind == ProjKind::Shockwave && !self.projectiles[pi].detonated {
                    self.detonate(pi);
                }
                let dmg = self.projectiles[pi].damage;
                self.damage_shield(ti, dmg);
                self.projectiles[pi].dead = true;
                return;
            }
        }

        // Map bounds and walls.
        let hits_bounds = poly.iter().any(|v| v.x < -1.0 || v.x > WIDTH + 1.0 || v.y < -1.0 || v.y > HEIGHT);
        let wall = self.walls.iter().find(|w| poly_poly(&w.rect.poly(), &poly)).map(|w| w.rect);
        if hits_bounds || wall.is_some() {
            match kind {
                ProjKind::Bounce => self.bounce(pi, wall),
                ProjKind::Shockwave => {
                    if !self.projectiles[pi].detonated {
                        self.detonate(pi);
                    }
                    return;
                }
                _ => {
                    self.projectiles[pi].dead = true;
                    return;
                }
            }
        }

        // Tanks.
        for ti in 0..self.tanks.len() {
            if self.tanks[ti].dead || !poly_poly(&self.tanks[ti].poly(), &poly) {
                continue;
            }
            let own = self.tanks[ti].id == owner;
            match kind {
                ProjKind::Shockwave => {
                    if !own {
                        self.detonate(pi);
                        return;
                    }
                }
                ProjKind::ShockwaveSecondary => {
                    // Hurts its own tank too, but less.
                    if own {
                        self.projectiles[pi].damage = 2.0;
                    }
                    let parent = self.projectiles[pi].parent.unwrap_or(0);
                    let hits = match self.shock_hits.iter_mut().find(|(id, _)| *id == parent) {
                        Some((_, n)) => {
                            *n += 1;
                            *n
                        }
                        None => {
                            self.shock_hits.push((parent, 1));
                            1
                        }
                    };
                    // One shockwave deals at most five hits.
                    if hits <= 5 {
                        let dmg = self.projectiles[pi].damage;
                        self.damage_tank(ti, dmg, kind);
                    }
                    self.projectiles[pi].dead = true;
                    return;
                }
                _ => {
                    if own {
                        continue;
                    }
                    let dmg = self.projectiles[pi].damage;
                    self.damage_tank(ti, dmg, kind);
                    self.projectiles[pi].dead = true;
                    return;
                }
            }
        }
    }

    /// BounceProjectile: reflect off the side it came through.
    /// FIX: the original compared x with the wall's width instead of its
    /// right edge, so bullets slid through walls from the right.
    fn bounce(&mut self, pi: usize, wall: Option<Aabb>) {
        let p = &mut self.projectiles[pi];
        let d = V2::from_deg(p.theta);
        let flip_x = match wall {
            Some(r) => {
                let was_beside = p.prev_pos.x < r.x || p.prev_pos.x > r.x + r.w;
                let was_above_below = p.prev_pos.y < r.y || p.prev_pos.y > r.y + r.h;
                was_beside && !was_above_below || (was_beside && was_above_below && {
                    let dx = (p.prev_pos.x - r.center().x).abs() - r.w / 2.0;
                    let dy = (p.prev_pos.y - r.center().y).abs() - r.h / 2.0;
                    dx > dy
                })
            }
            None => (p.pos.x - -1.0).abs() < 5.0 || (p.pos.x - (WIDTH + 1.0)).abs() < 5.0 || p.pos.x < 5.0 || p.pos.x > WIDTH - 5.0,
        };
        let (dx, dy) = if flip_x { (-d.x, d.y) } else { (d.x, -d.y) };
        p.theta = norm_deg((dy as f64).atan2(dx as f64).to_degrees());
        p.pos = p.prev_pos;
    }

    /// ShockwaveProjectile.detonate: a ring of 40 slow, hard-hitting bits.
    fn detonate(&mut self, pi: usize) {
        self.events.push(Event::Sound(Sound::ShockwaveDetonate));
        let (pos, theta, owner, id) = {
            let p = &mut self.projectiles[pi];
            p.detonated = true;
            p.dead = true;
            (p.pos, p.theta, p.owner, p.id)
        };
        let (speed, damage, duration, w, h) = proj_stats(ProjKind::ShockwaveSecondary);
        for i in 0..40 {
            let nid = self.gen_id();
            self.projectiles.push(Projectile {
                id: nid,
                kind: ProjKind::ShockwaveSecondary,
                owner,
                pos,
                prev_pos: pos,
                theta: norm_deg(theta + 90.0 + (i * 9) as f64),
                speed,
                damage,
                timer: duration,
                w,
                h,
                detonated: false,
                parent: Some(id),
                dead: false,
            });
        }
    }

    /// AbstractTank.receiveDamage
    fn damage_tank(&mut self, ti: usize, dmg: f64, kind: ProjKind) {
        if kind != ProjKind::Flame {
            self.events.push(Event::Sound(Sound::TankHit));
        }
        if self.tanks[ti].shield.as_ref().is_some_and(|s| s.health > 0.0) {
            self.damage_shield(ti, dmg);
        } else {
            self.tanks[ti].health -= dmg;
            self.check_death(ti);
        }
    }

    /// Shield.damageShield: what the shield can't take, the tank does.
    fn damage_shield(&mut self, ti: usize, dmg: f64) {
        let t = &mut self.tanks[ti];
        let Some(s) = &mut t.shield else { return };
        let overkill = s.health - dmg;
        if overkill <= 0.0 {
            t.health += overkill;
        }
        s.health -= dmg;
        if s.health <= 0.0 {
            t.shield = None;
        }
        self.check_death(ti);
    }

    /// AbstractTank.tankDeath + Player.tankDeath
    fn check_death(&mut self, ti: usize) {
        let t = &mut self.tanks[ti];
        if t.dead || t.health > 0.0 {
            return;
        }
        t.dead = true;
        let (pos, pid) = (t.pos, t.player);
        self.events.push(Event::Sound(Sound::TankDestroyed));
        self.events.push(Event::Explosion(pos));
        let Some(p) = self.player_mut(pid) else { return };
        p.lives -= 1;
        p.tank = None;
        p.active = false;
        if p.lives > 0 {
            self.queue_spawn(pid);
        } else {
            p.eliminated = true;
        }
    }

    // ----------------------------------------------------------------- view

    /// What a client needs to draw this tick. Drains the tick's events.
    pub fn view(&mut self) -> GameView {
        GameView {
            tick: self.tick,
            phase: self.phase(),
            delay_ms: if self.delaying { self.delay_timer.max(0) } else { 0 },
            round: self.round_counter,
            round_ms_left: self.round_timer.max(0),
            game_ms_left: self.game_timer.max(0),
            tanks: self
                .tanks
                .iter()
                .filter(|t| !t.dead)
                .map(|t| TankView {
                    id: t.id,
                    player: t.player,
                    color: t.color,
                    pos: t.pos,
                    theta: t.theta as f32,
                    health: t.health as f32,
                    shield: t.shield.as_ref().map(|s| s.health as f32),
                    turret_pos: t.turret.pos,
                    turret_angle: t.turret.angle as f32,
                    weapon: t.turret.kind,
                    ammo: t.turret.ammo,
                    power: t.power.map(|p| p.kind),
                })
                .collect(),
            projectiles: self
                .projectiles
                .iter()
                .map(|p| ProjView { id: p.id, kind: p.kind, pos: p.pos, theta: p.theta as f32 })
                .collect(),
            pickups: self
                .pickups
                .iter()
                .map(|k| PickupView { id: k.id, kind: k.kind, pos: k.rect.center() })
                .collect(),
            players: self
                .players
                .iter()
                .map(|p| PlayerView {
                    id: p.id,
                    name: p.name.clone(),
                    color: p.color,
                    score: p.score,
                    lives: p.lives,
                    active: p.active,
                    eliminated: p.eliminated,
                })
                .collect(),
            zone: self.zone,
            events: std::mem::take(&mut self.events),
        }
    }
}

/// (speed, damage, duration ms, width, height) of each projectile.
fn proj_stats(kind: ProjKind) -> (f32, f64, i32, f32, f32) {
    match kind {
        ProjKind::Default => (0.35, 5.0, 10_000, 5.0, 10.0),
        ProjKind::Bounce => (0.25, 8.0, 3000, 5.0, 10.0),
        ProjKind::Flame => (0.25, 2.0, 300, 5.0, 10.0),
        ProjKind::Shockwave => (0.2, 0.5, 1500, 8.0, 10.0),
        ProjKind::ShockwaveSecondary => (0.05, 10.0, 1200, 2.0, 2.0),
        ProjKind::Shotgun => (0.3, 3.0, 350, 3.0, 6.0),
        ProjKind::Slowspeedy => (0.05, 5.0, 3000, 15.0, 5.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_player_game(koth: bool) -> (Game, u32, u32) {
        let s = Settings { koth, ..Settings::default() };
        let mut g = Game::new(s, 7);
        let a = g.add_player("a", 0);
        let b = g.add_player("b", 1);
        g.start();
        // Countdown, then spawn.
        for _ in 0..200 {
            g.step();
        }
        (g, a, b)
    }

    #[test]
    fn players_spawn_after_countdown() {
        let (mut g, a, b) = two_player_game(false);
        let v = g.view();
        assert_eq!(v.phase, Phase::Playing);
        assert_eq!(v.tanks.len(), 2);
        assert!(g.player(a).unwrap().tank.is_some());
        assert!(g.player(b).unwrap().tank.is_some());
    }

    #[test]
    fn tank_drives_and_stops_at_walls() {
        let (mut g, a, _) = two_player_game(false);
        let start = g.tanks.iter().find(|t| t.player == a).unwrap().pos;
        g.set_input(a, Input { up: true, ..Input::default() });
        for _ in 0..30 {
            g.step();
        }
        let after = g.tanks.iter().find(|t| t.player == a).unwrap().pos;
        assert!(start.sub(after).len2() > 100.0, "tank should move");
        for _ in 0..600 {
            g.step();
        }
        let t = g.tanks.iter().find(|t| t.player == a).unwrap();
        assert!(t.pos.x > 0.0 && t.pos.x < WIDTH && t.pos.y > 0.0 && t.pos.y < HEIGHT);
    }

    #[test]
    fn shooting_kills_and_standard_scores() {
        let (mut g, a, b) = two_player_game(false);
        // Put the two tanks side by side and drop b's spawn shield.
        let bpos = g.tanks.iter().find(|t| t.player == b).unwrap().pos;
        for t in &mut g.tanks {
            t.shield = None;
            if t.player == a {
                t.pos = V2::new(bpos.x, bpos.y - 150.0).add(V2::new(0.0, 0.0));
                if t.pos.y < 40.0 {
                    t.pos = V2::new(bpos.x, bpos.y + 150.0);
                }
                t.place_turret();
            }
        }
        let lives = g.player(b).unwrap().lives;
        let mut deaths = 0;
        for _ in 0..4000 {
            let bt = g.tanks.iter().find(|t| t.player == b).map(|t| t.pos);
            if let Some(bt) = bt {
                g.set_input(a, Input { fire: true, aim: bt, ..Input::default() });
                for t in &mut g.tanks {
                    if t.player == b {
                        t.shield = None;
                    }
                }
            }
            let before = g.player(b).unwrap().lives;
            g.step();
            if g.player(b).map(|p| p.lives) < Some(before) {
                deaths += 1;
            }
            if g.player(b).unwrap().eliminated || g.is_game_over() {
                break;
            }
        }
        assert!(deaths >= 1, "b should die at least once (lives {lives})");
    }

    #[test]
    fn koth_scores_in_zone() {
        let (mut g, a, _) = two_player_game(true);
        for t in &mut g.tanks {
            if t.player == a {
                t.pos = ZONE_POS.add(V2::new(0.0, 20.0));
                t.place_turret();
            }
        }
        for _ in 0..100 {
            g.step();
        }
        assert!(g.player(a).unwrap().score >= 2);
    }

    #[test]
    fn pickups_spawn_over_time() {
        let (mut g, _, _) = two_player_game(false);
        for _ in 0..(16_000 / TICK_MS) {
            g.step();
        }
        assert!(g.pickups.len() >= 2);
    }

    #[test]
    fn every_weapon_fires() {
        for w in [Weapon::Bounce, Weapon::Flamethrower, Weapon::Shockwave, Weapon::Shotgun, Weapon::Slowspeedy] {
            let (mut g, a, _) = two_player_game(false);
            g.give_weapon(a, w);
            g.set_input(a, Input { fire: true, aim: V2::new(512.0, 384.0), ..Input::default() });
            let mut seen = 0;
            for _ in 0..60 {
                g.step();
                seen = seen.max(g.projectiles.len());
            }
            assert!(seen > 0, "{w:?} fired nothing");
        }
    }
}
