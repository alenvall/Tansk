//! The 3D game: the 2D game's rules (`tansk_sim::game`) on hilly terrain
//! with ballistic projectiles. API contract: the server and the browser code
//! against these signatures.
//!
//! Drawing notes: a tank's turret pivots at hull pos + `TURRET_PIVOT_UP`
//! straight up (hull tilt ignored); the muzzle is `BARREL_LENGTH` from the
//! pivot along (turret_yaw, barrel_pitch). The hull box is `TANK_LENGTH`
//! along the heading, `TANK_WIDTH` across, `TANK_BOX_HEIGHT` tall.
//! hull_pitch > 0: nose up; hull_roll > 0: right side (yaw + 90) up.

use tansk_sim::geom::{poly_poly, rect_poly, Aabb, V2};
use tansk_sim::rng::Rng;

use crate::ground;
use crate::proto::{
    Event3d, Input3d, Phase, PickupKind, PickupView3d, PlayerView, Power, ProjKind, ProjView3d, Settings,
    Sound, TankView3d, View3d, Weapon, MAX_PITCH, MIN_PITCH, P3,
};
use crate::terrain::{self, Wall3d, ARENA_D, ARENA_W, ZONE_RADIUS};

/// Same tick as the 2D game: 30 ticks per second.
pub const TICK_MS: i32 = 33;
pub const MAX_PLAYERS: usize = 4;

pub const TANK_LENGTH: f32 = 63.0;
pub const TANK_WIDTH: f32 = 48.0;
pub const TANK_BOX_HEIGHT: f32 = 40.0;
pub const TURRET_PIVOT_UP: f32 = 30.0;
pub const BARREL_LENGTH: f32 = 32.0;
pub const SHIELD_RADIUS: f32 = 53.0;
pub const SHIELD_CENTER_UP: f32 = 20.0;
/// Shell gravity, px/ms².
pub const GRAVITY: f32 = 0.00025;

const MAX_HEALTH: f64 = 100.0;
const MAX_SHIELD_HEALTH: f64 = 50.0;
const TANK_MAX_SPEED: f32 = 0.07;
const TANK_MIN_SPEED: f32 = -0.04;
const TURN_SPEED: f32 = 0.15;
const TURRET_TURN_DEG_S: f32 = 200.0;
const BARREL_DEG_S: f32 = 120.0;
const TANK_RADIUS: f32 = 30.0;
const EDGE_MARGIN: f32 = 30.0;
/// tan(40°): ground steeper than this is a wall.
const MAX_CLIMB: f32 = 0.84;
const PICKUP_RADIUS: f32 = 40.0;
const SPLASH_RADIUS: f32 = 30.0;
const SECONDARY_HOVER: f32 = 6.0;
const RESTITUTION: f32 = 0.8;
const POWERUP_SPAWN_MS: i32 = 10_000;
const WEAPON_SPAWN_MS: i32 = 15_000;
const POWERUP_LIMIT: i32 = 6;
const WEAPON_LIMIT: i32 = 6;
const ROUND_DELAY_MS: i32 = 5_000;
const POWER_EFFECT_MS: i32 = 7_000;

// ------------------------------------------------------------------ vectors

fn add(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: P3, s: f32) -> P3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: P3, b: P3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn len(a: P3) -> f32 {
    dot(a, a).sqrt()
}
fn norm_deg(d: f32) -> f32 {
    let r = d.rem_euclid(360.0);
    if r >= 360.0 {
        0.0
    } else {
        r
    }
}
/// Unit vector for a yaw and pitch (degrees).
fn dir(yaw: f32, pitch: f32) -> P3 {
    let (sy, cy) = yaw.to_radians().sin_cos();
    let (sp, cp) = pitch.to_radians().sin_cos();
    [cp * cy, sp, cp * sy]
}

// -------------------------------------------------------------------- types

#[derive(Clone, Debug)]
struct Player {
    id: u32,
    name: String,
    color: u8,
    score: i32,
    lives: i32,
    respawn_timer: i32,
    respawn_time: i32,
    active: bool,
    eliminated: bool,
    elimination_counted: bool,
    input: Input3d,
    tank: Option<u32>,
    zone_time: Option<i32>,
}

#[derive(Clone, Debug)]
struct Shield {
    health: f64,
    duration: Option<i32>,
}

#[derive(Clone, Copy, Debug)]
struct ActivePower {
    kind: Power,
    left: i32,
}

fn weapon_ammo(w: Weapon) -> i32 {
    match w {
        Weapon::Default => 0,
        Weapon::Bounce => 30,
        Weapon::Flamethrower => 400,
        Weapon::Shockwave => 5,
        Weapon::Shotgun => 8,
        Weapon::Slowspeedy => 7,
    }
}

