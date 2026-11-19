//! Bot crews: sight-based targeting, coarse-grid navigation, leading shots,
//! scrap hunting and branch picks. Bots produce an [`Input`] per tick, like a
//! player, so they obey the same physics and rules.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use glam::{Vec2, Vec3};

use crate::events::PickupKind;
use crate::map::Map;
use crate::parts::{AltKind, GunKind, PartKind, VehicleKind};
use crate::rng::Rng;
use crate::sim::Sim;
use crate::tank::{gun_stats, HullPose};
use crate::Input;

pub const BOT_NAMES: [&str; 12] = [
    "Rusty", "Gravel", "Dusty", "Sump", "Bolt", "Clank", "Mirage", "Cinder", "Jerrycan", "Sprocket", "Grit", "Weld",
];

#[derive(Clone, Debug)]
pub struct NavGrid {
    pub cell: f32,
    pub n: usize,
    pub origin: f32,
    pub blocked: Vec<bool>,
}

#[derive(Copy, Clone, PartialEq)]
struct Node {
    f: f32,
    i: usize,
}
impl Eq for Node {}
impl Ord for Node {
    fn cmp(&self, o: &Self) -> Ordering {
        o.f.total_cmp(&self.f).then(self.i.cmp(&o.i))
    }
}
impl PartialOrd for Node {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl NavGrid {
    pub fn build(map: &Map, cell: f32) -> NavGrid {
        let n = (map.size / cell).ceil() as usize;
        let origin = -map.size / 2.0;
        let mut blocked = vec![false; n * n];
        for z in 0..n {
            for x in 0..n {
                let c = Vec2::new(origin + (x as f32 + 0.5) * cell, origin + (z as f32 + 0.5) * cell);
                // Steep anywhere in the cell (corners and centre) counts.
                let q = cell * 0.5;
                let steep = [(0.0, 0.0), (-q, -q), (q, -q), (-q, q), (q, q)]
                    .iter()
                    .any(|(dx, dz)| map.normal_at(c.x + dx, c.y + dz).y < 0.85);
                blocked[z * n + x] = steep || map.blocked_xz(c, 2.8);
            }
        }
        NavGrid { cell, n, origin, blocked }
    }

    fn cell_of(&self, p: Vec2) -> (usize, usize) {
        let x = ((p.x - self.origin) / self.cell).floor().clamp(0.0, (self.n - 1) as f32) as usize;
        let z = ((p.y - self.origin) / self.cell).floor().clamp(0.0, (self.n - 1) as f32) as usize;
        (x, z)
    }

    fn center(&self, x: usize, z: usize) -> Vec2 {
        Vec2::new(self.origin + (x as f32 + 0.5) * self.cell, self.origin + (z as f32 + 0.5) * self.cell)
    }

    pub fn is_blocked(&self, p: Vec2) -> bool {
        let (x, z) = self.cell_of(p);
        self.blocked[z * self.n + x]
    }

    fn nearest_free(&self, c: (usize, usize)) -> (usize, usize) {
        if !self.blocked[c.1 * self.n + c.0] {
            return c;
        }
        for r in 1..8i32 {
            for dz in -r..=r {
                for dx in -r..=r {
                    let x = c.0 as i32 + dx;
                    let z = c.1 as i32 + dz;
                    if x >= 0 && z >= 0 && (x as usize) < self.n && (z as usize) < self.n && !self.blocked[z as usize * self.n + x as usize]
                    {
                        return (x as usize, z as usize);
                    }
                }
            }
        }
        c
    }

    fn line_clear(&self, a: Vec2, b: Vec2) -> bool {
        let d = b - a;
        let steps = (d.length() / (self.cell * 0.4)).ceil().max(1.0) as usize;
        (0..=steps).all(|k| !self.is_blocked(a + d * (k as f32 / steps as f32)))
    }

