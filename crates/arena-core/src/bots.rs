//! Bot crews: sight-based targeting, coarse-grid navigation, leading shots,
//! scrap hunting and branch picks. Bots produce an [`Input`] per tick, like a
//! player, so they obey the same physics and rules.
//!
//! They try to play like people: they notice a new enemy after a reaction
//! delay and their first shots are the loosest, they stick with a target for
//! a while, they peek from behind tall cover and fall back behind it to
//! reload, sit hull-down behind low cover, swing round to a flank, turn on
//! whoever shoots them in the back, go for a wrench when hurt, don't all race
//! for the same pickup, gang up on one enemy in team modes, and honk after a
//! kill. How well they do all that is the difficulty ([`Difficulty`]).

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

use glam::{Vec2, Vec3};

use crate::events::PickupKind;
use crate::map::{Cover, Map};
use crate::parts::{AltKind, GunKind, PartKind, VehicleKind};
use crate::rng::Rng;
use crate::sim::Sim;
use crate::tank::{gun_stats, HullPose};
use crate::Input;

pub const BOT_NAMES: [&str; 12] = [
    "Rusty", "Gravel", "Dusty", "Sump", "Bolt", "Clank", "Mirage", "Cinder", "Jerrycan", "Sprocket", "Grit", "Weld",
];

// ------------------------------------------------------------------ nav grid

/// A piece of cover a bot can use: the footprint of a map obstacle.
#[derive(Clone, Copy, Debug)]
pub struct CoverSite {
    /// Footprint centre (ground plane) and radius.
    pub pos: Vec2,
    pub radius: f32,
    /// Tall cover hides a tank; low cover only its hull (hull-down).
    pub tall: bool,
}

#[derive(Clone, Debug)]
pub struct NavGrid {
    pub cell: f32,
    pub n: usize,
    pub origin: f32,
    pub blocked: Vec<bool>,
    /// Free cells the bots' paths keep out of when they can: close to the
    /// foot of a steep slope, or in a lane one cell wide.
    pub avoid: Vec<bool>,
    /// The map's cover (obstacles that block shots), for the bots' cover search.
    pub covers: Vec<CoverSite>,
}

#[derive(Copy, Clone, PartialEq)]
struct Node {
    f: f32,
    i: u32,
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

/// A* scratch, reused between searches (per thread: the server runs rooms on
/// several). `stamp[i] == gen` marks the cells touched by the current search,
/// so nothing is cleared between searches.
#[derive(Default)]
struct Scratch {
    cost: Vec<f32>,
    came: Vec<u32>,
    stamp: Vec<u32>,
    gen: u32,
    open: BinaryHeap<Node>,
    cells: Vec<u32>,
}

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default());
}

/// Cells string pulling looks ahead (bounds the cost of long paths).
const PULL_LOOKAHEAD: usize = 28;

impl NavGrid {
    pub fn build(map: &Map, cell: f32) -> NavGrid {
        let n = (map.size / cell).ceil() as usize;
        let origin = -map.size / 2.0;
        let mut blocked = vec![false; n * n];
        let mut steep = vec![false; n * n];
        for z in 0..n {
            for x in 0..n {
                let c = Vec2::new(origin + (x as f32 + 0.5) * cell, origin + (z as f32 + 0.5) * cell);
                // Steep anywhere in the cell (corners, edge midpoints and centre) counts.
                let q = cell * 0.5;
                steep[z * n + x] = [(0.0, 0.0), (-q, -q), (q, -q), (-q, q), (q, q), (0.0, -q), (0.0, q), (-q, 0.0), (q, 0.0)]
                    .iter()
                    .any(|(dx, dz)| map.normal_at(c.x + dx, c.y + dz).y < 0.85);
                blocked[z * n + x] = steep[z * n + x] || map.blocked_xz(c, 2.8);
            }
        }
        // Keep off the foot of steep slopes: tanks that clip a cliff toe stall
        // against it. The cells round a steep one are blocked, and the ring
        // round those costs extra for the bots (`avoid`).
        let ring = |src: &[bool]| -> Vec<bool> {
            let mut out = vec![false; n * n];
            for z in 0..n {
                for x in 0..n {
                    if !src[z * n + x] {
                        continue;
                    }
                    for dz in -1i32..=1 {
                        for dx in -1i32..=1 {
                            let (nx, nz) = (x as i32 + dx, z as i32 + dz);
                            if nx >= 0 && nz >= 0 && (nx as usize) < n && (nz as usize) < n {
                                out[nz as usize * n + nx as usize] = true;
                            }
                        }
                    }
                }
            }
            out
        };
        let toe = ring(&steep);
        let near = ring(&toe);
        for i in 0..n * n {
            blocked[i] |= toe[i];
        }
        // Tight lanes (blocked on both sides, like the gaps in a container
        // yard) cost extra too: corners there snag tanks.
        let tight = |x: usize, z: usize| -> bool {
            let b = |dx: i32, dz: i32| {
                let (nx, nz) = (x as i32 + dx, z as i32 + dz);
                nx < 0 || nz < 0 || nx as usize >= n || nz as usize >= n || blocked[nz as usize * n + nx as usize]
            };
            (b(-1, 0) && b(1, 0)) || (b(0, -1) && b(0, 1))
        };
        let avoid: Vec<bool> = (0..n * n).map(|i| !blocked[i] && (near[i] || tight(i % n, i / n))).collect();
        let covers = map
            .obstacles
            .iter()
            .filter(|o| o.cover != Cover::Flat && o.radius > 0.8 && o.radius < 30.0)
            .map(|o| CoverSite { pos: Vec2::new(o.pos.x, o.pos.z), radius: o.radius, tall: o.cover == Cover::Tall })
            .collect();
        NavGrid { cell, n, origin, blocked, avoid, covers }
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

    /// Is the straight line a→b free of blocked cells?
    pub fn line_clear(&self, a: Vec2, b: Vec2) -> bool {
        let d = b - a;
        let steps = (d.length() / (self.cell * 0.4)).ceil().max(1.0) as usize;
        (0..=steps).all(|k| !self.is_blocked(a + d * (k as f32 / steps as f32)))
    }

    /// The bots' straight-line check: also keeps off the foot of steep slopes
    /// (`avoid`), except right at the ends.
    fn line_clear_wide(&self, a: Vec2, b: Vec2) -> bool {
        let d = b - a;
        let steps = (d.length() / (self.cell * 0.4)).ceil().max(1.0) as usize;
        (0..=steps).all(|k| {
            let p = a + d * (k as f32 / steps as f32);
            let (x, z) = self.cell_of(p);
            let i = z * self.n + x;
            !self.blocked[i] && (!self.avoid[i] || k <= 2 || k + 2 >= steps)
        })
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
    /// The Convoy route is built with this (on the server and the client), so
    /// it must stay exactly as it is.
    pub fn path(&self, from: Vec2, to: Vec2) -> Vec<Vec2> {
        let mut out = Vec::new();
        self.search(from, to, &mut out, usize::MAX, false);
        out
    }

    /// The bots' path: like [`NavGrid::path`] into `out` (cleared first), but
    /// cheaper: a goal in straight sight on the grid skips the search, and
    /// the string pulling only looks a bounded way ahead.
    pub fn path_into(&self, from: Vec2, to: Vec2, out: &mut Vec<Vec2>) {
        out.clear();
        if !self.is_blocked(from) && !self.is_blocked(to) && self.line_clear_wide(from, to) {
            out.push(to);
            return;
        }
        self.search(from, to, out, PULL_LOOKAHEAD, true);
    }

    fn search(&self, from: Vec2, to: Vec2, out: &mut Vec<Vec2>, lookahead: usize, cliff_cost: bool) {
        out.clear();
        let n = self.n;
        let s = self.nearest_free(self.cell_of(from));
        let g = self.nearest_free(self.cell_of(to));
        let si = s.1 * n + s.0;
        let gi = g.1 * n + g.0;
        if si == gi {
            out.push(to);
            return;
        }
        SCRATCH.with(|sc| {
            let sc = &mut *sc.borrow_mut();
            if sc.cost.len() != n * n {
                sc.cost = vec![f32::INFINITY; n * n];
                sc.came = vec![u32::MAX; n * n];
                sc.stamp = vec![0; n * n];
                sc.gen = 0;
            }
            sc.gen = sc.gen.wrapping_add(1);
            if sc.gen == 0 {
                sc.stamp.iter_mut().for_each(|x| *x = 0);
                sc.gen = 1;
            }
            let gen = sc.gen;
            let Scratch { cost, came, stamp, open, cells, .. } = sc;
            open.clear();
            let h = |i: usize| -> f32 {
                let dx = (i % n) as f32 - g.0 as f32;
                let dz = (i / n) as f32 - g.1 as f32;
                let (a, b) = (dx.abs(), dz.abs());
                a.max(b) + 0.414 * a.min(b)
            };
            stamp[si] = gen;
            cost[si] = 0.0;
            came[si] = u32::MAX;
            open.push(Node { f: h(si), i: si as u32 });
            let mut found = false;
            let mut expansions = 0;
            while let Some(Node { f, i }) = open.pop() {
                let i = i as usize;
                if i == gi {
                    found = true;
                    break;
                }
                // Stale heap entry (a cheaper route to `i` was found since).
                if f > cost[i] + h(i) + 1e-3 {
                    continue;
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
                    let c = cost[i] + step + if cliff_cost && self.avoid[j] { 3.0 } else { 0.0 };
                    if stamp[j] != gen || c < cost[j] {
                        stamp[j] = gen;
                        cost[j] = c;
                        came[j] = i as u32;
                        open.push(Node { f: c + h(j), i: j as u32 });
                    }
                }
            }
            if !found {
                out.push(to);
                return;
            }
            cells.clear();
            cells.push(gi as u32);
            let mut cur = gi;
            while came[cur] != u32::MAX && cur != si {
                cur = came[cur] as usize;
                cells.push(cur as u32);
            }
            cells.reverse();
            // String pulling, looking a bounded way ahead.
            let pt = |k: usize| self.center(cells[k] as usize % n, cells[k] as usize / n);
            let mut anchor = from;
            let mut k = 0;
            while k < cells.len() {
                let mut far = k;
                let last = k.saturating_add(lookahead).min(cells.len() - 1);
                for m in (k..=last).rev() {
                    let clear = if cliff_cost { self.line_clear_wide(anchor, pt(m)) } else { self.line_clear(anchor, pt(m)) };
                    if clear {
                        far = m;
                        break;
                    }
                }
                out.push(pt(far));
                anchor = pt(far);
                k = far + 1;
            }
        });
        if !self.is_blocked(to) {
            if let Some(last) = out.last_mut() {
                *last = to;
            }
        }
    }
}

// ------------------------------------------------------------------ difficulty

/// The three difficulty tiers. A brain's `difficulty` is a 0..=1 dial; these
/// are the values the menus use and the bands the tiers cover.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Difficulty {
    /// Slow to react, loose aim, drives at you, rarely uses cover or flanks.
    Easy,
    /// Reacts like a casual player, uses cover and flanks some of the time.
    Normal,
    /// Quick and accurate, peeks, flanks, punishes a turned back.
    Hard,
}

impl Difficulty {
    pub const ALL: [Difficulty; 3] = [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard];