fn weapon_rate(w: Weapon) -> i32 {
    match w {
        Weapon::Default | Weapon::Bounce => 500,
        Weapon::Flamethrower => 33,
        Weapon::Shockwave => 3000,
        Weapon::Shotgun => 1000,
        Weapon::Slowspeedy => 750,
    }
}

#[derive(Clone, Debug)]
struct Tank {
    id: u32,
    player: u32,
    color: u8,
    x: f32,
    z: f32,
    yaw: f32,
    speed: f32,
    last_x: f32,
    last_z: f32,
    last_yaw: f32,
    turret_yaw: f32,
    barrel_pitch: f32,
    health: f64,
    weapon: Weapon,
    ammo: i32,
    cooldown: i32,
    shield: Option<Shield>,
    power: Option<ActivePower>,
    dead: bool,
}

impl Tank {
    fn ground(&self) -> f32 {
        ground::height(self.x, self.z)
    }
    fn pos(&self) -> P3 {
        [self.x, self.ground(), self.z]
    }
    fn poly(&self) -> [V2; 4] {
        rect_poly(V2::new(self.x, self.z), TANK_WIDTH, TANK_LENGTH, self.yaw as f64 + 90.0)
    }
    fn pivot(&self) -> P3 {
        [self.x, self.ground() + TURRET_PIVOT_UP, self.z]
    }
    fn muzzle(&self) -> P3 {
        add(self.pivot(), scale(dir(self.turret_yaw, self.barrel_pitch), BARREL_LENGTH))
    }
    fn shield_center(&self) -> P3 {
        [self.x, self.ground() + SHIELD_CENTER_UP, self.z]
    }
    /// The point in hull-local coordinates (along, up, across), clamped into the box.
    fn box_distance(&self, p: P3) -> f32 {
        let (s, c) = self.yaw.to_radians().sin_cos();
        let dx = p[0] - self.x;
        let dz = p[2] - self.z;
        let along = dx * c + dz * s;
        let across = -dx * s + dz * c;
        let up = p[1] - self.ground();
        let ea = (along.abs() - TANK_LENGTH / 2.0).max(0.0);
        let ec = (across.abs() - TANK_WIDTH / 2.0).max(0.0);
        let eu = if up < 0.0 { -up } else { (up - TANK_BOX_HEIGHT).max(0.0) };
        (ea * ea + ec * ec + eu * eu).sqrt()
    }
    fn contains(&self, p: P3) -> bool {
        self.box_distance(p) <= 0.0
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
    fn turn(&mut self, delta: i32, sign: f32) {
        let amount = TURN_SPEED * delta as f32 * (self.speed.abs() * 0.2 + 0.7);
        self.yaw = norm_deg(self.yaw + sign * amount);
    }
    fn fire_rate(&self) -> i32 {
        let r = weapon_rate(self.weapon);
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
    fn set_weapon(&mut self, w: Weapon) {
        self.weapon = w;
        self.ammo = weapon_ammo(w);
    }
}

#[derive(Clone, Debug)]
struct Projectile {
    id: u32,
    kind: ProjKind,
    /// The firing tank's id.
    owner: u32,
    pos: P3,
    prev: P3,
    vel: P3,
    /// Slowspeedy's nominal (2D) speed.
    speed: f32,
    gravity: f32,
    damage: f64,
    timer: i32,
    detonated: bool,
    parent: Option<u32>,
    dead: bool,
}

/// (muzzle speed px/ms, damage, life ms, gravity px/ms²)
fn proj_stats(kind: ProjKind) -> (f32, f64, i32, f32) {
    match kind {
        ProjKind::Default => (0.7, 5.0, 10_000, GRAVITY),
        ProjKind::Bounce => (0.5, 8.0, 3000, GRAVITY),
        ProjKind::Flame => (0.5, 2.0, 300, 0.0),
        ProjKind::Shockwave => (0.4, 0.5, 1500, GRAVITY),
        ProjKind::ShockwaveSecondary => (0.1, 10.0, 1200, 0.0),
        ProjKind::Shotgun => (0.6, 3.0, 350, GRAVITY * 0.4),
        ProjKind::Slowspeedy => (0.1, 5.0, 3000, 0.0),
    }
}

#[derive(Clone, Debug)]
struct Pickup {
    id: u32,
    kind: PickupKind,
    x: f32,
    z: f32,
}

/// The whole 3D game: world, players and rules.
pub struct Game3d {
    pub settings: Settings,
    pub started: bool,
    pub tick: u32,
    players: Vec<Player>,
    tanks: Vec<Tank>,
    projectiles: Vec<Projectile>,
    pickups: Vec<Pickup>,
    walls: Vec<Wall3d>,
    spawn_points: Vec<(f32, f32, f32)>,
    zone: Option<P3>,
    shock_hits: Vec<(u32, u32)>,
    spawn_queue: Vec<u32>,
    next_id: u32,
    rng: Rng,
    events: Vec<Event3d>,
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

impl Game3d {
    pub fn new(settings: Settings, seed: u64) -> Self {
        let settings = settings.clamped();
        Game3d {
            players: Vec::new(),
            tanks: Vec::new(),
            projectiles: Vec::new(),
            pickups: Vec::new(),
            walls: terrain::walls(),
            spawn_points: terrain::spawn_points(),
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
        }
    }

    fn gen_id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    /// Lobby only. Returns the player's id.
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
            input: Input3d::default(),
            tank: None,
            zone_time: None,
        });
        id
    }

