//! The arena: a heightfield floor plus static obstacles, spawns and pickup sites.
//!
//! Coordinates: **Y is up**, metres, the map centred on the origin, spanning
//! `-size/2 ..= size/2` in X and Z. Everything here is a pure function of the
//! seed, so the client rebuilds the same map with `Map::the_sink(seed)` and
//! never needs it over the wire.
//!
//! Heightfield layout (same as the rapier collider): `heights[iz * resolution + ix]`
//! is the height at `x = -size/2 + ix * spacing`, `z = -size/2 + iz * spacing`.
//! Each cell is split into two triangles along the diagonal from
//! `(ix, iz+1)` to `(ix+1, iz)`; [`Map::height_at`] interpolates exactly the
//! way the collider does, so a mesh built with that split matches physics.
//!
//! The level rule (DESIGN.md "Reading movement"): every obstacle is either
//! [`Cover::Low`] (its top is under 1.2 m above the ground, a tank behind it
//! stays visible), [`Cover::Tall`] (at least 2.5 m: hides a whole tank, turret
//! included; single 2.6 m containers count, the rest is well over 4 m) or
//! [`Cover::Flat`] (the highway deck, drivable, not cover).

use glam::{Quat, Vec2, Vec3};
use serde::{Deserialize, Serialize};

use crate::rng::Rng;

/// The four scrap kinds, which are also the four upgrade tracks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum ScrapKind {
    /// Grey plates: armour and chassis.
    Steel = 0,
    /// Red shell crates: guns.
    Powder = 1,
    /// Blue coils: sensors and energy weapons.
    Volts = 2,
    /// Yellow jerrycans: engine and boost.
    Fuel = 3,
}