    /// The tier a 0..=1 difficulty falls in.
    pub fn of(d: f32) -> Difficulty {
        if d < 0.35 {
            Difficulty::Easy
        } else if d < 0.7 {
            Difficulty::Normal
        } else {
            Difficulty::Hard
        }
    }

    /// The difficulty value for this tier (what `Sim::add_bot` takes).
    pub fn value(self) -> f32 {
        match self {
            Difficulty::Easy => 0.2,
            Difficulty::Normal => 0.5,
            Difficulty::Hard => 0.85,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Easy => "Easy",
            Difficulty::Normal => "Normal",
            Difficulty::Hard => "Hard",
        }
    }
}

/// How a difficulty plays, derived from the 0..=1 dial.
#[derive(Clone, Copy, Debug)]
struct Skill {
    /// Seconds from first seeing a target to the first shot at it.
    reaction: f32,
    /// Seconds between looks round (target picking).
    perceive: f32,
    /// Aim error multiplier on `BotTuning::aim_error_deg`.
    aim: f32,
    /// Seconds for the aim to settle on a new target (first shots are worse).
    settle: f32,
    /// How much of the target's motion the lead accounts for.
    lead: f32,
    /// Chance to fight smart (cover, flanking) rather than drive at the enemy.
    tactics: f32,
    /// Seconds a bot commits to a target before it will switch for a closer one.
    commit: f32,
    /// Chance to notice being shot from the side or behind and react.
    awareness: f32,
}

impl Skill {
    fn of(d: f32) -> Skill {
        let d = d.clamp(0.0, 1.0);
        let lerp = |a: f32, b: f32| a + (b - a) * d;
        Skill {
            reaction: lerp(0.65, 0.18),
            perceive: lerp(0.35, 0.15),
            aim: 1.6 - 1.2 * d,
            settle: lerp(1.4, 0.4),
            lead: lerp(0.85, 1.0),
            tactics: lerp(0.2, 1.0),
            commit: lerp(2.0, 4.5),
            awareness: lerp(0.3, 0.95),
        }
    }
}

// ------------------------------------------------------------------ brain

/// How a bot is fighting its current target.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Tactic {
    /// Circle at the style's preferred range (the default).
    Orbit,
    /// Swing round to the target's side or rear, to a point at `to`.
    Flank { to: Vec2 },
    /// Hold behind cover at `spot`; behind tall cover, step out to `peek` to
    /// fire, and back to reload. `threat` is where the cover hides us from.
    Cover { spot: Vec2, peek: Option<Vec2>, threat: Vec2 },
}

#[derive(Clone, Debug)]
pub struct Brain {
    pub difficulty: f32,
    rng: Rng,
    path: Vec<Vec2>,
    /// The goal `path` leads to.
    path_goal: Option<Vec2>,
    goal: Option<Vec2>,
    goal_age: f32,
    repath: f32,
    think: f32,
    target: Option<u8>,
    last_seen: Option<(Vec3, Vec3, f32)>,
    /// Seconds on the current target; until `react` runs out it holds fire,
    /// and its aim settles over `Skill::settle`.
    target_t: f32,
    react: f32,
    stuck: f32,
    reverse: f32,
    reverse_steer: f32,
    /// Where and how long ago it last got stuck; stuck twice in one place,
    /// it drives off to `detour` for a few seconds before trying again.
    stuck_at: Vec2,
    since_stuck: f32,
    detour: Option<(Vec2, f32)>,
    strafe: f32,
    strafe_t: f32,
    noise: Vec3,
    noise_t: f32,
    chose: bool,
    style: Style,
    /// Wellhead: the derrick this crew is working on, and when to rethink it.
    well: Option<usize>,
    well_t: f32,
    tactic: Tactic,
    tactic_t: f32,
    /// Stepped out from tall cover to fire.
    peeking: bool,
    peek_t: f32,
    /// Ducked behind something close while the gun reloads (in the open fight).
    duck: Option<Vec2>,
    duck_tried: bool,
    /// Shot from an unexpected side: swing the hull round to `evade_to` (a
    /// point) for `evade` seconds.
    evade: f32,
    evade_to: Vec2,
    /// The reaction before the swing starts.
    evade_wait: f32,
    /// On a mode post (escorting, holding a derrick): no swinging off it.
    duty: bool,
    last_health: f32,
    /// The pickup this bot is driving for (other bots leave it alone).
    claim: Option<u16>,
    last_kills: u32,
    /// Horn after a kill: seconds until the next press, presses left.
    horn_t: f32,
    honks: u8,
    stats: BotStats,
}