    /// Is there a drivable route (on the nav grid) from `from` to `to`?
    pub fn reachable(&self, from: Vec2, to: Vec2) -> bool {
        let n = self.n;
        let s = self.nearest_free(self.cell_of(from));
        let g = self.nearest_free(self.cell_of(to));
        let (si, gi) = (s.1 * n + s.0, g.1 * n + g.0);
        let mut seen = vec![false; n * n];
        let mut stack = vec![si];
        seen[si] = true;
        while let Some(i) = stack.pop() {
            if i == gi {
                return true;
            }
            let (x, z) = ((i % n) as i32, (i / n) as i32);
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, nz) = (x + dx, z + dz);
                if nx < 0 || nz < 0 || nx >= n as i32 || nz >= n as i32 {
                    continue;
                }
                let j = nz as usize * n + nx as usize;
                if !seen[j] && !self.blocked[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        false
    }

    /// A* over the grid, then string-pulled. Returns waypoints (excluding start).
    pub fn path(&self, from: Vec2, to: Vec2) -> Vec<Vec2> {
        let n = self.n;
        let s = self.nearest_free(self.cell_of(from));
        let g = self.nearest_free(self.cell_of(to));
        let si = s.1 * n + s.0;
        let gi = g.1 * n + g.0;
        if si == gi {
            return vec![to];
        }
        let mut cost = vec![f32::INFINITY; n * n];
        let mut came = vec![usize::MAX; n * n];
        let mut open = BinaryHeap::new();
        cost[si] = 0.0;
        let h = |i: usize| -> f32 {
            let dx = (i % n) as f32 - g.0 as f32;
            let dz = (i / n) as f32 - g.1 as f32;
            let (a, b) = (dx.abs(), dz.abs());
            a.max(b) + 0.414 * a.min(b)
        };
        open.push(Node { f: h(si), i: si });
        let mut found = false;
        let mut expansions = 0;
        while let Some(Node { i, .. }) = open.pop() {
            if i == gi {
                found = true;
                break;
            }
            expansions += 1;
            if expansions > 40_000 {
                break;
            }
            let (x, z) = ((i % n) as i32, (i / n) as i32);
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
                let nx = x + dx;
                let nz = z + dz;
                if nx < 0 || nz < 0 || nx >= n as i32 || nz >= n as i32 {
                    continue;
                }
                let j = nz as usize * n + nx as usize;
                if self.blocked[j] {
                    continue;
                }
                if dx != 0 && dz != 0 && (self.blocked[z as usize * n + nx as usize] || self.blocked[nz as usize * n + x as usize]) {
                    continue;
                }
                let step = if dx != 0 && dz != 0 { 1.414 } else { 1.0 };
                let c = cost[i] + step;
                if c < cost[j] {
                    cost[j] = c;
                    came[j] = i;
                    open.push(Node { f: c + h(j), i: j });
                }
            }
        }
        if !found {
            return vec![to];
        }
        let mut cells = vec![gi];
        let mut cur = gi;
        while came[cur] != usize::MAX && cur != si {
            cur = came[cur];
            cells.push(cur);
        }
        cells.reverse();
        let pts: Vec<Vec2> = cells.iter().map(|&i| self.center(i % n, i / n)).collect();
        // String pulling.
        let mut out = Vec::new();
        let mut anchor = from;
        let mut k = 0;
        while k < pts.len() {
            let mut far = k;
            for m in (k..pts.len()).rev() {
                if self.line_clear(anchor, pts[m]) {
                    far = m;
                    break;
                }
            }
            out.push(pts[far]);
            anchor = pts[far];
            k = far + 1;
        }
        if !self.is_blocked(to) {
            if let Some(last) = out.last_mut() {
                *last = to;
            }
        }
        out
    }
}

#[derive(Clone, Debug)]
pub struct Brain {
    pub difficulty: f32,
    rng: Rng,
    path: Vec<Vec2>,
    goal: Option<Vec2>,
    goal_age: f32,
    repath: f32,
    think: f32,
    target: Option<u8>,
    last_seen: Option<(Vec3, Vec3, f32)>,
    stuck: f32,
    reverse: f32,
    reverse_steer: f32,
    strafe: f32,
    strafe_t: f32,
    noise: Vec3,
    noise_t: f32,
    chose: bool,
    style: Style,
}

impl Brain {
    pub fn new(seed: u64, difficulty: f32) -> Brain {
        Brain {
            difficulty: difficulty.clamp(0.0, 1.0),
            rng: Rng::new(seed),
            path: Vec::new(),
            goal: None,
            goal_age: 0.0,
            repath: 0.0,
            think: 0.0,
            target: None,
            last_seen: None,
            stuck: 0.0,
            reverse: 0.0,
            reverse_steer: 1.0,
            strafe: 1.0,
            strafe_t: 0.0,
            noise: Vec3::ZERO,
            noise_t: 0.0,
            chose: false,
            style: Style::ALL[(seed % 4) as usize],
        }
    }