    pub fn remove_player(&mut self, id: u32) {
        if let Some(tid) = self.player(id).and_then(|p| p.tank) {
            if let Some(t) = self.tanks.iter_mut().find(|t| t.id == tid) {
                t.dead = true;
                let pos = t.pos();
                self.events.push(Event3d::Explosion(pos));
                self.events.push(Event3d::Sound(Sound::TankDestroyed, pos));
            }
        }
        self.tanks.retain(|t| !t.dead);
        self.spawn_queue.retain(|&p| p != id);
        self.players.retain(|p| p.id != id);
    }

    fn player(&self, id: u32) -> Option<&Player> {
        self.players.iter().find(|p| p.id == id)
    }

    fn player_mut(&mut self, id: u32) -> Option<&mut Player> {
        self.players.iter_mut().find(|p| p.id == id)
    }

    pub fn set_input(&mut self, id: u32, input: Input3d) {
        if let Some(p) = self.player_mut(id) {
            p.input = input;
        }
    }

    /// Host pressed start: 5 s countdown, then round one.
    pub fn start(&mut self) {
        self.start_with_delay(5000)
    }

    pub fn start_with_delay(&mut self, delay_ms: i32) {
        if self.started {
            return;
        }
        self.started = true;
        if self.settings.koth {
            self.zone = Some(terrain::zone_center());
        }
        self.delaying = true;
        self.delay_timer = delay_ms;
    }

    /// One tick of TICK_MS.
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

    pub fn is_game_over(&self) -> bool {
        self.game_over
    }

    /// What to draw this tick; drains the tick's events.
    pub fn view(&mut self) -> View3d {
        View3d {
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
                .map(|t| {
                    let n = ground::normal(t.x, t.z);
                    let (s, c) = t.yaw.to_radians().sin_cos();
                    let f = [c, 0.0, s];
                    let r = [-s, 0.0, c];
                    let ny = n[1].max(1e-3);
                    TankView3d {
                        id: t.id,
                        player: t.player,
                        color: t.color,
                        pos: t.pos(),
                        yaw: t.yaw,
                        hull_pitch: (-dot(n, f) / ny).atan().to_degrees(),
                        hull_roll: (-dot(n, r) / ny).atan().to_degrees(),
                        turret_yaw: t.turret_yaw,
                        barrel_pitch: t.barrel_pitch,
                        health: t.health as f32,
                        shield: t.shield.as_ref().map(|s| s.health as f32),
                        weapon: t.weapon,
                        ammo: t.ammo,
                        power: t.power.map(|p| p.kind),
                    }
                })
                .collect(),
            projectiles: self
                .projectiles
                .iter()
                .map(|p| ProjView3d { id: p.id, kind: p.kind, pos: p.pos, vel: p.vel })
                .collect(),
            pickups: self
                .pickups
                .iter()
                .map(|k| PickupView3d { id: k.id, kind: k.kind, pos: [k.x, ground::height(k.x, k.z), k.z] })
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

    /// Playground: swap a player's weapon.
    pub fn give_weapon(&mut self, id: u32, weapon: Weapon) {
        let Some(tid) = self.player(id).and_then(|p| p.tank) else { return };
        if let Some(t) = self.tanks.iter_mut().find(|t| t.id == tid) {
            t.set_weapon(weapon);
        }
    }

    /// Playground: drop a pickup at ground position (x, z).
    pub fn add_pickup(&mut self, kind: PickupKind, x: f32, z: f32) {
        let id = self.gen_id();
        match kind {
            PickupKind::Weapon(_) => self.weapon_count += 1,
            PickupKind::Power(_) => self.powerup_count += 1,
        }
        self.pickups.push(Pickup { id, kind, x, z });
    }

    // --------------------------------------------------------------- spawning

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
            let free: Vec<usize> = (0..self.spawn_points.len()).filter(|&i| self.spawn_point_free(i)).collect();
            if free.is_empty() {
                keep.push(pid);
                continue;
            }
            let i = free[self.rng.below(free.len() as u32) as usize];
            self.spawn_tank_at(pid, i);
        }
        keep.append(&mut self.spawn_queue);
        self.spawn_queue = keep;
    }