/// What a bot has been up to (statistics for the harness and tests).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BotStats {
    /// Flanking runs started.
    pub flanks: u32,
    /// Times it took cover, and times it looked for cover and found none.
    pub covers: u32,
    pub cover_misses: u32,
    /// Times it stepped out from tall cover to fire.
    pub peeks: u32,
    /// Times it ducked behind something close to reload.
    pub ducks: u32,
    /// Times it noticed being shot from the side or behind and turned on it.
    pub evades: u32,
    /// Celebrations after a kill.
    pub honks: u32,
    /// Seconds spent alive, and behind cover.
    pub alive_s: f32,
    pub cover_s: f32,
    /// Seconds spent fighting (manoeuvring on a target).
    pub fight_s: f32,
}

impl Brain {
    pub fn new(seed: u64, difficulty: f32) -> Brain {
        let mut rng = Rng::new(seed);
        // Stagger the bots' looks round so they don't all land on one tick.
        let think = rng.range(0.0, 0.3);
        Brain {
            difficulty: difficulty.clamp(0.0, 1.0),
            rng,
            path: Vec::new(),
            path_goal: None,
            goal: None,
            goal_age: 0.0,
            repath: 0.0,
            think,
            target: None,
            last_seen: None,
            target_t: 0.0,
            react: 0.0,
            stuck: 0.0,
            reverse: 0.0,
            reverse_steer: 1.0,
            stuck_at: Vec2::ZERO,
            since_stuck: 100.0,
            detour: None,
            strafe: 1.0,
            strafe_t: 0.0,
            noise: Vec3::ZERO,
            noise_t: 0.0,
            chose: false,
            style: Style::ALL[(seed % 4) as usize],
            well: None,
            well_t: 0.0,
            tactic: Tactic::Orbit,
            tactic_t: 0.0,
            peeking: false,
            peek_t: 0.0,
            duck: None,
            duck_tried: false,
            evade: 0.0,
            evade_to: Vec2::ZERO,
            evade_wait: 0.0,
            duty: false,
            last_health: 0.0,
            claim: None,
            last_kills: 0,
            horn_t: 0.0,
            honks: 0,
            stats: BotStats::default(),
        }
    }

    pub fn style(&self) -> Style {
        self.style
    }

    /// The tier of this brain's difficulty.
    pub fn tier(&self) -> Difficulty {
        Difficulty::of(self.difficulty)
    }

    /// Who this bot is fighting (or hunting from memory), if anyone.
    pub fn target(&self) -> Option<u8> {
        self.target
    }

    /// Flanking runs started this life of the brain (statistics).
    pub fn flank_attempts(&self) -> u32 {
        self.stats.flanks
    }

    /// What this bot has been up to.
    pub fn stats(&self) -> BotStats {
        self.stats
    }

    /// Whether the bot is set up behind cover right now (statistics, debug).
    pub fn in_cover(&self) -> bool {
        (matches!(self.tactic, Tactic::Cover { .. }) && !self.peeking) || self.duck.is_some()
    }

    fn set_target(&mut self, id: Option<u8>, skill: &Skill) {
        if id != self.target {
            self.target = id;
            self.target_t = 0.0;
            self.react = skill.reaction * self.rng.range(0.8, 1.3);
            // Cover stays (it's re-checked against the new threat); a flank
            // on the old target is off.
            if matches!(self.tactic, Tactic::Flank { .. }) {
                self.tactic = Tactic::Orbit;
                self.tactic_t = 0.0;
            }
            self.duck = None;
            self.duck_tried = false;
        }
    }
}