    pub fn style(&self) -> Style {
        self.style
    }
}

/// A bot crew's personality: how it picks branches, what range it likes,
/// how greedy it is for scrap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// Gets close: Twin Barrel / Big Bore, Bulwark, Tesla, Afterburner.
    Brawler,
    /// Hangs back: Long Gun, Coilgun, Spaced Armour, Twin Engines.
    Sniper,
    /// Scrap first, fights second; picks branches at random.
    Scavenger,
    /// Fast and flanky: Twin Barrel, Twin Engines, Tesla, Spaced Armour.
    Speedster,
}

impl Style {
    pub const ALL: [Style; 4] = [Style::Brawler, Style::Sniper, Style::Scavenger, Style::Speedster];

    fn preferred(self) -> &'static [PartKind] {
        use PartKind::*;
        match self {
            Style::Brawler => &[TwinBarrel, BigBore, Bulwark, TeslaSpire, Afterburner],
            Style::Sniper => &[LongGun, Coilgun, SpacedArmour, TwinEngines],
            Style::Scavenger => &[],
            Style::Speedster => &[TwinBarrel, TwinEngines, TeslaSpire, SpacedArmour, Afterburner],
        }
    }

    /// The chassis this style takes from the Garage.
    pub fn chassis(self) -> crate::garage::ChassisClass {
        use crate::garage::ChassisClass::*;
        match self {
            Style::Brawler => Hauler,
            Style::Sniper => Brawler,
            Style::Scavenger => Hauler,
            Style::Speedster => Scout,
        }
    }

    fn range(self, base: f32) -> f32 {
        match self {
            Style::Brawler => base * 0.55,
            Style::Sniper => base * 2.2,
            Style::Scavenger => base,
            Style::Speedster => base * 0.8,
        }
    }
}


fn wrap(a: f32) -> f32 {
    let mut a = a % std::f32::consts::TAU;
    if a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    if a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}

fn flat(v: Vec3) -> Vec2 {
    Vec2::new(v.x, v.z)
}