    fn spawn_point_free(&self, i: usize) -> bool {
        let (x, z, _) = self.spawn_points[i];
        !self
            .tanks
            .iter()
            .any(|t| !t.dead && (t.x - x).powi(2) + (t.z - z).powi(2) < (2.0 * TANK_RADIUS + 5.0).powi(2))
    }

    fn spawn_tank_at(&mut self, pid: u32, sp: usize) {
        let id = self.gen_id();
        let (x, z, rotation) = self.spawn_points[sp];
        let color = self.player(pid).map(|p| p.color).unwrap_or(0);
        let yaw = norm_deg(rotation + 90.0);
        self.tanks.push(Tank {
            id,
            player: pid,
            color,
            x,
            z,
            yaw,
            speed: 0.0,
            last_x: x,
            last_z: z,
            last_yaw: yaw,
            turret_yaw: yaw,
            barrel_pitch: 0.0,
            health: MAX_HEALTH,
            weapon: Weapon::Default,
            ammo: 0,
            cooldown: 0,
            shield: Some(Shield { health: 9999.0, duration: Some(3000) }),
            power: None,
            dead: false,
        });
        if let Some(p) = self.player_mut(pid) {
            p.tank = Some(id);
        }
    }

    fn queue_spawn(&mut self, pid: u32) {
        // Marked dead rather than removed: callers may be iterating tanks by index.
        if let Some(tid) = self.player(pid).and_then(|p| p.tank) {
            if let Some(t) = self.tanks.iter_mut().find(|t| t.id == tid) {
                t.dead = true;
            }
        }
        let Some(p) = self.player_mut(pid) else { return };
        p.tank = None;
        p.active = true;
        p.respawn_timer = p.respawn_time;
        if !self.spawn_queue.contains(&pid) {
            self.spawn_queue.push(pid);
        }
    }

    fn update_spawner(&mut self, delta: i32) {
        if self.game_over {
            return;
        }
        self.powerup_timer -= delta;
        self.weapon_timer -= delta;
        if self.powerup_count < POWERUP_LIMIT && self.powerup_timer <= 0 {
            self.powerup_timer = POWERUP_SPAWN_MS;
            if let Some((x, z)) = self.find_pickup_spot() {
                let kind = match self.rng.below(5) {
                    1 => Power::FireRate,
                    2 => Power::Speed,
                    3 => Power::Shield,
                    4 => Power::Damage,
                    _ => Power::Health,
                };
                self.add_pickup(PickupKind::Power(kind), x, z);
            }
        }
        if self.weapon_count < WEAPON_LIMIT && self.weapon_timer <= 0 {
            self.weapon_timer = WEAPON_SPAWN_MS;
            if let Some((x, z)) = self.find_pickup_spot() {
                let kind = match self.rng.below(5) {
                    1 => Weapon::Flamethrower,
                    2 => Weapon::Shotgun,
                    3 => Weapon::Shockwave,
                    4 => Weapon::Slowspeedy,
                    _ => Weapon::Bounce,
                };
                self.add_pickup(PickupKind::Weapon(kind), x, z);
            }
        }
    }

    /// A random spot clear of spawn pads, tanks, pickups and walls, off the rim.
    fn find_pickup_spot(&mut self) -> Option<(f32, f32)> {
        let m = 60.0;
        for _ in 0..200 {
            let x = m + self.rng.f64() as f32 * (ARENA_W - 2.0 * m);
            let z = m + self.rng.f64() as f32 * (ARENA_D - 2.0 * m);
            let near = |ax: f32, az: f32, r: f32| (ax - x).powi(2) + (az - z).powi(2) < r * r;
            let blocked = self.spawn_points.iter().any(|s| near(s.0, s.1, 65.0))
                || self.tanks.iter().any(|t| near(t.x, t.z, 75.0))
                || self.pickups.iter().any(|k| near(k.x, k.z, 60.0))
                || self.walls.iter().any(|w| {
                    x > w.min[0] - 37.5 && x < w.max[0] + 37.5 && z > w.min[2] - 37.5 && z < w.max[2] + 37.5
                });
            if !blocked {
                return Some((x, z));
            }
        }
        None
    }