/// A bot crew's personality: how it picks branches, what range it likes,
/// how greedy it is for scrap, how it fights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// Gets close: Twin Barrel / Big Bore, Bulwark, Tesla, Afterburner. Flanks.
    Brawler,
    /// Hangs back: Long Gun, Coilgun, Spaced Armour, Twin Engines. Peeks from cover.
    Sniper,
    /// Scrap first, fights second; picks branches at random. A bit of everything.
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
            Style::Brawler => base * 0.4,
            Style::Sniper => base * 2.2,
            Style::Scavenger => base,
            Style::Speedster => base * 0.6,
        }
    }

    /// Weights of (cover, flank) when a bot that fights smart picks a tactic;
    /// the rest is orbiting.
    fn tactic_weights(self) -> (f32, f32) {
        match self {
            Style::Brawler => (0.25, 0.5),
            Style::Sniper => (0.75, 0.1),
            Style::Scavenger => (0.45, 0.3),
            Style::Speedster => (0.15, 0.65),
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

fn perp(v: Vec2) -> Vec2 {
    Vec2::new(-v.y, v.x)
}

/// Cover from `threat` near `here`: a free spot behind an obstacle that hides
/// the hull from the threat's eye, checked with a sight ray. Returns the spot
/// and, behind tall cover, the edge to peek from.
fn find_cover(sim: &Sim, here: Vec2, threat: Vec3, max_dist: f32, prefer_tall: bool) -> Option<(Vec2, Option<Vec2>)> {
    let nav = &sim.nav;
    let t2 = flat(threat);
    let mut cands: [(f32, Vec2, Option<Vec2>); 4] = [(f32::MAX, Vec2::ZERO, None); 4];
    for c in &nav.covers {
        let dc = c.pos.distance(here);
        if dc > max_dist + c.radius {
            continue;
        }
        let dt = c.pos.distance(t2);
        if dt < c.radius + 12.0 {
            continue;
        }
        let away = (c.pos - t2) / dt;
        let back = if c.tall { c.radius + 4.5 } else { c.radius + 3.2 };
        let spot = c.pos + away * back;
        if nav.is_blocked(spot) {
            continue;
        }
        let mut score = spot.distance(here);
        // Don't cross the enemy's line of fire to get there.
        if (spot - here).dot(t2 - here) > 0.0 && spot.distance(t2) < here.distance(t2) - 5.0 {
            score += 40.0;
        }
        if c.tall != prefer_tall {
            score += 12.0;
        }
        let peek = if c.tall {
            // The edge on our side: step out sideways past the corner.
            let side = perp(away);
            let s = if (here - c.pos).dot(side) >= 0.0 { 1.0 } else { -1.0 };
            let p = c.pos + side * s * (c.radius + 3.5) + away * (c.radius * 0.5 + 2.0);
            if nav.is_blocked(p) {
                let p2 = c.pos - side * s * (c.radius + 3.5) + away * (c.radius * 0.5 + 2.0);
                if nav.is_blocked(p2) {
                    continue;
                }
                Some(p2)
            } else {
                Some(p)
            }
        } else {
            None
        };
        // Keep the best four.
        let worst = (0..4).max_by(|a, b| cands[*a].0.total_cmp(&cands[*b].0)).unwrap();
        if score < cands[worst].0 {
            cands[worst] = (score, spot, peek);
        }
    }
    cands.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (score, spot, peek) in cands.iter().take(3) {
        if *score == f32::MAX {
            break;
        }
        let ground = sim.map.height_at(spot.x, spot.y);
        // Hidden: the hull can't be seen from the threat's eye.
        if !sim.sight_clear(threat, Vec3::new(spot.x, ground + 0.9, spot.y)) {
            return Some((*spot, *peek));
        }
    }
    None
}

/// Where player `me`'s living teammates are, on average.
fn team_centre(sim: &Sim, me: usize) -> Option<Vec2> {
    let p = &sim.players[me];
    p.team?;
    let (mut sum, mut n) = (Vec2::ZERO, 0);
    for m in &sim.players {
        if m.team == p.team && m.id != p.id && m.tank.alive {
            sum += flat(sim.phys.body(m.tank.body).translation());
            n += 1;
        }
    }
    (n > 0).then(|| sum / n as f32)
}

/// Where a team's living crews are, on average.
fn team_centre_of(sim: &Sim, team: Option<u8>) -> Option<Vec2> {
    team?;
    let (mut sum, mut n) = (Vec2::ZERO, 0);
    for m in sim.players.iter().filter(|m| m.team == team && m.tank.alive) {
        sum += flat(sim.phys.body(m.tank.body).translation());
        n += 1;
    }
    (n > 0).then(|| sum / n as f32)
}

/// One tick of a bot's decisions for player index `me`.
pub fn think(sim: &Sim, me: usize, b: &mut Brain, dt: f32) -> Input {
    let t = &sim.tuning;
    let bt = &t.bots;
    let p = &sim.players[me];
    let tank = &p.tank;
    let skill = Skill::of(b.difficulty);
    if !tank.alive {
        b.path.clear();
        b.path_goal = None;
        b.goal = None;
        b.target = None;
        b.last_seen = None;
        b.chose = false;
        b.tactic = Tactic::Orbit;
        b.peeking = false;
        b.duck = None;
        b.evade = 0.0;
        b.claim = None;
        b.last_health = 0.0;
        b.last_kills = p.kills;
        return Input::default();
    }
    let hp = HullPose::of(&sim.phys, tank.body);
    let pos = hp.pos;
    let here = flat(pos);
    let fwd = hp.rot * Vec3::X;
    let vel = sim.phys.body(tank.body).linvel();
    let eye = tank.eye(&hp);
    let mut input = Input::default();
    let outpost = sim.raid.as_ref().map(|r| r.outpost);
    let raider = outpost.is_some() && p.team == Some(crate::raid::TEAM_RAIDERS);
    let crew = tank.vehicle == VehicleKind::Tank && !raider;
    let team_mode = p.team.is_some();
    // Convoy: escort the truck, or hunt it.
    let convoy = sim.convoy_info().filter(|c| c.alive && p.id != c.truck);
    let escort = convoy.is_some_and(|c| p.team == Some(c.escort_team));
    let ring = t.wellhead.radius;
    // Last Crew: the wall and the circle to make for.
    let zone = sim.storm_zone();
    // Last Crew: scavenge first, fight what comes close.
    let looting = zone.is_some() && (sim.match_ticks as f32) < t.last_crew.loot_time * crate::TICK_HZ as f32;
    let health_frac = tank.health / tank.stats.max_health;
    let g = gun_stats(t, &tank.stats);

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

    // --- Being shot. From an unexpected side, a crew that notices swings
    // round to face it (front armour) and turns its attention there.
    if b.last_health > 0.0 && tank.health < b.last_health - 0.5 && crew {
        if let Some(a) = tank.last_attacker.filter(|a| Some(*a) != b.target) {
            if let Some(q) = sim.players.iter().find(|q| q.id == a && q.tank.alive) {
                let ap = flat(sim.phys.body(q.tank.body).translation());
                let to = (ap - here).normalize_or_zero();
                let facing = Vec2::new(fwd.x, fwd.z).normalize_or_zero().dot(to);
                if facing < 0.5 && b.evade <= 0.0 && b.rng.chance(skill.awareness) {
                    b.evade_wait = skill.reaction * b.rng.range(0.8, 1.3);
                    b.evade = b.rng.range(1.0, 1.8) + b.evade_wait;
                    b.evade_to = ap;
                    // Look round soon (after a reaction), to take it as the target.
                    b.think = b.think.min(skill.reaction);
                    b.tactic_t = 0.0;
                    b.stats.evades += 1;
                }
            }
        }
    }
    b.last_health = tank.health;
    b.stats.alive_s += dt;
    if b.in_cover() {
        b.stats.cover_s += dt;
    }
    b.evade = (b.evade - dt).max(0.0);
    b.evade_wait = (b.evade_wait - dt).max(0.0);

    // --- A kill: a honk or two now and then.
    if p.kills > b.last_kills {
        if crew && b.honks == 0 && b.rng.chance(0.55) {
            b.honks = if b.rng.chance(0.35) { 2 } else { 1 };
            b.stats.honks += 1;
            b.horn_t = b.rng.range(0.3, 1.0);
        }
        b.last_kills = p.kills;
    }
    if b.honks > 0 {
        b.horn_t -= dt;
        if b.horn_t <= 0.0 {
            // Press for ~0.3 s; the release then a short gap before the next.
            input.horn = b.horn_t > -0.3;
            if b.horn_t <= -0.3 {
                b.honks -= 1;
                b.horn_t = 0.25;
            }
        }
    }

    // --- Perception (every `Skill::perceive` s).
    b.think -= dt;
    b.target_t += dt;
    b.react = (b.react - dt).max(0.0);
    let rethink = b.think <= 0.0;
    if rethink {
        // A target that's gone (wrecked, left) is forgotten at once.
        if b.target.is_some_and(|id| !sim.players.iter().any(|q| q.id == id && q.tank.alive)) {
            b.last_seen = None;
            b.set_target(None, &skill);
        }
        let step = skill.perceive * b.rng.range(0.85, 1.15);
        b.think = step;
        let storm = 1.0 + (t.storm.sight_mult - 1.0) * sim.storm.intensity;
        // Score everyone first (cheap), then look (a sight ray each) in
        // order of score until someone is in sight.
        let mut cands: Vec<(f32, usize, f32)> = Vec::with_capacity(sim.players.len());
        for (k, q) in sim.players.iter().enumerate() {
            if q.id == p.id || !q.tank.alive || (p.team.is_some() && q.team == p.team) {
                continue;
            }
            let qp = sim.phys.body(q.tank.body).translation();
            let d = qp.distance(pos);
            if d > bt.sight_range * storm {
                continue;
            }
            let mut score = d;
            if tank.last_attacker == Some(q.id) {
                score -= if tank.since_damage < 3.0 { 55.0 } else { 30.0 };
            }
            if b.target == Some(q.id) {
                // Commitment: strong at first, easing off.
                score -= if b.target_t < skill.commit { 60.0 } else { 25.0 };
            }
            if let Some(c) = convoy {
                if q.id == c.truck {
                    // Attackers: the truck is the job.
                    score -= 70.0;
                } else if escort && flat(qp).distance(flat(c.pos)) < 50.0 {
                    // Escorts: whoever is on the truck first.
                    score -= 30.0;
                }
            }
            score -= (q.tank.stats.max_health - q.tank.health) * 0.05;
            // A side or a back turned to us: the weak armour. Sharp crews jump on it.
            let qf = sim.phys.body(q.tank.body).rotation() * Vec3::X;
            let to_me = (pos - qp).normalize_or_zero();
            score -= (1.0 - qf.dot(to_me)) * 10.0 * b.difficulty;
            if team_mode {
                // Focus fire: gang up on whom the team is already shooting.
                let mates = sim
                    .players
                    .iter()
                    .filter(|m| m.team == p.team && m.id != p.id && m.tank.alive)
                    .filter(|m| m.brain.as_ref().is_some_and(|mb| mb.target == Some(q.id)))
                    .count()
                    .min(2);
                score -= mates as f32 * 14.0 * (0.5 + 0.5 * b.difficulty);
            }
            cands.push((score, k, d));
        }
        cands.sort_by(|a, c| a.0.total_cmp(&c.0));
        let mut best: Option<(u8, Vec3, Vec3)> = None;
        for &(_, k, _) in cands.iter().take(4) {
            let q = &sim.players[k];
            let qhp = HullPose::of(&sim.phys, q.tank.body);
            // Sight to the hull or the turret top (the no-peeking rule: walls block, low cover doesn't).
            if sim.sight_clear(eye, qhp.pos + Vec3::Y * 0.3) || sim.sight_clear(eye, qhp.pos + Vec3::Y * 1.5) {
                best = Some((q.id, qhp.pos, sim.phys.body(q.tank.body).linvel()));
                break;
            }
        }
        // Memory is longer behind cover: the enemy is just round the corner.
        let memory = if matches!(b.tactic, Tactic::Cover { .. }) { bt.memory * 2.0 } else { bt.memory };
        match best {
            Some((id, tp, tv)) => {
                // Committed to someone out of sight for now? Stay on them
                // unless this one is much closer or shooting at us.
                let keep_old = b.target.is_some_and(|old| old != id)
                    && b.target_t < skill.commit
                    && b.last_seen.is_some_and(|ls| ls.2 < if b.in_cover() { memory } else { 1.0 } && ls.0.distance(pos) < tp.distance(pos) * 2.0)
                    && tank.last_attacker != Some(id);
                if keep_old {
                    if let Some(ls) = b.last_seen.as_mut() {
                        ls.2 += step;
                    }
                } else {
                    b.set_target(Some(id), &skill);
                    b.last_seen = Some((tp, tv, 0.0));
                }
            }
            None => {
                if let Some(ls) = b.last_seen.as_mut() {
                    ls.2 += step;
                    if ls.2 > memory {
                        b.last_seen = None;
                        b.set_target(None, &skill);
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
        b.strafe_t -= step;
        if b.strafe_t <= 0.0 {
            b.strafe = if b.rng.chance(0.5) { 1.0 } else { -1.0 };
            b.strafe_t = b.rng.range(3.0, 7.0);
        }
        let mut goal: Option<Vec2> = None;
        if raider {
            // Raiders press on the outpost; they turn on defenders that get close.
            let o = flat(outpost.unwrap());
            goal = Some(o);
            if let Some((tp, _, age)) = b.last_seen {
                let d = tp.distance(pos);
                let chase = match tank.vehicle {
                    VehicleKind::Buggy => 30.0,
                    VehicleKind::WarRig => {
                        if tank.stats.gun == GunKind::RigCannons {
                            0.0
                        } else {
                            60.0
                        }
                    }
                    // The Land-Ship's guns do the fighting: it keeps coming.
                    VehicleKind::LandShip => 0.0,
                    _ => 70.0,
                };
                if age < 0.5 && d < chase {
                    goal = Some(flat(tp));
                }
            }
        }
        // Convoy roles. `convoy_post` is where a crew goes when it isn't fighting.
        let mut convoy_post: Option<Vec2> = None;
        if let Some(c) = convoy {
            let tpos = flat(c.pos);
            let side = Vec2::new(-c.dir.y, c.dir.x);
            let threat = b.last_seen.filter(|(tp, _, age)| *age < 0.5 && flat(*tp).distance(tpos) < if escort { 90.0 } else { 140.0 });
            let fighting = threat.is_some();
            convoy_post = Some(if escort {
                if let Some((tp, _, _)) = threat {
                    // Screen the truck: between it and the threat, never far from it.
                    tpos + (flat(tp) - tpos).normalize_or(c.dir) * 13.0 + side * b.strafe * 5.0
                } else if c.health_frac < 0.7 {
                    // Patch it up: hug the truck.
                    tpos + side * b.strafe * 6.0
                } else {
                    // Ride shotgun, a little ahead, on our side of the road.
                    flat(c.ahead) + side * b.strafe * 11.0
                }
            } else {
                tpos
            });
            if !fighting || escort {
                goal = convoy_post;
            }
        }
        // Wellhead: pick a derrick (contested and enemy ones first, ours when
        // they're under attack or thinly held), then stand in its ring.
        let mut well_post: Option<(Vec2, Vec2)> = None;
        if sim.wellhead.is_some() {
            let wells = sim.well_info();
            let mine = p.team.unwrap_or(0).min(1) as usize;
            let theirs = 1 - mine;
            b.well_t -= step;
            let score = |k: usize, w: &crate::wellhead::WellInfo| -> f32 {
                let d = flat(w.pos).distance(here);
                let mut s = d;
                match w.owner {
                    Some(o) if o as usize == mine => {
                        if w.crews[theirs] > 0 {
                            s -= 90.0;
                        } else if w.crews[mine] >= 2 {
                            s += 170.0;
                        } else {
                            s += 80.0;
                        }
                    }
                    Some(_) => s -= 40.0,
                    None => s -= 55.0,
                }
                if w.contested {
                    s -= 70.0;
                }
                // A stable per-crew lean, so the team spreads over the wells.
                let lean = ((p.id as u32).wrapping_mul(2654435761).wrapping_add(k as u32 * 40503) >> 16) as f32 / 65536.0;
                s + lean * 90.0
            };
            let current = b.well.filter(|k| *k < wells.len());
            let held_safe = current.is_some_and(|k| {
                let w = &wells[k];
                w.owner == Some(mine as u8) && w.crews[theirs] == 0 && w.crews[mine] >= 2 && !w.contested
            });
            if current.is_none() || b.well_t <= 0.0 || held_safe {
                let pick = (0..wells.len()).min_by(|a, c| score(*a, &wells[*a]).total_cmp(&score(*c, &wells[*c])));
                if pick != b.well {
                    b.goal_age = 0.0;
                }
                b.well = pick;
                b.well_t = b.rng.range(4.0, 7.0);
            }
            if let Some(k) = b.well.filter(|k| *k < wells.len()) {
                let c = flat(wells[k].pos);
                let a = p.id as f32 * 2.39996 + b.goal_age * 0.05;
                let post = c + Vec2::new(a.cos(), a.sin()) * ring * 0.5;
                well_post = Some((c, post));
                let inside = here.distance(c) < ring * 1.2;
                let threat = b.last_seen.filter(|(tp, _, age)| *age < 0.5 && flat(*tp).distance(c) < 70.0);
                goal = match (inside, threat) {
                    // In the ring with a target: work round it without leaving the ring.
                    (true, Some((tp, _, _))) => {
                        let away = (here - flat(tp)).normalize_or(Vec2::X);
                        let side = Vec2::new(-away.y, away.x) * b.strafe;
                        let want = here + side * 10.0;
                        let off = want - c;
                        Some(if off.length() > ring * 0.75 { c + off.normalize_or_zero() * ring * 0.75 } else { want })
                    }
                    _ => Some(post),
                };
            }
        }
        // Pickups other bots are already driving for (and are closer to).
        let claimed = |id: u16, at: Vec2, mine: f32| -> bool {
            sim.players.iter().any(|q| {
                q.id != p.id
                    && q.tank.alive
                    && q.brain.as_ref().is_some_and(|qb| qb.claim == Some(id))
                    && flat(sim.phys.body(q.tank.body).translation()).distance(at) < mine
            })
        };
        let mut claim: Option<u16> = None;
        // Hurt: a wrench. Badly hurt or crippled, from anywhere; a bit hurt
        // and not fighting, if one is near.
        let engaged = b.last_seen.is_some_and(|ls| ls.2 < 1.0);
        let crippled = tank.tracks.iter().any(|s| s.blown > 0.0);
        let want_repair = (health_frac < bt.repair_threshold || crippled || (health_frac < 0.65 && !engaged)) && tank.repair.is_none();
        if want_repair {
            let reach = if health_frac < bt.repair_threshold || crippled { 260.0 } else { 90.0 };
            let kit = sim
                .pickups
                .iter()
                .filter(|k| k.kind == PickupKind::Repair)
                .map(|k| (k.id, flat(k.pos), flat(k.pos).distance(here)))
                .filter(|k| k.2 < reach && !claimed(k.0, k.1, k.2))
                .min_by(|a, c| a.2.total_cmp(&c.2));
            if let Some((id, at, _)) = kit {
                goal = Some(at);
                claim = Some(id);
            } else if goal.is_none() && sim.mode == crate::raid::ModeKind::TeamBrawl && health_frac < 0.5 {
                // No wrench about: fall back on the team.
                if let Some(c) = team_centre(sim, me).filter(|c| c.distance(here) > 25.0) {
                    goal = Some(c);
                }
            }
        }
        // Scrap: prefer kinds that complete a tier.
        let mut scrap: Option<(f32, Vec2, f32, u16)> = None;
        let scav = b.style == Style::Scavenger;
        for k in &sim.pickups {
            let kp = flat(k.pos);
            let d = kp.distance(here);
            if d > bt.scrap_range + if scav { 60.0 } else { 0.0 } {
                continue;
            }
            if let Some(o) = outpost {
                // Defenders stay near home.
                if kp.distance(flat(o)) > 90.0 {
                    continue;
                }
            }
            if let Some(c) = convoy {
                // Convoy crews don't wander off for scrap.
                if kp.distance(flat(c.pos)) > 50.0 {
                    continue;
                }
            }
            if let Some((c, _)) = well_post {
                // Nor do Wellhead crews: only what's near their derrick.
                if kp.distance(c) > 45.0 {
                    continue;
                }
            }
            if let Some((_, (sc, sr))) = zone {
                // Last Crew: nothing outside the circle is worth the storm.
                if kp.distance(sc) > sr * 0.85 {
                    continue;
                }
            }
            let bonus = if scav { 30.0 } else { 0.0 }
                + match k.kind {
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
            // Someone nearer is already on it: look elsewhere.
            let taken = if claimed(k.id, kp, d) { 60.0 } else { 0.0 };
            let score = d - bonus + taken;
            if scrap.is_none_or(|x| score < x.0) {
                scrap = Some((score, kp, d, k.id));
            }
        }
        // Hiding is for crew-on-crew fights: the truck and the bosses are
        // the job (go at them), and Raider Waves defenders hold the line
        // unless they're hurt.
        let objective = b.target.and_then(|id| sim.players.iter().find(|q| q.id == id)).is_some_and(|q| {
            matches!(q.tank.vehicle, VehicleKind::WarRig | VehicleKind::LandShip | VehicleKind::FuelTanker)
        });
        let hide_ok = !objective && (outpost.is_none() || health_frac < 0.5);
        if !hide_ok && matches!(b.tactic, Tactic::Cover { .. }) {
            b.tactic = Tactic::Orbit;
            b.tactic_t = 0.0;
            b.peeking = false;
        }
        // --- Fighting: a target in sight (or just ducked behind cover) and
        // within reach of where this crew should be.
        b.tactic_t -= step;
        let mut engage = None;
        let mut hunt = None;
        if let Some((tp, _, age)) = b.last_seen {
            let d = tp.distance(pos);
            let leash = outpost.is_none_or(|o| flat(tp).distance(flat(o)) < 110.0)
                && convoy.is_none_or(|c| flat(tp).distance(flat(c.pos)) < 140.0)
                && (!looting || d < 55.0);
            let reach = if b.style == Style::Sniper { bt.engage_range + 40.0 } else { bt.engage_range };
            let in_cover = matches!(b.tactic, Tactic::Cover { .. });
            if (age < 0.5 || (in_cover && age < bt.memory * 2.0)) && d < reach && leash {
                engage = Some((tp, d));
            } else if age >= 0.5 && leash {
                hunt = Some(flat(tp));
            }
        }
        // A mode post or a wrench: duty first, no ducking off it.
        let duty = goal.is_some();
        b.duty = duty && claim.is_none();
        if goal.is_none() {
            // Grab scrap close by, even mid-fight (closer then).
            let close_fight = engage.is_some_and(|(_, d)| d < 80.0);
            let grab = match (scav || looting, close_fight) {
                (true, false) => 80.0,
                (false, false) => 50.0,
                (true, true) => 55.0,
                (false, true) => 35.0,
            };
            if let Some(s) = scrap.filter(|s| s.2 < grab) {
                goal = Some(s.1);
                claim = Some(s.3);
            }
        }
        let mut fight_goal = None;
        if goal.is_none() {
            if let Some((tp, d)) = engage {
                fight_goal = Some(fight(sim, b, &skill, here, tp, d, health_frac, crew, hide_ok, tank.reload, g.reload, step));
                b.stats.fight_s += step;
            } else if let Some(h) = hunt {
                goal = Some(h);
                b.tactic = Tactic::Orbit;
                b.peeking = false;
            }
        }
        if fight_goal.is_some() {
            goal = fight_goal;
        }
        // Shoot and scoot: in a fight in the open, a hurt crew ducks behind
        // something close while a slow gun reloads; back out when it's loaded.
        if tank.reload < 0.4 {
            b.duck = None;
            b.duck_tried = false;
        }
        let open_fight = !matches!(b.tactic, Tactic::Cover { .. });
        match engage {
            Some((tp, _)) if !duty && crew && hide_ok && open_fight && g.reload >= 2.5 && health_frac < 0.7 => {
                if tank.reload > 1.0 && !b.duck_tried {
                    b.duck_tried = true;
                    if b.rng.chance(skill.tactics * 0.7) {
                        // Only something right by us: it has to be reached before the gun's loaded.
                        b.duck = find_cover(sim, here, tp + Vec3::Y * 2.0, 14.0, true).map(|c| c.0);
                        if b.duck.is_some() {
                            b.stats.ducks += 1;
                        }
                    }
                }
                if let Some(dk) = b.duck {
                    goal = Some(dk);
                }
            }
            _ => b.duck = None,
        }
        if fight_goal.is_none() && b.duck.is_none() && goal.is_some() {
            b.peeking = false;
        }
        if goal.is_none() {
            goal = convoy_post;
        }
        let pushing = sim.mode == crate::raid::ModeKind::TeamBrawl && !scav && sim.match_ticks > 40 * crate::TICK_HZ;
        if goal.is_none() && pushing {
            // Scrap on the way still gets picked up.
            if let Some(s) = scrap.filter(|s| s.2 < 70.0) {
                goal = Some(s.1);
                claim = Some(s.3);
            }
        }
        if goal.is_none() && pushing {
            // Team Brawl: move up with the team towards the other one (the
            // radio has a rough idea where they are), a spread-out line.
            if let (Some(mine), Some(theirs)) = (team_centre(sim, me), team_centre_of(sim, p.team.map(|t| 1 - t.min(1)))) {
                let front = mine.lerp(theirs, 0.5);
                let across = perp((theirs - mine).normalize_or_zero());
                let slot = (p.id % 4) as f32 - 1.5;
                if front.distance(here) > 20.0 {
                    goal = Some(front + across * slot * 14.0);
                }
            }
        }
        if goal.is_none() {
            if let Some(s) = scrap {
                goal = Some(s.1);
                claim = Some(s.3);
            }
        }
        if goal.is_none() && sim.mode == crate::raid::ModeKind::TeamBrawl {
            // Team Brawl, nothing to do: regroup with the team.
            if let Some(c) = team_centre(sim, me).filter(|c| c.distance(here) > 45.0) {
                goal = Some(c + perp((c - here).normalize_or_zero()) * b.strafe * 8.0);
            }
        }
        if goal.is_none() {
            if let Some(o) = outpost {
                // Defenders with nothing to do hold near the outpost.
                let o = flat(o);
                if here.distance(o) > 40.0 {
                    goal = Some(o);
                } else if b.goal.is_none_or(|g| g.distance(here) < 8.0 || b.goal_age > 12.0) {
                    let a = b.rng.range(0.0, std::f32::consts::TAU);
                    goal = Some(o + Vec2::new(a.cos(), a.sin()) * b.rng.range(18.0, 34.0));
                } else {
                    goal = b.goal;
                }
            }
        }
        if goal.is_none() {
            let keep = b.goal.filter(|g| g.distance(here) > 12.0 && b.goal_age < 30.0);
            goal = keep.or_else(|| {
                let sites = &sim.map.scrap_sites;
                if sites.is_empty() {
                    return Some(Vec2::ZERO);
                }
                Some(flat(sites[b.rng.below(sites.len())].pos))
            });
        }
        // Last Crew: stay inside the circle (the one it's closing to, once
        // that's called); outside it, run for the middle.
        if let Some((_, (sc, sr))) = zone {
            let keep = (sr * 0.7).max(3.0);
            if here.distance(sc) > sr * 0.85 {
                goal = Some(sc + (here - sc).normalize_or_zero() * sr * 0.35);
            } else if let Some(g) = goal {
                if g.distance(sc) > keep {
                    goal = Some(sc + (g - sc).normalize_or_zero() * keep);
                }
            } else {
                goal = Some(sc);
            }
        }
        b.claim = claim;
        let changed = match (b.goal, goal) {
            (Some(a), Some(c)) => a.distance(c) > 10.0,
            (None, Some(_)) => true,
            _ => false,
        };
        if changed {
            b.goal_age = 0.0;
        }
        b.goal = goal;
        b.repath -= step;
        // A new path when the goal has moved off the old one's end, now and
        // then anyway (the world changes), or when the old one ran out.
        let drifted = match (b.path_goal, b.goal) {
            (Some(a), Some(c)) => a.distance(c) > 6.0,
            (None, Some(_)) => true,
            _ => false,
        };
        if drifted || b.repath <= 0.0 || b.path.is_empty() {
            if let Some(gp) = b.goal {
                sim.nav.path_into(here, gp, &mut b.path);
                b.path_goal = Some(gp);
            }
            b.repath = bt.repath_interval * 2.0;
        }
    }
    b.goal_age += dt;

    // --- Steering.
    while let Some(w) = b.path.first() {
        if w.distance(here) < 6.0 && b.path.len() > 1 {
            b.path.remove(0);
        } else {
            break;
        }
    }
    let speed = vel.length();
    let hull_yaw = (-fwd.z).atan2(fwd.x);
    let steer_to = |to: Vec2| -> f32 {
        let want = (-to.y).atan2(to.x);
        wrap(want - hull_yaw)
    };
    if let Some(&w) = b.path.first() {
        let to = w - here;
        let dist = to.length();
        let err = steer_to(to);
        input.steer = (-err * 2.2).clamp(-1.0, 1.0);
        input.throttle = if err.abs() > 1.1 { 0.0 } else { err.cos().max(0.25) };
        if b.path.len() == 1 && dist < if matches!(b.tactic, Tactic::Cover { .. }) { 3.5 } else { 8.0 } {
            input.throttle = 0.0;
            input.steer = 0.0;
            // Parked behind cover (or on a post): turn the front armour to the threat.
            if let Tactic::Cover { threat, .. } = b.tactic {
                let e = steer_to(threat - here);
                if e.abs() > 0.3 {
                    input.steer = (-e * 1.5).clamp(-1.0, 1.0);
                }
            }
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
        // Boost on long straight runs, and on a flanking run.
        let thirst = if b.style == Style::Speedster { 0.3 } else { 0.6 };
        let flanking = matches!(b.tactic, Tactic::Flank { .. });
        input.boost = (b.last_seen.is_none() || b.style == Style::Speedster || flanking)
            && err.abs() < 0.2
            && dist > if flanking { 25.0 } else { 50.0 }
            && tank.fuel > tank.stats.fuel_max * thirst;
        // Caught outside the wall: burn everything to get back in.
        if let Some(((wc, wr), _)) = zone {
            if here.distance(wc) > wr - 5.0 && err.abs() < 0.4 {
                input.boost = tank.fuel > 0.0;
            }
        }
        if raider && tank.vehicle == VehicleKind::Buggy {
            input.throttle = input.throttle.max(0.6);
        }
    }
    // Shot from the side or behind: swing round to face it, still rolling.
    if b.evade > 0.0 && b.evade_wait <= 0.0 && !b.duty {
        let e = steer_to(b.evade_to - here);
        input.steer = (-e * 2.5).clamp(-1.0, 1.0);
        input.throttle = if e.abs() > 1.2 { 0.35 * b.strafe } else { input.throttle.max(0.3) };
    }

    // Stuck: pushing but not moving -> back off and swing.
    // A detour after getting stuck twice in one place: drive to it first.
    b.since_stuck += dt;
    if let Some((d, t)) = b.detour {
        let left = t - dt;
        if left <= 0.0 || d.distance(here) < 5.0 {
            b.detour = None;
            b.repath = 0.0;
        } else {
            b.detour = Some((d, left));
            let e = steer_to(d - here);
            input.steer = (-e * 2.2).clamp(-1.0, 1.0);
            input.throttle = if e.abs() > 1.1 { 0.3 } else { 1.0 };
            input.boost = false;
        }
    }
    if b.reverse > 0.0 {
        b.reverse -= dt;
        input.throttle = -1.0;
        input.steer = b.reverse_steer;
        input.boost = false;
        // Backed into something too: give up reversing.
        if speed < 0.5 && b.reverse < 0.6 {
            b.reverse = 0.0;
        }
    } else if input.throttle.abs() > 0.4 && speed < 0.8 {
        b.stuck += dt;
        if b.stuck > 1.5 {
            b.stuck = 0.0;
            let again = b.since_stuck < 10.0 && b.stuck_at.distance(here) < 12.0;
            b.stuck_at = here;
            b.since_stuck = 0.0;
            b.reverse = b.rng.range(1.0, 1.8);
            b.reverse_steer = if b.rng.chance(0.5) { 1.0 } else { -1.0 };
            b.repath = 0.0;
            b.evade = 0.0;
            if again {
                // Stuck here before: back off further, then go round by a
                // free point off to one side before trying the goal again.
                b.reverse = b.rng.range(1.8, 2.6);
                let back = -Vec2::new(fwd.x, fwd.z).normalize_or(Vec2::X);
                let side = perp(back) * if b.rng.chance(0.5) { 1.0 } else { -1.0 };
                for (k, dir) in [back + side, back - side, side, -side, back].into_iter().enumerate() {
                    let pt = here + dir.normalize_or(back) * (18.0 + k as f32 * 2.0);
                    let free = |p: Vec2| {
                        let (x, z) = sim.nav.cell_of(p);
                        let i = z * sim.nav.n + x;
                        !sim.nav.blocked[i] && !sim.nav.avoid[i]
                    };
                    if free(pt) && free((pt + here) * 0.5) {
                        b.detour = Some((pt, 5.0));
                        break;
                    }
                }
                b.claim = None;
            }
            // Cover we can't reach isn't cover.
            if b.tactic != Tactic::Orbit {
                b.tactic = Tactic::Orbit;
                b.tactic_t = 1.5;
                b.peeking = false;
            }
        }
    } else {
        b.stuck = (b.stuck - dt).max(0.0);
    }

    // --- Aim and fire.
    b.noise_t -= dt;
    if b.noise_t <= 0.0 {
        b.noise_t = b.rng.range(0.4, 1.0);
        let mut err = bt.aim_error_deg.to_radians() * skill.aim;
        // A fresh target: the first shots are the loosest.
        err *= 1.0 + 0.4 * (-b.target_t / skill.settle).exp();
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
    let can_gun = tank.vehicle != VehicleKind::Buggy
        && !(tank.vehicle == VehicleKind::WarRig && tank.stats.gun != GunKind::RigCannons)
        && !matches!(tank.vehicle, VehicleKind::LandShip | VehicleKind::FuelTanker);

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
        // Against the Land-Ship, crews pick at the module nearest them (guns,
        // nests, tracks, plates) rather than the middle of the hull.
        let mut offset = Vec3::Y * 0.4;
        let tq = b.target.and_then(|id| sim.players.iter().find(|q| q.id == id));
        if let Some(q) = tq {
            if q.tank.vehicle == VehicleKind::LandShip && q.tank.alive {
                let qhp = HullPose::of(&sim.phys, q.tank.body);
                let near = q
                    .tank
                    .location_centers(&qhp, t)
                    .into_iter()
                    .filter(|(loc, _)| matches!(loc, crate::events::HitLocation::Part(_)))
                    .min_by(|a, c| a.1.distance(pos).total_cmp(&c.1.distance(pos)));
                if let Some((_, c)) = near {
                    offset = c - qhp.pos;
                }
            }
        }
        // Track a visible target live (perception only refreshes it now and then).
        let (tp, tv) = match tq {
            Some(q) if age < 0.3 && q.tank.alive => (sim.phys.body(q.tank.body).translation(), sim.phys.body(q.tank.body).linvel()),
            _ => (tp, tv),
        };
        let mut aim = tp + offset;
        if age < 0.3 {
            // Lead: time of flight from distance, iterated; better crews lead fully.
            let mut tof = aim.distance(pos) / g.shell_speed;
            for _ in 0..2 {
                aim = tp + offset + tv * tof * skill.lead;
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
        // Last Crew, early: hold fire on far crews that leave you alone (scavenging).
        let truce = looting && d > 30.0 && tank.last_attacker != b.target;
        let ready = b.react <= 0.0;
        if can_gun && ready && tank.reload <= 0.0 && aligned && in_range && visible && !truce {
            let muzzle = tank.muzzle(&hp, t, 0.0);
            if sim.sight_clear(muzzle, tp + Vec3::Y * 0.5) {
                input.fire = true;
            }
        }
        if visible && !truce && ready {
            match tank.stats.alt {
                AltKind::Mg => input.alt_fire = d < 110.0 && wrap(wy - tank.turret_yaw).abs() < 0.15,
                AltKind::Coilgun => input.alt_fire = !(tank.charge >= 1.0 && aligned),
                AltKind::Tesla => input.alt_fire = d < t.alt.tesla_range,
                AltKind::Flamethrower => input.alt_fire = d < t.special.flame_range + 2.0,
                AltKind::Rockets => input.alt_fire = tank.lock_progress >= 1.0 && !tank.prev_alt,
            }
        }
        // The special button: the air strike, then the rare part.
        if !truce && ready && tank.vehicle == VehicleKind::Tank {
            let s = &t.special;
            let tpos = flat(tp);
            let target_speed = tv.length();
            let near_enemies = sim
                .players
                .iter()
                .filter(|q| q.tank.alive && q.id != p.id && (p.team.is_none() || q.team != p.team))
                .filter(|q| flat(sim.phys.body(q.tank.body).translation()).distance(tpos) < 18.0)
                .count();
            let friends_near = sim
                .players
                .iter()
                .filter(|q| q.tank.alive && (q.id == p.id || (p.team.is_some() && q.team == p.team)))
                .any(|q| flat(sim.phys.body(q.tank.body).translation()).distance(tpos) < 22.0);
            let want = if tank.air_strike {
                // A worthwhile drop: a sitting duck, a crowd, or a camper we
                // can't see (last seen within a few seconds), never on friends.
                d > 40.0 && d < s.strike_max_range && !friends_near && age < 3.0 && (target_speed < 4.0 || near_enemies >= 2 || !visible)
            } else {
                match tank.stats.rare {
                    // Mortars lob over cover: fine on someone we just lost.
                    Some(PartKind::Mortar) => d > 45.0 && d < 260.0 && age < 2.5 && !friends_near,
                    Some(PartKind::Harpoon) => visible && d > 15.0 && d < s.harpoon_range * 0.9 && aligned,
                    Some(PartKind::SmokeLauncher) => (health_frac < 0.5 && tank.since_damage < 1.0) || (b.evade > 0.0 && health_frac < 0.7),
                    // Mines behind us when someone's on our tail.
                    Some(PartKind::MineLayer) => {
                        let behind = (flat(tp) - here).normalize_or_zero().dot(Vec2::new(fwd.x, fwd.z).normalize_or_zero()) < -0.3;
                        visible && d < 45.0 && (behind || d < 20.0)
                    }
                    _ => false,
                }
            };
            if want && !visible {
                // Strike or mortar on where they were.
                input.aim_point = Vec3::new(tp.x, sim.map.height_at(tp.x, tp.z), tp.z);
            }
            input.special = want && !tank.prev_special;
        }
    }
    input
}

/// The goal while fighting a target at `tp` (velocity `tv`, `d` m away):
/// orbit it, swing round its flank, or hold cover and peek. Picks a new
/// tactic when the current one is done or stale.
#[allow(clippy::too_many_arguments)]
fn fight(sim: &Sim, b: &mut Brain, skill: &Skill, here: Vec2, tp: Vec3, d: f32, health_frac: f32, crew: bool, hide_ok: bool, reload: f32, reload_time: f32, step: f32) -> Vec2 {
    let bt = &sim.tuning.bots;
    let t2 = flat(tp);
    let range = b.style.range(bt.preferred_range);
    let threat_eye = tp + Vec3::Y * 2.0;
    if crew && b.tactic_t <= 0.0 {
        // Time to (re)think how to fight.
        b.tactic_t = b.rng.range(3.0, 5.5);
        let (w_cover, w_flank) = b.style.tactic_weights();
        // Sharp crews press (flank) more and sit back less, unless hurt;
        // hurt or slow-firing crews are more inclined to cover.
        let dif = b.difficulty;
        let hurt = (1.0 - health_frac).clamp(0.0, 1.0);
        let w_flank = w_flank * (0.6 + 0.8 * dif);
        let w_cover = (w_cover * 0.6 * (1.2 - 0.5 * dif) + hurt * 0.5 + if reload_time > 3.0 { 0.15 } else { 0.0 }).min(0.95);
        let w_flank = w_flank.min(1.0 - w_cover);
        let roll = b.rng.range(0.0, 1.0);
        let smart = b.rng.chance(skill.tactics);
        let mut next = Tactic::Orbit;
        if smart && roll < w_cover && hide_ok {
            let prefer_tall = b.style == Style::Sniper || health_frac < 0.5 || b.rng.chance(0.5);
            if let Some((spot, peek)) = find_cover(sim, here, threat_eye, 45.0, prefer_tall) {
                next = Tactic::Cover { spot, peek, threat: t2 };
                b.stats.covers += 1;
                // Hold it a good while: shoot, duck, reload, repeat.
                b.tactic_t = b.rng.range(10.0, 18.0);
            } else {
                b.stats.cover_misses += 1;
            }
        } else if smart && roll < w_cover + w_flank && d > 25.0 {
            // Come in from the side away from its gun (or its hull's facing).
            let to_me = (here - t2).normalize_or(Vec2::X);
            let side = perp(to_me);
            let s = if b.rng.chance(0.5) { 1.0 } else { -1.0 };
            let r = range.clamp(25.0, 70.0);
            for s in [s, -s] {
                let dir = (side * s * 0.9 - to_me * 0.45).normalize_or(side);
                let pt = t2 + dir * r;
                if !sim.nav.is_blocked(pt) {
                    next = Tactic::Flank { to: pt };
                    b.stats.flanks += 1;
                    b.tactic_t = 12.0;
                    break;
                }
            }
        }
        b.tactic = next;
        b.peeking = false;
    }
    match b.tactic {
        Tactic::Flank { to } => {
            // The target moves: keep the flank point on its side.
            if here.distance(to) < 10.0 || to.distance(t2) > range.clamp(25.0, 70.0) * 1.8 {
                b.tactic = Tactic::Orbit;
                b.tactic_t = b.rng.range(2.0, 4.0);
            } else {
                return to;
            }
        }
        Tactic::Cover { spot, peek, threat } => {
            // The threat has moved round our cover: it doesn't cover anymore.
            // Look for cover from where it is now, else fight in the open.
            let swing = (threat - spot).normalize_or_zero().dot((t2 - spot).normalize_or_zero());
            let moved = swing < 0.87 || t2.distance(spot) < 15.0;
            if moved {
                b.peeking = false;
                match find_cover(sim, here, threat_eye, 35.0, peek.is_some()) {
                    Some((spot, peek)) => {
                        b.tactic = Tactic::Cover { spot, peek, threat: t2 };
                        b.stats.covers += 1;
                        return spot;
                    }
                    None => {
                        b.tactic = Tactic::Orbit;
                        b.tactic_t = b.rng.range(2.0, 4.0);
                    }
                }
            } else if let Some(pk) = peek {
                // Tall cover: out when loaded, back in to reload.
                b.peek_t += step;
                if b.peeking {
                    if (reload > 0.5 && b.peek_t > 0.5) || b.peek_t > 4.5 {
                        b.peeking = false;
                        b.peek_t = 0.0;
                    }
                } else if reload < 0.35 && here.distance(spot) < 6.0 && b.peek_t > 0.4 {
                    b.peeking = true;
                    b.peek_t = 0.0;
                    b.stats.peeks += 1;
                }
                return if b.peeking { pk } else { spot };
            } else {
                // Low cover: sit hull-down and trade.
                return spot;
            }
        }
        Tactic::Orbit => {}
    }
    // Orbit at the preferred range, swapping sides now and then.
    let away = (here - t2).normalize_or(Vec2::X);
    let ang = 0.7 * b.strafe;
    let rot = Vec2::new(ang.cos(), ang.sin());
    let dir = Vec2::new(away.x * rot.x - away.y * rot.y, away.x * rot.y + away.y * rot.x);
    t2 + dir * range
}