/// One tick of a bot's decisions for player index `me`.
pub fn think(sim: &Sim, me: usize, b: &mut Brain, dt: f32) -> Input {
    let t = &sim.tuning;
    let bt = &t.bots;
    let p = &sim.players[me];
    let tank = &p.tank;
    if !tank.alive {
        b.path.clear();
        b.goal = None;
        b.target = None;
        b.last_seen = None;
        b.chose = false;
        return Input::default();
    }
    let hp = HullPose::of(&sim.phys, tank.body);
    let pos = hp.pos;
    let fwd = hp.rot * Vec3::X;
    let vel = sim.phys.body(tank.body).linvel();
    let eye = tank.eye(&hp);
    let mut input = Input::default();
    let outpost = sim.raid.as_ref().map(|r| r.outpost);
    let raider = outpost.is_some() && p.team == Some(crate::raid::TEAM_RAIDERS);

    // Branch pick: bots choose at random after a short "thinking" pause.
    if let Some(c) = &tank.choice {
        if !b.chose && c.time_left < t.upgrades.branch_choice_time - 1.0 {
            // Style preference most of the time, any option otherwise.
            let pref = c.options.iter().position(|o| b.style.preferred().contains(o));
            let pick = match pref {
                Some(k) if b.rng.chance(0.75) => k,
                _ => b.rng.below(c.options.len()),
            };
            input.choice = Some(pick as u8);

            b.chose = true;
        }
    } else {
        b.chose = false;
    }

    // --- Perception (every 0.2 s).
    b.think -= dt;
    let rethink = b.think <= 0.0;
    if rethink {
        b.think = 0.2;
        let mut best: Option<(f32, u8, Vec3, Vec3)> = None;
        for q in &sim.players {
            if q.id == p.id || !q.tank.alive || (p.team.is_some() && q.team == p.team) {
                continue;
            }
            let qhp = HullPose::of(&sim.phys, q.tank.body);
            let d = qhp.pos.distance(pos);
            let storm = 1.0 + (t.storm.sight_mult - 1.0) * sim.storm.intensity;
            if d > bt.sight_range * storm {
                continue;
            }
            // Sight to the hull or the turret top (the no-peeking rule: walls block, low cover doesn't).
            let seen = sim.sight_clear(eye, qhp.pos + Vec3::Y * 0.3) || sim.sight_clear(eye, qhp.pos + Vec3::Y * 1.5);
            if !seen {
                continue;
            }
            let mut score = d;
            if tank.last_attacker == Some(q.id) {
                score -= 40.0;
            }
            if b.target == Some(q.id) {
                score -= 25.0;
            }
            score -= (q.tank.stats.max_health - q.tank.health) * 0.05;
            if best.is_none_or(|x| score < x.0) {
                best = Some((score, q.id, qhp.pos, sim.phys.body(q.tank.body).linvel()));
            }
        }
        match best {
            Some((_, id, tp, tv)) => {
                b.target = Some(id);
                b.last_seen = Some((tp, tv, 0.0));
            }
            None => {
                if let Some(ls) = b.last_seen.as_mut() {
                    ls.2 += 0.2;
                    if ls.2 > bt.memory {
                        b.last_seen = None;
                        b.target = None;
                    }
                }
            }
        }
        // Keep fresh positions for a visible target.
        if let (Some(id), Some(ls)) = (b.target, b.last_seen.as_mut()) {
            if ls.2 == 0.0 {
                if let Some(q) = sim.players.iter().find(|q| q.id == id) {
                    ls.0 = sim.phys.body(q.tank.body).translation();
                    ls.1 = sim.phys.body(q.tank.body).linvel();
                }
            }
        }

        // --- Goal.
        b.strafe_t -= 0.2;
        if b.strafe_t <= 0.0 {
            b.strafe = if b.rng.chance(0.5) { 1.0 } else { -1.0 };
            b.strafe_t = b.rng.range(3.0, 7.0);
        }
        let health_frac = tank.health / tank.stats.max_health;
        let mut goal: Option<Vec2> = None;
        if raider {
            // Raiders press on the outpost; they turn on defenders that get close.
            let o = flat(outpost.unwrap());
            goal = Some(o);
            if let Some((tp, _, age)) = b.last_seen {
                let d = tp.distance(pos);
                let chase = match tank.vehicle {
                    VehicleKind::Buggy => 30.0,
                    VehicleKind::WarRig => if tank.stats.gun == GunKind::RigCannons { 0.0 } else { 60.0 },
                    _ => 70.0,
                };
                if age < 0.5 && d < chase {
                    goal = Some(flat(tp));
                }
            }
        }
        if health_frac < bt.repair_threshold || tank.tracks.iter().any(|s| s.blown > 0.0) {
            goal = sim
                .pickups
                .iter()
                .filter(|k| k.kind == PickupKind::Repair)
                .map(|k| flat(k.pos))
                .filter(|k| k.distance(flat(pos)) < 260.0)
                .min_by(|a, c| a.distance(flat(pos)).total_cmp(&c.distance(flat(pos))));
        }
        // Scrap: prefer kinds that complete a tier.
        let mut scrap: Option<(f32, Vec2, f32)> = None;
        for k in &sim.pickups {
            let d = flat(k.pos).distance(flat(pos));
            let scav = b.style == Style::Scavenger;
            if d > bt.scrap_range + if scav { 60.0 } else { 0.0 } {
                continue;
            }
            if let Some(o) = outpost {
                // Defenders stay near home.
                if flat(k.pos).distance(flat(o)) > 90.0 {
                    continue;
                }
            }
            let bonus = if scav { 30.0 } else { 0.0 } + match k.kind {
                PickupKind::Scrap(s) => {
                    if tank.tiers[s.index()] >= t.upgrades.max_tier {
                        continue;
                    }
                    tank.scrap[s.index()] as f32 * 25.0
                }
                PickupKind::Repair => {
                    if health_frac > 0.85 {
                        continue;
                    }
                    20.0
                }
                PickupKind::Rare(kind) => {
                    if tank.parts.iter().any(|p| p.kind == kind) {
                        continue;
                    }
                    90.0
                }
                PickupKind::AirStrike => {
                    if tank.air_strike {
                        continue;
                    }
                    120.0
                }
            };
            let score = d - bonus;
            if scrap.is_none_or(|x| score < x.0) {
                scrap = Some((score, flat(k.pos), d));
            }
        }
        if goal.is_none() {
            // Grab scrap close by even mid-fight.
            let grab = if b.style == Style::Scavenger { 80.0 } else { 50.0 };
            goal = scrap.filter(|s| s.2 < grab).map(|s| s.1);
        }
        if goal.is_none() {
            if let Some((tp, _, age)) = b.last_seen {
                let d = tp.distance(pos);
                let leash = outpost.is_none_or(|o| flat(tp).distance(flat(o)) < 110.0);
                let reach = if b.style == Style::Sniper { bt.engage_range + 40.0 } else { bt.engage_range };
                if age < 0.5 && d < reach && leash {
                    // Orbit at the preferred range, swapping sides now and then.
                    let away = flat(pos - tp).normalize_or(Vec2::X);
                    let ang = 0.7 * b.strafe;
                    let rot = Vec2::new(ang.cos(), ang.sin());
                    let dir = Vec2::new(away.x * rot.x - away.y * rot.y, away.x * rot.y + away.y * rot.x);
                    goal = Some(flat(tp) + dir * b.style.range(bt.preferred_range));
                } else if age >= 0.5 && leash {
                    goal = Some(flat(tp));
                }
            }
        }
        if goal.is_none() {
            goal = scrap.map(|s| s.1);
        }
        if goal.is_none() {
            if let Some(o) = outpost {
                // Defenders with nothing to do hold near the outpost.
                let o = flat(o);
                if flat(pos).distance(o) > 40.0 {
                    goal = Some(o);
                } else if b.goal.is_none_or(|g| g.distance(flat(pos)) < 8.0 || b.goal_age > 12.0) {
                    let a = b.rng.range(0.0, std::f32::consts::TAU);
                    goal = Some(o + Vec2::new(a.cos(), a.sin()) * b.rng.range(18.0, 34.0));
                } else {
                    goal = b.goal;
                }
            }
        }
        if goal.is_none() {
            let keep = b.goal.filter
(|g| g.distance(flat(pos)) > 12.0 && b.goal_age < 30.0);
            goal = keep.or_else(|| {
                let s = &sim.map.scrap_sites[b.rng.below(sim.map.scrap_sites.len().max(1))];
                Some(flat(s.pos))
            });
        }
        let changed = match (b.goal, goal) {
            (Some(a), Some(c)) => a.distance(c) > 10.0,
            (None, Some(_)) => true,
            _ => false,
        };
        if changed {
            b.goal_age = 0.0;
        }
        b.goal = goal;
        b.repath -= 0.2;
        if changed || b.repath <= 0.0 {
            if let Some(g) = b.goal {
                b.path = sim.nav.path(flat(pos), g);
            }
            b.repath = bt.repath_interval;
        }
    }
    b.goal_age += dt;

    // --- Steering.
    while let Some(w) = b.path.first() {
        if w.distance(flat(pos)) < 6.0 && b.path.len() > 1 {
            b.path.remove(0);
        } else {
            break;
        }
    }
    let speed = vel.length();
    let hull_yaw = (-fwd.z).atan2(fwd.x);
    if let Some(&w) = b.path.first() {
        let to = w - flat(pos);
        let dist = to.length();
        let want = (-to.y).atan2(to.x);
        let err = wrap(want - hull_yaw);
        input.steer = (-err * 2.2).clamp(-1.0, 1.0);
        input.throttle = if err.abs() > 1.1 { 0.0 } else { err.cos().max(0.25) };
        if b.path.len() == 1 && dist < 8.0 {
            input.throttle = 0.0;
            input.steer = 0.0;
        }
        // Make room for other tanks in front.
        for q in &sim.players {
            if q.id == p.id || !q.tank.alive {
                continue;
            }
            let rel = sim.phys.body(q.tank.body).translation() - pos;
            let ahead = rel.dot(fwd);
            let side = rel.dot(hp.rot * Vec3::Z);
            if ahead > 0.0 && ahead < 14.0 && side.abs() < 5.0 {
                input.steer = if side > 0.0 { -1.0 } else { 1.0 };
                input.throttle *= 0.5;
            }
        }
        // Boost on long straight runs.
        let thirst = if b.style == Style::Speedster { 0.3 } else { 0.6 };
        input.boost = (b.last_seen.is_none() || b.style == Style::Speedster) && err.abs() < 0.2 && dist > 50.0 && tank.fuel > tank.stats.fuel_max * thirst;
        if raider && tank.vehicle == VehicleKind::Buggy {
            input.throttle = input.throttle.max(0.6);
        }
    }

    // Stuck: pushing but not moving -> back off and swing.
    if b.reverse > 0.0 {
        b.reverse -= dt;
        input.throttle = -1.0;
        input.steer = b.reverse_steer;
        input.boost = false;
    } else if input.throttle > 0.4 && speed < 0.8 {
        b.stuck += dt;
        if b.stuck > 1.5 {
            b.stuck = 0.0;
            b.reverse = b.rng.range(1.0, 1.8);
            b.reverse_steer = if b.rng.chance(0.5) { 1.0 } else { -1.0 };
            b.repath = 0.0;
        }
    } else {
        b.stuck = (b.stuck - dt).max(0.0);
    }

    // --- Aim and fire.
    let g = gun_stats(t, &tank.stats);
    b.noise_t -= dt;
    if b.noise_t <= 0.0 {
        b.noise_t = b.rng.range(0.4, 1.0);
        let mut err = bt.aim_error_deg.to_radians() * (1.6 - 1.2 * b.difficulty);
        if raider {
            err *= t.raid.raider_aim_error_mult;
        }
        if tank.vehicle == VehicleKind::WarRig && !tank.parts.iter().any(|q| q.kind == PartKind::RigMast) {
            // Mast shot off: firing blind.
            err = t.raid.rig_blind_error_deg.to_radians();
        }
        b.noise = Vec3::new(b.rng.range(-1.0, 1.0), b.rng.range(-0.4, 0.4), b.rng.range(-1.0, 1.0)) * err;
    }
    input.aim_point = pos + fwd * 60.0 + Vec3::Y;
    let can_gun = tank.vehicle != VehicleKind::Buggy && !(tank.vehicle == VehicleKind::WarRig && tank.stats.gun != GunKind::RigCannons);
    if raider && b.last_seen.is_none_or(|l| l.2 >= 0.3) {
        // Nobody to shoot: shell the outpost.
        if let Some(o) = outpost {
            let aim = o + Vec3::Y * 1.0 + b.noise * o.distance(pos);
            input.aim_point = aim;
            let d = o.distance(pos);
            let (wy, wp) = tank.aim_solution(&hp, t, aim, g.shell_speed, 9.81 * g.gravity_scale);
            let tol = bt.fire_tolerance_deg.to_radians() * 2.0;
            let aligned = wrap(wy - tank.turret_yaw).abs() < tol && (wp - tank.gun_pitch).abs() < tol;
            if can_gun && aligned && d < g.shell_speed * g.lifetime * 0.8 {
                input.fire = true;
            }
        }
    }
    if let Some((tp, tv, age)) = b.last_seen {
        let mut aim = tp + Vec3::Y * 0.4;
        if age < 0.3 {
            // Lead: time of flight from distance, iterated.
            let mut tof = aim.distance(pos) / g.shell_speed;
            for _ in 0..2 {
                aim = tp + Vec3::Y * 0.4 + tv * tof;
                tof = aim.distance(pos) / g.shell_speed;
            }
        }
        let d = aim.distance(pos);
        aim += b.noise * d;
        input.aim_point = aim;
        let (wy, wp) = tank.aim_solution(&hp, t, aim, g.shell_speed, 9.81 * g.gravity_scale);
        let tol = bt.fire_tolerance_deg.to_radians();
        let aligned = wrap(wy - tank.turret_yaw).abs() < tol && (wp - tank.gun_pitch).abs() < tol * 1.5;
        let in_range = d < g.shell_speed * g.lifetime * 0.85;
        let visible = age < 0.3;
        let muzzle = tank.muzzle(&hp, t, 0.0);
        if can_gun && aligned && in_range && visible && sim.sight_clear(muzzle, tp + Vec3::Y * 0.5) {
            input.fire = true;
        }
        if visible {
            match tank.stats.alt {
                AltKind::Mg => input.alt_fire = d < 110.0 && wrap(wy - tank.turret_yaw).abs() < 0.15,
                AltKind::Coilgun => input.alt_fire = !(tank.charge >= 1.0 && aligned),
                AltKind::Tesla => input.alt_fire = d < t.alt.tesla_range,
                AltKind::Flamethrower => input.alt_fire = d < t.special.flame_range + 2.0,
                AltKind::Rockets => input.alt_fire = tank.lock_progress >= 1.0 && !tank.prev_alt,
            }
            // The special button: air strike, then the rare part.
            let s = &t.special;
            let health_frac = tank.health / tank.stats.max_health;
            let want = if tank.air_strike {
                d > 35.0 && d < s.strike_max_range
            } else {
                match tank.stats.rare {
                    Some(PartKind::Mortar) => d > 45.0 && d < 260.0,
                    Some(PartKind::Harpoon) => d > 15.0 && d < s.harpoon_range * 0.9 && aligned,
                    Some(PartKind::SmokeLauncher) => health_frac < 0.5 && tank.since_damage < 1.0,
                    Some(PartKind::MineLayer) => d < 45.0,
                    _ => false,
                }
            };
            input.special = want && !tank.prev_special;
        }
    }
    input
}