impl ScrapKind {
    pub const ALL: [ScrapKind; 4] = [ScrapKind::Steel, ScrapKind::Powder, ScrapKind::Volts, ScrapKind::Fuel];
    pub fn index(self) -> usize {
        self as usize
    }
    pub fn from_index(i: usize) -> ScrapKind {
        Self::ALL[i % 4]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObstacleKind {
    /// A beached, rusted ship hull with a bridge tower. Tall.
    ShipHull,
    /// A cluster of sandstone blocks. Tall.
    RockFormation,
    /// Shipping containers (2.6 m each) in rows, one to three high. Tall.
    ContainerStack,
    /// A flat-topped sandstone mesa, wide and high. Tall.
    Mesa,
    /// A rusted grain/fuel silo (draw as a cylinder of radius `boxes[0].half.x`). Tall.
    Silo,
    /// A row of concrete road barriers. Low.
    Barrier,
    /// A line of sandbags. Low.
    Sandbags,
    /// One straight piece of the old highway: a flat asphalt deck flush with
    /// the ground (drivable, not cover). Consecutive segments form the strip.
    Highway,
    /// A big rusted pipe section lying on its side. Low.
    Pipe,
    /// A flattened car wreck. Low.
    CarWreck,
    /// A broken chunk of the old pier deck. Low.
    PierRuin,
    /// A pallet of wooden crates. Low, destructible.
    Crates,
    /// A horizontal fuel tank on saddles (draw as a cylinder along local X of radius
    /// `boxes[0].half.y`). Tall; explodes when shot enough.
    FuelTank,
    /// A dockside crane: tower + jib (`boxes[0]`, `boxes[1]`). A wrecking ball
    /// hangs from the jib tip on a rope; see `Map::cranes`. Tall.
    Crane,
    /// A hydraulic launch plate set into the ground (flat, drivable). Throws a
    /// tank that drives over it fast; recharges.
    LaunchPlate,
    /// A concrete wall segment, 3 m tall, 0.7 m thick (along local X). Tall.
    Wall,
    /// A quonset tent: draw a half-cylinder over `boxes[0]` (long along local X). Tall.
    Tent,
    /// A wooden watchtower: legs + cabin, drawn over `boxes[0]` (8 m). Tall.
    Tower,
    /// A gas station ruin: `boxes[0]` the shop, `boxes[1]` the canopy roof
    /// (overhead, 4.9-5.5 m), `boxes[2..]` the canopy pillars. Tall.
    GasStation,
    /// A Czech hedgehog (welded steel X), about 1 m. Low.
    Hedgehog,
    /// A wooden power pole with a crossarm (`boxes[1]`, overhead). Tall (thin).
    PowerPole,
    // ---- The Refinery -------------------------------------------------------
    /// A big vertical storage tank (draw as a cylinder of radius `boxes[0].half.x`).
    /// Tall; explodes, and sets off its neighbours (chain reactions).
    StorageTank,
    /// A distillation column (draw as a cylinder of radius `boxes[0].half.x`). Tall.
    Column,
    /// A pipe rack: `boxes[0]` the overhead pipe bundle (from 5.8 m up: tanks
    /// drive under it), `boxes[1..]` its legs. Tall (overhead).
    PipeRack,
    /// An oil pumpjack: `boxes[0]` the gearbox and frame; the walking beam
    /// above it bobs (drawn from time). Tall.
    Pumpjack,
    /// A drilling derrick: `boxes[0..4]` the four legs (drive between them),
    /// `boxes[4]` the upper tower (overhead). Tall.
    Derrick,
    /// A rail tank car (long along local X). Tall; explodes like a fuel tank.
    TankCar,
    /// One straight piece of railway: ties and rails flush with the ground
    /// (drivable, not cover).
    Rails,
    /// The flare stack: `boxes[0]` the plinth, `boxes[1]` the chimney (a
    /// burning flare on top: the landmark). Tall.
    FlareStack,
    /// The blast-proof control house of the refinery. Tall.
    ControlHouse,
    // ---- Landmarks (appended) ------------------------------------------------
    /// A water tower: `boxes[0..4]` the four legs (drive between them),
    /// `boxes[4]` the tank (overhead, from 8 m up). Tall.
    WaterTower,
    /// A guyed radio mast: `boxes[0]` the thin lattice mast (about 24 m). Tall (thin).
    RadioMast,
    /// A wind pump: `boxes[0]` the lattice tower (about 11 m) with the fan
    /// wheel on top. Tall (thin).
    Windmill,
    // ---- The Rust Belt (appended) ---------------------------------------------
    /// A wall of crushed car cubes: one box per column of bales (1.35 m a
    /// bale, two to four high), side by side along local X. Tall.
    CarCube,
    /// The car crusher: `boxes[0]` the press bed, `boxes[1]` the raised press
    /// head on its frame. Tall.
    Crusher,
    /// An electromagnet crane: `boxes[0]` the tower, `boxes[1]` the jib
    /// (overhead). The magnet and the car it holds hang off the jib tip
    /// (visual only). Tall.
    MagnetCrane,
    /// A rusted school bus (long along local X); a second box is a bus
    /// stacked on top. Tall.
    Bus,
    /// A tin shack of corrugated sheets. Tall.
    Shack,
    /// A market stall: a low counter (the awning over it is visual only). Low.
    Stall,
    /// A corrugated tin fence run, about 2.9 m (along local X). Tall.
    Fence,
    /// The broken concrete bridge over the dry river: `boxes[0]` the deck
    /// (overhead: tanks drive on it and under it), `boxes[1]` the pier in the
    /// river bed. The obstacle stands on the river bed. Tall.
    Bridge,
    /// The tower of stacked cars, the skyline landmark: `boxes[0]` the lower
    /// stack, `boxes[1]` the upper. Tall.
    CarTower,
}

impl ObstacleKind {
    /// Fuel hazards: explode when shot enough.
    pub fn is_fuel(self) -> bool {
        matches!(self, ObstacleKind::Silo | ObstacleKind::FuelTank | ObstacleKind::StorageTank | ObstacleKind::TankCar)
    }

    /// Colliders that are upright cylinders (box half x = radius, half y = half height).
    pub fn is_cylinder(self) -> bool {
        matches!(self, ObstacleKind::Silo | ObstacleKind::StorageTank | ObstacleKind::Column)
    }
}

/// Which map to build. The seed only jitters details; the id picks the place.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MapId {
    /// The dry lake bed (the first map).
    #[default]
    Sink,
    /// The sun-baked oil refinery on a salt flat.
    Refinery,
    /// A dead scrapyard city in a dry canyon.
    RustBelt,
}

impl MapId {
    pub const ALL: [MapId; 3] = [MapId::Sink, MapId::Refinery, MapId::RustBelt];

    /// The display name.
    pub fn name(self) -> &'static str {
        match self {
            MapId::Sink => "The Sink",
            MapId::Refinery => "The Refinery",
            MapId::RustBelt => "The Rust Belt",
        }
    }