    // -------------------------------------------------------------- game mode

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
                self.delaying = true;
                self.delay_timer = ROUND_DELAY_MS;
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
            let newly: Vec<u32> =
                self.players.iter().filter(|p| p.eliminated && !p.elimination_counted).map(|p| p.id).collect();
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
                    (Some(t), Some(z)) => (t.x - z[0]).powi(2) + (t.z - z[2]).powi(2) < ZONE_RADIUS * ZONE_RADIUS,
                    _ => false,
                };
                if in_zone {
                    let p = self.player_mut(id).unwrap();
                    match p.zone_time {
                        None => p.zone_time = Some(0),
                        Some(t) => {
                            let t = t + delta;
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

    // ------------------------------------------------------------------ input

    fn apply_inputs(&mut self, delta: i32) {
        for pi in 0..self.players.len() {
            let input = self.players[pi].input;
            let Some(tid) = self.players[pi].tank else { continue };
            let Some(ti) = self.tanks.iter().position(|t| t.id == tid && !t.dead) else { continue };
            {
                let t = &mut self.tanks[ti];
                // Turret: toward the aim, the shortest way, at a limited rate.
                if input.aim_yaw.is_finite() {
                    let max = TURRET_TURN_DEG_S * delta as f32 / 1000.0;
                    let diff = (input.aim_yaw - t.turret_yaw + 180.0).rem_euclid(360.0) - 180.0;
                    t.turret_yaw = norm_deg(t.turret_yaw + diff.clamp(-max, max));
                }
                if input.aim_pitch.is_finite() {
                    let want = input.aim_pitch.clamp(MIN_PITCH, MAX_PITCH);
                    let max = BARREL_DEG_S * delta as f32 / 1000.0;
                    t.barrel_pitch += (want - t.barrel_pitch).clamp(-max, max);
                }
                if input.forward {
                    t.set_speed(t.speed + TANK_MAX_SPEED * 0.001 * delta as f32);
                } else if input.back {
                    t.set_speed(t.speed - TANK_MAX_SPEED * 0.001 * delta as f32);
                } else {
                    t.friction(delta);
                }
                t.last_yaw = t.yaw;
                if input.left && !input.right {
                    t.turn(delta, if input.back { 1.0 } else { -1.0 });
                }
                if input.right && !input.left {
                    t.turn(delta, if input.back { -1.0 } else { 1.0 });
                }
            }
            if input.fire {
                self.fire(ti);
            }
        }
    }

    fn fire(&mut self, ti: usize) {
        if self.tanks[ti].cooldown > 0 {
            return;
        }
        let (weapon, yaw, pitch, muzzle) = {
            let t = &self.tanks[ti];
            (t.weapon, t.turret_yaw, t.barrel_pitch, t.muzzle())
        };
        let mut reset = false;
        match weapon {
            Weapon::Default => {
                self.spawn_projectile(ti, ProjKind::Default, yaw, pitch, None);
                self.events.push(Event3d::Sound(Sound::DefaultFire, muzzle));
            }
            Weapon::Bounce => {
                if self.tanks[ti].ammo > 0 {
                    self.spawn_projectile(ti, ProjKind::Bounce, yaw, pitch, None);
                    self.tanks[ti].ammo -= 1;
                    self.events.push(Event3d::Sound(Sound::DefaultFire, muzzle));
                } else {
                    reset = true;
                }
            }
            Weapon::Flamethrower => {
                for _ in 0..2 {
                    if self.tanks[ti].ammo > 0 {
                        self.spawn_projectile(ti, ProjKind::Flame, yaw, pitch, None);
                        self.tanks[ti].ammo -= 1;
                        if self.tanks[ti].ammo % 10 == 0 {
                            self.events.push(Event3d::Sound(Sound::Flamethrower, muzzle));
                        }
                    } else {
                        reset = true;
                        break;
                    }
                }
            }
            Weapon::Shotgun => {
                if self.tanks[ti].ammo > 0 {
                    for i in -8i32..8 {
                        let spread = i as f32 * self.rng.f64() as f32 * 2.0;
                        let a = if i % 2 == 0 { yaw - spread } else { yaw + spread };
                        let p = pitch + (self.rng.f64() as f32 - 0.5) * 4.0;
                        let speed = (0.3 - i as f32 * 0.025).abs() * 2.0;
                        self.spawn_projectile(ti, ProjKind::Shotgun, a, p, Some(speed));
                    }
                    self.tanks[ti].ammo -= 1;
                    self.events.push(Event3d::Sound(Sound::Shotgun, muzzle));
                } else {
                    reset = true;
                }
            }
            Weapon::Slowspeedy => {
                if self.tanks[ti].ammo > 0 {
                    for i in 0..5 {
                        self.spawn_projectile(ti, ProjKind::Slowspeedy, yaw - 4.0 + (i * 2) as f32, pitch, None);
                    }
                    self.tanks[ti].ammo -= 1;
                    self.events.push(Event3d::Sound(Sound::SlowspeedyFire, muzzle));
                } else {
                    reset = true;
                }
            }
            Weapon::Shockwave => {
                if self.tanks[ti].ammo > 0 {
                    self.tanks[ti].ammo -= 2;
                    self.spawn_projectile(ti, ProjKind::Shockwave, yaw, pitch, None);
                } else {
                    reset = true;
                }
            }
        }
        let t = &mut self.tanks[ti];
        if reset {
            t.set_weapon(Weapon::Default);
        }
        t.cooldown = t.fire_rate();
    }

    fn spawn_projectile(&mut self, ti: usize, kind: ProjKind, yaw: f32, pitch: f32, speed: Option<f32>) {
        let id = self.gen_id();
        let t = &self.tanks[ti];
        let pos = t.muzzle();
        let (speed0, damage, life, gravity) = proj_stats(kind);
        let speed = speed.unwrap_or(speed0);
        self.projectiles.push(Projectile {
            id,
            kind,
            owner: t.id,
            pos,
            prev: pos,
            vel: scale(dir(yaw, pitch), speed),
            speed: speed / 2.0,
            gravity,
            damage: damage * t.damage_mult(),
            timer: life,
            detonated: false,
            parent: None,
            dead: false,
        });
    }

    // ------------------------------------------------------------------ world

    fn update_world(&mut self, delta: i32) {
        for ti in 0..self.tanks.len() {
            if !self.tanks[ti].dead {
                self.update_tank(ti, delta);
            }
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
        let projectiles = &self.projectiles;
        self.shock_hits.retain(|(id, _)| projectiles.iter().any(|p| p.id == *id || p.parent == Some(*id)));
    }

    fn update_tank(&mut self, ti: usize, delta: i32) {
        {
            let t = &mut self.tanks[ti];
            let (s, c) = t.yaw.to_radians().sin_cos();
            // Slower uphill.
            let e = 6.0;
            let slope = (ground::height(t.x + c * e, t.z + s * e) - ground::height(t.x - c * e, t.z - s * e)) / (2.0 * e);
            let uphill = slope * t.speed.signum();
            let factor = if uphill > 0.0 { (1.0 - 0.7 * uphill).max(0.2) } else { 1.0 };
            let step = t.speed * delta as f32 * 2.0 * factor;
            t.last_x = t.x;
            t.last_z = t.z;
            t.x += c * step;
            t.z += s * step;
            t.cooldown -= delta;

            if let Some(p) = &mut t.power {
                p.left -= delta;
                if p.left <= 0 {
                    t.power = None;
                    let m = t.speed_mult();
                    t.speed = t.speed.clamp(TANK_MIN_SPEED * m, TANK_MAX_SPEED * m);
                }
            }
            if let Some(sh) = &mut t.shield {
                if let Some(d) = &mut sh.duration {
                    *d -= delta;
                }
                if sh.health <= 0.0 || sh.duration.is_some_and(|d| d <= 0) {
                    t.shield = None;
                }
            }
        }

        // Collisions.
        let t = &self.tanks[ti];
        let moved = t.x != t.last_x || t.z != t.last_z;
        let too_steep = moved && {
            let mx = t.x - t.last_x;
            let mz = t.z - t.last_z;
            let l = (mx * mx + mz * mz).sqrt();
            let (mx, mz) = (mx / l, mz / l);
            let e = 4.0;
            (ground::height(t.x + mx * e, t.z + mz * e) - ground::height(t.x - mx * e, t.z - mz * e)) / (2.0 * e)
                > MAX_CLIMB
        };
        let out = t.x < EDGE_MARGIN || t.x > ARENA_W - EDGE_MARGIN || t.z < EDGE_MARGIN || t.z > ARENA_D - EDGE_MARGIN;
        let poly = t.poly();
        let hits_wall = self.walls.iter().any(|w| {
            let r = Aabb { x: w.min[0], y: w.min[2], w: w.max[0] - w.min[0], h: w.max[2] - w.min[2] };
            poly_poly(&r.poly(), &poly)
        });
        let hits_tank = self.tanks.iter().enumerate().any(|(j, o)| {
            if j == ti || o.dead {
                return false;
            }
            let d2 = (o.x - t.x).powi(2) + (o.z - t.z).powi(2);
            let d2_before = (o.x - t.last_x).powi(2) + (o.z - t.last_z).powi(2);
            d2 < (2.0 * TANK_RADIUS).powi(2) && d2 < d2_before
        });
        if too_steep || out || hits_wall || hits_tank {
            let t = &mut self.tanks[ti];
            if moved {
                t.x = t.last_x;
                t.z = t.last_z;
                t.set_speed(-(t.speed * 0.5));
            }
            if hits_wall || out {
                t.yaw = t.last_yaw;
            }
        }

        let (x, z) = (self.tanks[ti].x, self.tanks[ti].z);
        let mut k = 0;
        while k < self.pickups.len() {
            let pk = &self.pickups[k];
            if (pk.x - x).powi(2) + (pk.z - z).powi(2) < PICKUP_RADIUS * PICKUP_RADIUS {
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
                t.set_weapon(w);
            }
            PickupKind::Power(p) => {
                self.powerup_count -= 1;
                match p {
                    Power::Health => t.health = (t.health + 25.0).min(MAX_HEALTH),
                    Power::Shield => t.shield = Some(Shield { health: MAX_SHIELD_HEALTH, duration: None }),
                    Power::Speed | Power::FireRate | Power::Damage => {
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
                p.speed *= 1.02;
            } else if p.speed < 0.35 {
                p.speed = 0.55;
                let pos = p.pos;
                self.events.push(Event3d::Sound(Sound::SlowspeedySecondary, pos));
            }
            let p = &mut self.projectiles[pi];
            let l = len(p.vel).max(1e-6);
            p.vel = scale(p.vel, p.speed * 2.0 / l);
        }
        let wobble = match kind {
            ProjKind::Flame => (self.rng.f64() as f32 * 80.0 - 40.0).trunc(),
            ProjKind::Shotgun => (self.rng.f64() as f32 * 10.0 - 5.0).trunc(),
            _ => 0.0,
        };
        let disp = {
            let p = &mut self.projectiles[pi];
            p.timer -= delta;
            if p.timer < 0 {
                p.dead = true;
                return;
            }
            p.vel[1] -= p.gravity * delta as f32;
            let mut d = scale(p.vel, delta as f32);
            if wobble != 0.0 {
                let (s, c) = wobble.to_radians().sin_cos();
                d = [d[0] * c - d[2] * s, d[1], d[0] * s + d[2] * c];
            }
            d
        };

        if kind == ProjKind::ShockwaveSecondary {
            let p = &mut self.projectiles[pi];
            p.prev = p.pos;
            p.pos[0] += disp[0];
            p.pos[2] += disp[2];
            p.pos[1] = ground::height(p.pos[0], p.pos[2]) + SECONDARY_HOVER;
            if !in_footprint(p.pos) {
                p.dead = true;
                return;
            }
            self.collide(pi);
            return;
        }

        let n = (len(disp) / 10.0).ceil().max(1.0) as usize;
        let d = scale(disp, 1.0 / n as f32);
        for _ in 0..n {
            let p = &mut self.projectiles[pi];
            p.prev = p.pos;
            p.pos = add(p.pos, d);
            if !in_footprint(p.pos) {
                p.dead = true;
                return;
            }
            if self.collide(pi) {
                return;
            }
        }
    }

    /// Collisions at the projectile's current point. True: stop moving it this tick.
    fn collide(&mut self, pi: usize) -> bool {
        let (kind, owner, pos, prev) = {
            let p = &self.projectiles[pi];
            (p.kind, p.owner, p.pos, p.prev)
        };

        // Enemy shields.
        for ti in 0..self.tanks.len() {
            let t = &self.tanks[ti];
            if t.dead || t.id == owner || t.shield.is_none() {
                continue;
            }
            if len(sub(pos, t.shield_center())) < SHIELD_RADIUS {
                if kind == ProjKind::Shockwave && !self.projectiles[pi].detonated {
                    self.detonate(pi);
                }
                let dmg = self.projectiles[pi].damage;
                self.damage_shield(ti, dmg);
                self.projectiles[pi].dead = true;
                return true;
            }
        }

        // Terrain.
        let h = ground::height(pos[0], pos[2]);
        if pos[1] <= h {
            match kind {
                ProjKind::Bounce => {
                    let n = ground::normal(pos[0], pos[2]);
                    let p = &mut self.projectiles[pi];
                    let vn = dot(p.vel, n);
                    if vn < 0.0 {
                        p.vel = scale(sub(p.vel, scale(n, 2.0 * vn)), RESTITUTION);
                    }
                    p.pos = prev;
                    let hp = ground::height(prev[0], prev[2]);
                    if p.pos[1] <= hp {
                        p.pos[1] = hp + 0.5;
                    }
                    if vn < -0.05 {
                        self.events.push(Event3d::Impact([pos[0], h, pos[2]]));
                    }
                }
                ProjKind::Shockwave => {
                    self.projectiles[pi].pos[1] = h;
                    self.detonate(pi);
                }
                ProjKind::Flame | ProjKind::Shotgun | ProjKind::ShockwaveSecondary => {
                    self.projectiles[pi].dead = true;
                }
                ProjKind::Default | ProjKind::Slowspeedy => {
                    let at = [pos[0], h, pos[2]];
                    self.events.push(Event3d::Impact(at));
                    self.projectiles[pi].dead = true;
                    self.splash(pi, at);
                }
            }
            return true;
        }

        // Walls.
        if let Some(w) = self.walls.iter().find(|w| w.contains(pos)).copied() {
            match kind {
                ProjKind::Bounce => {
                    // Reflect off the face it came through.
                    let mut axis = 0;
                    let mut best = f32::NEG_INFINITY;
                    for i in 0..3 {
                        let outside = (w.min[i] - prev[i]).max(prev[i] - w.max[i]);
                        if outside > best {
                            best = outside;
                            axis = i;
                        }
                    }
                    let p = &mut self.projectiles[pi];
                    p.vel[axis] = -p.vel[axis];
                    p.vel = scale(p.vel, RESTITUTION);
                    p.pos = prev;
                    self.events.push(Event3d::Impact(pos));
                }
                ProjKind::Shockwave => self.detonate(pi),
                ProjKind::Flame | ProjKind::ShockwaveSecondary => self.projectiles[pi].dead = true,
                ProjKind::Shotgun => {
                    self.events.push(Event3d::Impact(pos));
                    self.projectiles[pi].dead = true;
                }
                ProjKind::Default | ProjKind::Slowspeedy => {
                    self.events.push(Event3d::Impact(pos));
                    self.projectiles[pi].dead = true;
                    self.splash(pi, pos);
                }
            }
            return true;
        }

        // Tanks.
        for ti in 0..self.tanks.len() {
            if self.tanks[ti].dead || !self.tanks[ti].contains(pos) {
                continue;
            }
            let own = self.tanks[ti].id == owner;
            match kind {
                ProjKind::Shockwave => {
                    if !own {
                        self.detonate(pi);
                        return true;
                    }
                }
                ProjKind::ShockwaveSecondary => {
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
                    if hits <= 5 {
                        let dmg = self.projectiles[pi].damage;
                        self.damage_tank(ti, dmg, kind);
                    }
                    self.projectiles[pi].dead = true;
                    return true;
                }
                _ => {
                    if own {
                        continue;
                    }
                    let dmg = self.projectiles[pi].damage;
                    self.damage_tank(ti, dmg, kind);
                    self.projectiles[pi].dead = true;
                    return true;
                }
            }
        }
        false
    }

    /// Half damage to enemy hulls within SPLASH_RADIUS of an impact.
    fn splash(&mut self, pi: usize, at: P3) {
        let (owner, dmg, kind) = {
            let p = &self.projectiles[pi];
            (p.owner, p.damage * 0.5, p.kind)
        };
        for ti in 0..self.tanks.len() {
            let t = &self.tanks[ti];
            if t.dead || t.id == owner || t.box_distance(at) > SPLASH_RADIUS {
                continue;
            }
            self.damage_tank(ti, dmg, kind);
        }
    }

    /// A ring of 40 slow, ground-hugging, hard-hitting bits.
    fn detonate(&mut self, pi: usize) {
        let (pos, vel, owner, id) = {
            let p = &mut self.projectiles[pi];
            p.detonated = true;
            p.dead = true;
            (p.pos, p.vel, p.owner, p.id)
        };
        self.events.push(Event3d::Sound(Sound::ShockwaveDetonate, pos));
        self.events.push(Event3d::Explosion(pos));
        let base = vel[2].atan2(vel[0]).to_degrees();
        let (speed, damage, life, _) = proj_stats(ProjKind::ShockwaveSecondary);
        let start = [pos[0], ground::height(pos[0], pos[2]) + SECONDARY_HOVER, pos[2]];
        for i in 0..40 {
            let nid = self.gen_id();
            self.projectiles.push(Projectile {
                id: nid,
                kind: ProjKind::ShockwaveSecondary,
                owner,
                pos: start,
                prev: start,
                vel: scale(dir(base + (i * 9) as f32, 0.0), speed),
                speed: speed / 2.0,
                gravity: 0.0,
                damage,
                timer: life,
                detonated: false,
                parent: Some(id),
                dead: false,
            });
        }
    }

    fn damage_tank(&mut self, ti: usize, dmg: f64, kind: ProjKind) {
        if kind != ProjKind::Flame {
            let pos = self.tanks[ti].pos();
            self.events.push(Event3d::Sound(Sound::TankHit, pos));
        }
        if self.tanks[ti].shield.as_ref().is_some_and(|s| s.health > 0.0) {
            self.damage_shield(ti, dmg);
        } else {
            self.tanks[ti].health -= dmg;
            self.check_death(ti);
        }
    }

    /// What the shield can't take, the hull does.
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

    fn check_death(&mut self, ti: usize) {
        let t = &mut self.tanks[ti];
        if t.dead || t.health > 0.0 {
            return;
        }
        t.dead = true;
        let (pos, pid) = (t.pos(), t.player);
        self.events.push(Event3d::Sound(Sound::TankDestroyed, pos));
        self.events.push(Event3d::Explosion(pos));
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
}

fn in_footprint(p: P3) -> bool {
    p[0] >= 0.0 && p[0] <= ARENA_W && p[2] >= 0.0 && p[2] <= ARENA_D
}

#[cfg(test)]
mod tests;