    /// A short key for URLs and menus (`sink`, `refinery`).
    pub fn key(self) -> &'static str {
        match self {
            MapId::Sink => "sink",
            MapId::Refinery => "refinery",
            MapId::RustBelt => "rustbelt",
        }
    }

    /// Parses a key (`sink`, `the_sink`, `refinery`, `the_refinery`).
    pub fn from_key(s: &str) -> Option<MapId> {
        let s = s.trim().to_ascii_lowercase();
        match s.trim_start_matches("the_").trim_start_matches("the ") {
            "sink" => Some(MapId::Sink),
            "refinery" => Some(MapId::Refinery),
            "rustbelt" | "rust_belt" | "rust belt" | "rust-belt" => Some(MapId::RustBelt),
            _ => None,
        }
    }

    /// The next map in the rotation (quick-match rooms alternate).
    pub fn next(self) -> MapId {
        MapId::ALL[(self as usize + 1) % MapId::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cover {
    /// Top under 1.2 m above the ground: you can't hide behind it.
    Low,
    /// 2.5 m or more: hides a tank, blocks sight.
    Tall,
    /// Flush with the ground (the highway): not cover, drivable.
    Flat,
}

/// One box of an obstacle, in the obstacle's local frame.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObstacleBox {
    pub center: Vec3,
    pub half: Vec3,
    pub rot: Quat,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Obstacle {
    pub kind: ObstacleKind,
    pub cover: Cover,
    /// Base position (on the ground) and orientation of the obstacle frame.
    pub pos: Vec3,
    pub rot: Quat,
    /// Colliders, in the obstacle frame. The client draws its model over these.
    pub boxes: Vec<ObstacleBox>,
    /// Footprint radius used for placement and bot navigation.
    pub radius: f32,
}

impl Obstacle {
    /// World-space (center, rotation, half extents) of each box.
    pub fn world_boxes(&self) -> impl Iterator<Item = (Vec3, Quat, Vec3)> + '_ {
        self.boxes
            .iter()
            .map(move |b| (self.pos + self.rot * b.center, self.rot * b.rot, b.half))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spawn {
    pub pos: Vec3,
    /// Hull yaw about +Y; 0 faces +X.
    pub yaw: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScrapSite {
    pub pos: Vec3,
    pub kind: ScrapKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PropKindSpawn {
    OilDrum,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PropSpawn {
    pub kind: PropKindSpawn,
    pub pos: Vec3,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Map {
    /// Which map this is (the client rebuilds it from the id and the seed).
    pub id: MapId,
    pub name: String,
    pub seed: u64,
    /// Side length of the heightfield, m.
    pub size: f32,
    /// Samples per side.
    pub resolution: usize,
    /// `heights[iz * resolution + ix]`, metres.
    pub heights: Vec<f32>,
    pub obstacles: Vec<Obstacle>,
    pub spawns: Vec<Spawn>,
    pub scrap_sites: Vec<ScrapSite>,
    pub repair_sites: Vec<Vec3>,
    pub props: Vec<PropSpawn>,
    /// Invisible walls stand at |x| = play_half and |z| = play_half.
    pub play_half: f32,
    /// Golden-crate sites (rare parts).
    pub rare_sites: Vec<Vec3>,
    /// Golden flare-crate site (air strike).
    pub strike_sites: Vec<Vec3>,
    /// Wrecking balls: which obstacle, the rope anchor (jib tip, world) and rope length.
    pub cranes: Vec<CraneSpawn>,
    /// Centre of THE OUTPOST compound (also the Raider Waves outpost).
    pub outpost: Vec3,
    /// Named places, for the radio and the minimap.
    pub regions: Vec<Region>,
    /// Convoy: the truck's route, start to finish (control points on the ground,
    /// along the road where there is one). The sim joins them on the nav grid.
    #[serde(default)]
    pub convoy_route: Vec<Vec3>,
    /// Wellhead: the three derricks, in order (team 0's side, the middle, team
    /// 1's side). Team spawns split across the line between the two sides.
    #[serde(default)]
    pub wells: Vec<WellSite>,
    /// Ground storytelling (visual only): dirt tracks between the places,
    /// packed ground in the compounds, oil stains by the pumps.
    #[serde(default)]
    pub marks: Vec<GroundMark>,
}

/// What a [`GroundMark`] paints on the ground.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroundMarkKind {
    /// A dirt track with two tyre ruts, along `points`.
    Track,
    /// Packed, trodden ground: a wide band along `points` (a compound floor).
    Packed,
    /// A dark oil stain round each of `points`.
    Oil,
}

/// A painted mark on the ground (visual only: the client drapes it over
/// the terrain). `width` is the track or band width, or the stain diameter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroundMark {
    pub kind: GroundMarkKind,
    pub points: Vec<Vec2>,
    pub width: f32,
}

/// A Wellhead derrick: where it stands and what the radio calls it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WellSite {
    pub pos: Vec3,
    /// "the gas station", "the beach" (lower case, with the article).
    pub name: String,
}


pub(crate) struct Pad {
    pub center: Vec2,
    pub inner: f32,
    pub outer: f32,
    pub target: f32,
}

impl Map {
    /// Builds map `id` from `seed`.
    pub fn by_id(id: MapId, seed: u64) -> Map {
        match id {
            MapId::Sink => Map::the_sink(seed),
            MapId::Refinery => Map::refinery(seed),
            MapId::RustBelt => Map::rust_belt(seed),
        }
    }

    pub fn spacing(&self) -> f32 {
        self.size / (self.resolution - 1) as f32
    }

    /// Height sample at grid indices.
    pub fn sample(&self, ix: usize, iz: usize) -> f32 {
        self.heights[iz * self.resolution + ix]
    }

    /// Ground height under (x, z), interpolated exactly like the collider.
    pub fn height_at(&self, x: f32, z: f32) -> f32 {
        let n = self.resolution;
        let sp = self.spacing();
        let fx = ((x + self.size / 2.0) / sp).clamp(0.0, (n - 1) as f32);
        let fz = ((z + self.size / 2.0) / sp).clamp(0.0, (n - 1) as f32);
        let j = (fx.floor() as usize).min(n - 2);
        let i = (fz.floor() as usize).min(n - 2);
        let u = fx - j as f32;
        let v = fz - i as f32;
        let y00 = self.sample(j, i);
        let y10 = self.sample(j, i + 1);
        let y01 = self.sample(j + 1, i);
        let y11 = self.sample(j + 1, i + 1);
        if u + v <= 1.0 {
            y00 + u * (y01 - y00) + v * (y10 - y00)
        } else {
            y11 + (1.0 - u) * (y10 - y11) + (1.0 - v) * (y01 - y11)
        }
    }

    /// Approximate ground normal by central differences.
    pub fn normal_at(&self, x: f32, z: f32) -> Vec3 {
        let e = 1.0;
        let dx = self.height_at(x + e, z) - self.height_at(x - e, z);
        let dz = self.height_at(x, z + e) - self.height_at(x, z - e);
        Vec3::new(-dx, 2.0 * e, -dz).normalize()
    }

    /// Is (x, z) inside an obstacle box (grown by `margin`) or outside the walls?
    pub fn blocked_xz(&self, p: Vec2, margin: f32) -> bool {
        if p.x.abs() > self.play_half - margin || p.y.abs() > self.play_half - margin {
            return true;
        }
        for o in &self.obstacles {
            if o.cover == Cover::Flat || Vec2::new(o.pos.x, o.pos.z).distance(p) > o.radius + margin + 2.0 {
                continue;
            }
            for (c, r, h) in o.world_boxes() {
                // Overhead boxes (a crane jib) don't block the ground.
                if c.y - h.y > o.pos.y + 4.0 {
                    continue;
                }
                let local = r.inverse() * (Vec3::new(p.x, c.y, p.y) - c);
                // Footprint test in the box's own XZ (boxes are only slightly tilted).
                if local.x.abs() < h.x + margin && local.z.abs() < h.z + margin {
                    return true;
                }
            }
        }
        false
    }

    /// The highest point of an obstacle above the ground beneath it.
    pub fn obstacle_top_above_ground(&self, o: &Obstacle) -> f32 {
        let mut top = f32::MIN;
        for (c, r, h) in o.world_boxes() {
            for sx in [-1.0, 1.0] {
                for sy in [-1.0, 1.0] {
                    for sz in [-1.0, 1.0] {
                        let p = c + r * Vec3::new(sx * h.x, sy * h.y, sz * h.z);
                        top = top.max(p.y - self.height_at(p.x, p.z));
                    }
                }
            }
        }
        top
    }
}

/// How far ahead of a spawn the ground must be clear and drivable.
pub const SPAWN_CLEAR: f32 = 30.0;

impl Map {
    /// Is the ground ahead of `p` along `yaw` (0 faces +X) open and drivable
    /// for `dist` m: no cover within 3 m of the line, nothing steep, inside
    /// the walls? (A new player holding W drives straight down it.)
    pub fn heading_clear(&self, p: Vec2, yaw: f32, dist: f32) -> bool {
        let dir = Vec2::new(yaw.cos(), -yaw.sin());
        let mut d = 3.0;
        while d <= dist {
            let q = p + dir * d;
            if q.x.abs() > self.play_half - 4.0 || q.y.abs() > self.play_half - 4.0 || self.blocked_xz(q, 3.0) || self.normal_at(q.x, q.y).y < 0.88 {
                return false;
            }
            d += 1.5;
        }
        true
    }

    /// Turns every spawn toward the action: the heading nearest the outpost
    /// (within 75 degrees of it) with four times [`SPAWN_CLEAR`] m of open
    /// ground ahead (a new player holding W for a good while), else twice,
    /// else at least [`SPAWN_CLEAR`].
    pub(crate) fn aim_spawns(&mut self) {
        let o = Vec2::new(self.outpost.x, self.outpost.z);
        for i in 0..self.spawns.len() {
            let p = Vec2::new(self.spawns[i].pos.x, self.spawns[i].pos.z);
            let d = o - p;
            let want = (-d.y).atan2(d.x);
            // Twice the minimum if we can get it (a long open run), else the minimum.
            'search: for dist in [4.0 * SPAWN_CLEAR, 2.0 * SPAWN_CLEAR, SPAWN_CLEAR] {
                for k in 0..=15 {
                    for sign in [1.0f32, -1.0] {
                        let yaw = want + sign * k as f32 * 5f32.to_radians();
                        if self.heading_clear(p, yaw, dist) {
                            self.spawns[i].yaw = yaw;
                            break 'search;
                        }
                    }
                }
            }
        }
    }
}

/// The rim round the play area: flat right up to the invisible wall, then
/// a steep rise of `height` m beyond it (a clean boundary: nothing inside
/// the wall a tank can climb at a silly angle). `edge` is the distance from
/// the centre along the axis (or the larger of |x| and |z|).
pub(crate) fn rim(play_half: f32, size: f32, edge: f32, height: f32) -> f32 {
    smoothstep(play_half + 0.5, size / 2.0, edge) * height
}

pub(crate) fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(crate) fn bx(center: Vec3, half: Vec3, yaw: f32) -> ObstacleBox {
    ObstacleBox { center, half, rot: Quat::from_rotation_y(yaw) }
}

pub(crate) fn make_obstacle(kind: ObstacleKind, rng: &mut Rng) -> Obstacle {
    let yaw = rng.range(0.0, std::f32::consts::TAU);
    let mut rot = Quat::from_rotation_y(yaw);
    let mut boxes = Vec::new();
    let cover;
    let radius;
    match kind {
        ObstacleKind::ShipHull => {
            cover = Cover::Tall;
            let l = rng.range(34.0, 46.0);
            let w = rng.range(9.0, 11.0);
            let h = rng.range(8.0, 10.0);
            let sink = 1.5;
            // Main hull, a narrower bow, the bridge tower aft.
            boxes.push(bx(Vec3::new(-l * 0.08, h / 2.0 - sink, 0.0), Vec3::new(l * 0.42, h / 2.0, w / 2.0), 0.0));
            boxes.push(bx(Vec3::new(l * 0.38, h / 2.0 - sink + 0.5, 0.0), Vec3::new(l * 0.12, h / 2.0, w * 0.32), 0.0));
            boxes.push(bx(Vec3::new(-l * 0.3, h - sink + 3.0, 0.0), Vec3::new(3.5, 3.0, w * 0.35), 0.0));
            // Beached: listing to one side.
            rot *= Quat::from_rotation_x(rng.range(-0.1, 0.1));
            radius = l * 0.5 + 2.0;
        }
        ObstacleKind::RockFormation => {
            cover = Cover::Tall;
            let n = 2 + rng.below(3);
            let mut rad: f32 = 0.0;
            for i in 0..n {
                let half = Vec3::new(rng.range(3.0, 6.5), rng.range(3.5, 7.0), rng.range(3.0, 6.5));
                let off = if i == 0 {
                    Vec3::ZERO
                } else {
                    let a = rng.range(0.0, std::f32::consts::TAU);
                    Vec3::new(a.cos(), 0.0, a.sin()) * rng.range(4.0, 8.0)
                };
                boxes.push(bx(off + Vec3::Y * (half.y - 1.0), half, rng.range(0.0, 3.0)));
                rad = rad.max(off.length() + half.x.max(half.z) * 1.42);
            }
            radius = rad;
        }
        ObstacleKind::ContainerStack => {
            cover = Cover::Tall;
            let cols = 1 + rng.below(3);
            let half = Vec3::new(6.1, 1.3, 1.22);
            for c in 0..cols {
                let levels = 1 + rng.below(3);
                for k in 0..levels {
                    let jitter = rng.range(-0.06, 0.06);
                    let slide = rng.range(-0.4, 0.4);
                    boxes.push(bx(
                        Vec3::new(slide, half.y + 2.6 * k as f32, (c as f32 - (cols - 1) as f32 / 2.0) * 2.5),
                        half,
                        jitter,
                    ));
                }
            }
            radius = 7.5 + cols as f32 * 0.6;
        }
        ObstacleKind::Barrier => {
            cover = Cover::Low;
            let n = 2 + rng.below(3);
            let seg = 1.7;
            for i in 0..n {
                let x = (i as f32 - (n - 1) as f32 / 2.0) * seg * 2.05;
                boxes.push(bx(Vec3::new(x, 0.45, 0.0), Vec3::new(seg, 0.5, 0.35), rng.range(-0.06, 0.06)));
            }
            radius = n as f32 * seg + 1.0;
        }
        ObstacleKind::Mesa => {
            cover = Cover::Tall;
            let half = Vec3::new(rng.range(13.0, 19.0), rng.range(7.0, 9.5), rng.range(10.0, 14.0));
            boxes.push(bx(Vec3::Y * (half.y - 1.5), half, 0.0));
            // A lower shoulder on one side.
            let s = Vec3::new(rng.range(6.0, 9.0), rng.range(3.5, 5.0), rng.range(5.0, 8.0));
            boxes.push(bx(Vec3::new(half.x * 0.7, s.y - 1.0, half.z + s.z * 0.5), s, rng.range(-0.4, 0.4)));
            radius = half.x.max(half.z) * 1.3 + 4.0;
        }
        ObstacleKind::Silo => {
            cover = Cover::Tall;
            let r = rng.range(3.5, 4.5);
            let h = rng.range(6.0, 8.5);
            boxes.push(bx(Vec3::Y * (h - 0.5), Vec3::new(r, h, r), 0.0));
            if rng.chance(0.5) {
                boxes.push(bx(Vec3::new(0.0, h - 1.0, 2.0 * r + 1.5), Vec3::new(r, h - 0.5, r), 0.0));
            }
            radius = 2.0 * r + 3.0;
        }
        ObstacleKind::Sandbags => {
            cover = Cover::Low;
            let n = 3 + rng.below(4);
            let bend = rng.range(-0.25, 0.25);
            let mut p = Vec3::ZERO;
            let mut a: f32 = 0.0;
            let mut pts = Vec::new();
            for _ in 0..n {
                pts.push((p, a));
                p += Vec3::new(a.cos(), 0.0, -a.sin()) * 2.9;
                a += bend;
            }
            let mid = pts.iter().map(|(q, _)| *q).sum::<Vec3>() / n as f32;
            for (q, a) in pts {
                boxes.push(bx(q - mid + Vec3::Y * 0.4, Vec3::new(1.5, 0.45, 0.55), a));
            }
            radius = n as f32 * 1.5 + 1.0;
        }
        ObstacleKind::Crates => {
            cover = Cover::Low;
            let n = 2 + rng.below(4);
            for i in 0..n {
                let a = rng.range(0.0, std::f32::consts::TAU);
                let off = if i == 0 { Vec3::ZERO } else { Vec3::new(a.cos(), 0.0, a.sin()) * rng.range(1.2, 2.4) };
                boxes.push(bx(off + Vec3::Y * 0.5, Vec3::splat(0.55), rng.range(0.0, 1.5)));
            }
            radius = 3.5;
        }
        ObstacleKind::FuelTank => {
            cover = Cover::Tall;
            let l = rng.range(3.5, 5.0);
            boxes.push(bx(Vec3::new(0.0, 1.75, 0.0), Vec3::new(l, 1.5, 1.5), 0.0));
            radius = l + 2.0;
        }
        ObstacleKind::Crane => {
            cover = Cover::Tall;
            boxes.push(bx(Vec3::new(0.0, 11.5, 0.0), Vec3::new(1.5, 12.0, 1.5), 0.0));
            boxes.push(bx(Vec3::new(7.0, 23.8, 0.0), Vec3::new(9.0, 0.6, 0.6), 0.0));
            radius = 18.0;
        }
        ObstacleKind::LaunchPlate => {
            cover = Cover::Flat;
            boxes.push(bx(Vec3::new(0.0, -0.25, 0.0), Vec3::new(3.0, 0.3, 2.6), 0.0));
            radius = 4.5;
        }
        ObstacleKind::Wall => {
            cover = Cover::Tall;
            let l = rng.range(5.0, 7.0);
            boxes.push(bx(Vec3::new(0.0, 1.35, 0.0), Vec3::new(l, 1.55, 0.35), 0.0));
            radius = l + 1.0;
        }
        ObstacleKind::Tent => {
            cover = Cover::Tall;
            boxes.push(bx(Vec3::new(0.0, 1.45, 0.0), Vec3::new(5.0, 1.65, 3.0), 0.0));
            radius = 6.5;
        }
        ObstacleKind::Tower => {
            cover = Cover::Tall;
            boxes.push(bx(Vec3::new(0.0, 3.9, 0.0), Vec3::new(2.0, 4.0, 2.0), 0.0));
            radius = 3.5;
        }
        ObstacleKind::GasStation => {
            cover = Cover::Tall;
            boxes.push(bx(Vec3::new(0.0, 1.7, -7.0), Vec3::new(6.0, 1.9, 4.0), 0.0));
            boxes.push(bx(Vec3::new(0.0, 5.2, 7.0), Vec3::new(8.0, 0.3, 5.5), 0.0));
            for (x, z) in [(-6.5, 2.5), (6.5, 2.5), (-6.5, 11.5), (6.5, 11.5)] {
                boxes.push(bx(Vec3::new(x, 2.4, z), Vec3::new(0.3, 2.5, 0.3), 0.0));
            }
            radius = 14.0;
        }
        ObstacleKind::Hedgehog => {
            cover = Cover::Low;
            boxes.push(bx(Vec3::new(0.0, 0.45, 0.0), Vec3::new(0.75, 0.5, 0.75), rng.range(0.0, 1.5)));
            radius = 1.5;
        }
        ObstacleKind::PowerPole => {
            cover = Cover::Tall;
            boxes.push(bx(Vec3::new(0.0, 4.4, 0.0), Vec3::new(0.2, 4.6, 0.2), 0.0));
            boxes.push(bx(Vec3::new(0.0, 8.6, 0.0), Vec3::new(0.12, 0.12, 1.3), 0.0));
            radius = 1.5;
        }
        ObstacleKind::Highway => {
            // Built by the_sink directly (it follows the road profile).
            cover = Cover::Flat;
            radius = 8.0;
        }
        ObstacleKind::Pipe => {
            cover = Cover::Low;
            let l = rng.range(4.0, 6.0);
            boxes.push(bx(Vec3::new(0.0, 0.5, 0.0), Vec3::new(l, 0.55, 0.55), 0.0));
            radius = l + 1.0;
        }
        ObstacleKind::CarWreck => {
            cover = Cover::Low;
            boxes.push(bx(Vec3::new(0.0, 0.45, 0.0), Vec3::new(2.2, 0.5, 0.95), 0.0));
            radius = 3.2;
        }
        ObstacleKind::StorageTank => {
            cover = Cover::Tall;
            let r = rng.range(7.0, 8.5);
            let h = rng.range(4.6, 5.6);
            boxes.push(bx(Vec3::Y * (h - 0.4), Vec3::new(r, h, r), 0.0));
            rot = Quat::IDENTITY;
            radius = r + 3.0;
        }
        ObstacleKind::Column => {
            cover = Cover::Tall;
            let r = rng.range(1.8, 2.6);
            let h = rng.range(10.0, 14.0);
            boxes.push(bx(Vec3::Y * (h - 0.4), Vec3::new(r, h, r), 0.0));
            rot = Quat::IDENTITY;
            radius = r + 2.5;
        }
        ObstacleKind::PipeRack => {
            cover = Cover::Tall;
            pipe_rack_boxes(&mut boxes, 28.0);
            radius = 16.0;
        }
        ObstacleKind::Pumpjack => {
            cover = Cover::Tall;
            boxes.push(bx(Vec3::new(-0.6, 1.35, 0.0), Vec3::new(3.0, 1.55, 1.1), 0.0));
            radius = 5.5;
        }
        ObstacleKind::Derrick => {
            cover = Cover::Tall;
            for (sx, sz) in [(-1.0f32, -1.0f32), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
                boxes.push(bx(Vec3::new(sx * 3.7, 4.3, sz * 3.7), Vec3::new(0.35, 4.6, 0.35), 0.0));
            }
            boxes.push(bx(Vec3::new(0.0, 15.6, 0.0), Vec3::new(2.1, 6.6, 2.1), 0.0));
            radius = 6.5;
        }
        ObstacleKind::TankCar => {
            cover = Cover::Tall;
            boxes.push(bx(Vec3::new(0.0, 2.05, 0.0), Vec3::new(6.6, 2.05, 1.5), 0.0));
            radius = 7.5;
        }
        ObstacleKind::Rails => {
            cover = Cover::Flat;
            boxes.push(bx(Vec3::new(0.0, -0.2, 0.0), Vec3::new(8.0, 0.3, 1.6), 0.0));
            radius = 8.5;
        }
        ObstacleKind::FlareStack => {
            cover = Cover::Tall;
            boxes.push(bx(Vec3::new(0.0, 1.3, 0.0), Vec3::new(3.4, 1.6, 3.4), 0.0));
            boxes.push(bx(Vec3::new(0.0, 22.0, 0.0), Vec3::new(1.2, 19.5, 1.2), 0.0));
            radius = 6.0;
        }
        ObstacleKind::ControlHouse => {
            cover = Cover::Tall;
            boxes.push(bx(Vec3::new(0.0, 2.2, 0.0), Vec3::new(8.0, 2.6, 4.6), 0.0));
            radius = 10.5;
        }
        ObstacleKind::WaterTower => {
            cover = Cover::Tall;
            for (sx, sz) in [(-1.0f32, -1.0f32), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
                boxes.push(bx(Vec3::new(sx * 2.6, 4.2, sz * 2.6), Vec3::new(0.3, 4.4, 0.3), 0.0));
            }
            boxes.push(bx(Vec3::new(0.0, 11.0, 0.0), Vec3::new(3.2, 2.6, 3.2), 0.0));
            rot = Quat::IDENTITY;
            radius = 5.0;
        }
        ObstacleKind::RadioMast => {
            cover = Cover::Tall;
            boxes.push(bx(Vec3::new(0.0, 12.0, 0.0), Vec3::new(1.1, 12.2, 1.1), 0.0));
            radius = 2.5;
        }
        ObstacleKind::Windmill => {
            cover = Cover::Tall;
            boxes.push(bx(Vec3::new(0.0, 5.6, 0.0), Vec3::new(0.9, 5.8, 0.9), 0.0));
            radius = 2.5;
        }
        ObstacleKind::CarCube => {
            cover = Cover::Tall;
            let cols = 2 + rng.below(3);
            for c in 0..cols {
                let n = 2 + rng.below(3);
                car_cube_column(&mut boxes, (c as f32 - (cols - 1) as f32 / 2.0) * CUBE_W, n as u32, rng);
            }
            radius = cols as f32 * CUBE_W * 0.5 + 1.5;
        }
        ObstacleKind::Crusher => {
            cover = Cover::Tall;
            boxes.push(bx(Vec3::new(0.0, 1.9, 0.0), Vec3::new(8.0, 2.0, 4.0), 0.0));
            boxes.push(bx(Vec3::new(-2.0, 6.6, 0.0), Vec3::new(4.0, 2.7, 3.6), 0.0));
            radius = 10.5;
        }
        ObstacleKind::MagnetCrane => {
            cover = Cover::Tall;
            boxes.push(bx(Vec3::new(0.0, 10.2, 0.0), Vec3::new(1.6, 10.6, 1.6), 0.0));
            boxes.push(bx(Vec3::new(MAGNET_JIB * 0.5 - 4.0, 21.4, 0.0), Vec3::new(MAGNET_JIB * 0.5 + 4.0, 0.7, 0.7), 0.0));
            radius = 6.0;
        }
        ObstacleKind::Bus => {
            cover = Cover::Tall;
            boxes.push(bx(Vec3::new(0.0, BUS_HALF.y, 0.0), BUS_HALF, 0.0));
            radius = BUS_HALF.x + 1.5;
        }
        ObstacleKind::Shack => {
            cover = Cover::Tall;
            boxes.push(bx(Vec3::new(0.0, 1.7, 0.0), SHACK_HALF, 0.0));
            radius = 5.5;
        }
        ObstacleKind::Stall => {
            cover = Cover::Low;
            boxes.push(bx(Vec3::new(0.0, 0.45, 0.0), Vec3::new(2.2, 0.5, 0.9), 0.0));
            radius = 3.0;
        }
        ObstacleKind::Fence => {
            cover = Cover::Tall;
            let l = rng.range(5.0, 7.0);
            boxes.push(bx(Vec3::new(0.0, 1.35, 0.0), Vec3::new(l, 1.55, 0.3), 0.0));
            radius = l + 1.0;
        }
        ObstacleKind::Bridge => {
            // Built by rust_belt (it spans the river bed); a default 7 m drop.
            cover = Cover::Tall;
            bridge_boxes(&mut boxes, 7.0, 25.0);
            rot = Quat::IDENTITY;
            radius = 26.0;
        }
        ObstacleKind::CarTower => {
            cover = Cover::Tall;
            boxes.push(bx(Vec3::new(0.0, 6.0, 0.0), Vec3::new(3.0, 6.0, 3.0), 0.0));
            boxes.push(bx(Vec3::new(0.0, 17.5, 0.0), Vec3::new(2.6, 5.5, 2.6), 0.5));
            radius = 6.0;
        }
        ObstacleKind::PierRuin => {
            cover = Cover::Low;
            let n = 2 + rng.below(3);
            for i in 0..n {
                boxes.push(bx(
                    Vec3::new(i as f32 * 7.5, 0.4, rng.range(-1.0, 1.0)),
                    Vec3::new(3.4, 0.45, 2.2),
                    rng.range(-0.25, 0.25),
                ));
            }
            // Centre the run on the obstacle origin.
            let mid = (n - 1) as f32 * 7.5 / 2.0;
            for b in &mut boxes {
                b.center.x -= mid;
            }
            radius = mid + 4.5;
        }
    }
    Obstacle { kind, cover, pos: Vec3::ZERO, rot, boxes, radius }
}

/// A pipe rack `len` long (along local X): the overhead bundle, then a
/// pair of legs every 14 m or so. 10 m wide, clear 5.8 m underneath.
pub(crate) fn pipe_rack_boxes(boxes: &mut Vec<ObstacleBox>, len: f32) {
    boxes.push(bx(Vec3::new(0.0, 6.6, 0.0), Vec3::new(len / 2.0, 0.8, 5.4), 0.0));
    let bents = (len / 14.0).round().max(1.0) as i32;
    for k in 0..=bents {
        let x = -len / 2.0 + 0.4 + k as f32 * (len - 0.8) / bents as f32;
        for z in [-5.0f32, 5.0] {
            boxes.push(bx(Vec3::new(x, 2.9, z), Vec3::new(0.3, 3.0, 0.3), 0.0));
        }
    }
}

/// A crushed car bale: 2.8 m wide (along a cube wall), 1.35 m high.
pub const CUBE_W: f32 = 2.8;
pub const CUBE_H: f32 = 1.35;
/// A school bus's half extents (long along X).
pub const BUS_HALF: Vec3 = Vec3::new(6.2, 1.6, 1.35);
/// A tin shack's half extents.
pub const SHACK_HALF: Vec3 = Vec3::new(4.0, 1.7, 3.0);
/// How far the magnet crane's jib reaches out from the tower.
pub const MAGNET_JIB: f32 = 30.0;

/// One column of `n` car bales at local x, standing on y = 0.
pub(crate) fn car_cube_column(boxes: &mut Vec<ObstacleBox>, x: f32, n: u32, rng: &mut Rng) {
    let hy = n as f32 * CUBE_H * 0.5;
    boxes.push(bx(Vec3::new(x, hy, rng.range(-0.12, 0.12)), Vec3::new(CUBE_W * 0.5, hy, 1.0), rng.range(-0.04, 0.04)));
}

/// The bridge, in a frame standing on the river bed: the deck (along local
/// Z, `half_len` long each way, its top `drop` m up) and the pier under it.
pub(crate) fn bridge_boxes(boxes: &mut Vec<ObstacleBox>, drop: f32, half_len: f32) {
    let under = drop - 1.15;
    boxes.push(bx(Vec3::new(0.0, drop + 0.05 - 0.6, 0.0), Vec3::new(7.0, 0.6, half_len), 0.0));
    boxes.push(bx(Vec3::new(0.0, under * 0.5, 0.0), Vec3::new(5.0, under * 0.5, 0.8), 0.0));
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CraneSpawn {
    /// Index into `Map::obstacles`.
    pub obstacle: usize,
    /// World position of the jib tip the rope hangs from.
    pub anchor: Vec3,
    /// Rope length, m (the ball swings about 2.6 m above the ground).
    pub rope: f32,
}

/// A named place on the map.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Region {
    pub name: String,
    pub center: Vec3,
    pub radius: f32,
}
