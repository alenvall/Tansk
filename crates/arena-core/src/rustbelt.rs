//! THE RUST BELT, laid out by hand: a dead scrapyard city in a dry canyon.
//!
//! ```text
//!                         N (-z): the canyon's north wall
//!   THE CRUSHER (NW: the crusher,     road     THE BUS GRAVEYARD (NE: rows of
//!     a maze of car-cube stacks)       |         rusted school buses)
//!                          THE SALVAGE MARKET (fenced tin shacks and stalls:
//!                                      |        the outpost)
//!   ~~~~~~~~~~ THE DRY RIVER (a sunken bed, four fords) ==BRIDGE== ~~~~~~~~~~
//!                                      |
//!   THE MAGNET CRANE (SW: a crane       |        THE TOWER (SE: the tower of
//!     over a pile of wrecks)           road       stacked cars in a ring of cubes)
//!                         S (+z): the canyon's south wall
//! ```
//!
//! Same rules as the other maps: low cover under 1.2 m, tall cover 2.5 m and
//! up (the bridge deck and the crane jib are tall *overhead*: tanks drive
//! under them), lanes at least 8 m wide or nothing, eight spawns round the
//! edge, every region worth a visit. The river bed is the second level: the
//! bridge carries the road over it and you can drive under the bridge along
//! the bed; the fords are the ways down. The seed only jitters details.

use glam::{Quat, Vec2, Vec3};

use crate::map::*;
use crate::rng::{value_noise, Rng};

struct Builder {
    obstacles: Vec<Obstacle>,
    rng: Rng,
}

impl Builder {
    fn push(&mut self, mut ob: Obstacle, x: f32, z: f32, yaw: f32) -> usize {
        ob.pos = Vec3::new(x, 0.0, z);
        ob.rot = Quat::from_rotation_y(yaw);
        self.obstacles.push(ob);
        self.obstacles.len() - 1
    }

    fn place(&mut self, kind: ObstacleKind, x: f32, z: f32, yaw: f32) -> usize {
        let ob = make_obstacle(kind, &mut self.rng);
        self.push(ob, x, z, yaw)
    }

    /// A straight tin fence from a to b.
    fn fence(&mut self, a: Vec2, b: Vec2) {
        let d = b - a;
        let l = d.length();
        let c = (a + b) * 0.5;
        let ob = Obstacle {
            kind: ObstacleKind::Fence,
            cover: Cover::Tall,
            pos: Vec3::ZERO,
            rot: Quat::IDENTITY,
            boxes: vec![bx(Vec3::new(0.0, 1.35, 0.0), Vec3::new(l / 2.0 + 0.3, 1.55, 0.3), 0.0)],
            radius: l / 2.0 + 1.0,
        };
        self.push(ob, c.x, c.y, face(a, b));
    }

    /// A wall of car cubes: columns side by side along local X, each
    /// `levels[i]` bales high (2..=4).
    fn cubes(&mut self, x: f32, z: f32, yaw: f32, levels: &[u8]) {
        let cols = levels.len();
        let mut boxes = Vec::new();
        for (c, &n) in levels.iter().enumerate() {
            car_cube_column(&mut boxes, (c as f32 - (cols - 1) as f32 / 2.0) * CUBE_W, n.clamp(2, 4) as u32, &mut self.rng);
        }
        let ob = Obstacle { kind: ObstacleKind::CarCube, cover: Cover::Tall, pos: Vec3::ZERO, rot: Quat::IDENTITY, boxes, radius: cols as f32 * CUBE_W * 0.5 + 1.5 };
        self.push(ob, x, z, yaw);
    }

    /// A rectangular ring of cube walls round `c` (half extents `hx`, `hz`)
    /// with doors `w` m wide at (side, offset along it): 0 north, 1 south,
    /// 2 west, 3 east. Walls meet at the corners and stop clean at the doors
    /// (no slots a tank could wedge in).
    fn cube_ring(&mut self, c: Vec2, hx: f32, hz: f32, doors: &[(u8, f32)], w: f32) {
        for side in 0..4u8 {
            let half = if side < 2 { hx + 1.0 } else { hz - 1.0 };
            let mut cuts: Vec<f32> = doors.iter().filter(|d| d.0 == side).map(|d| d.1).collect();
            cuts.sort_by(f32::total_cmp);
            let mut segs = Vec::new();
            let (mut start, mut corner) = (-half, true);
            for d in cuts {
                segs.push((start, d - w / 2.0, corner, false));
                start = d + w / 2.0;
                corner = false;
            }
            segs.push((start, half, corner, true));
            for (s0, s1, c0, c1) in segs {
                let len = s1 - s0;
                // Corner to corner: overshoot into the corners; at a door: stop short.
                let cols = if c0 && c1 { (len / CUBE_W).ceil() } else { (len / CUBE_W).floor() } as usize;
                if cols < 1 {
                    continue;
                }
                let used = cols as f32 * CUBE_W;
                let mid = if c0 && !c1 { s0 + used / 2.0 } else if c1 && !c0 { s1 - used / 2.0 } else { (s0 + s1) / 2.0 };
                let levels: Vec<u8> = (0..cols).map(|_| 2 + self.rng.below(3) as u8).collect();
                let (p, yaw) = match side {
                    0 => (c + Vec2::new(mid, -hz), 0.0),
                    1 => (c + Vec2::new(mid, hz), 0.0),
                    2 => (c + Vec2::new(-hx, mid), std::f32::consts::FRAC_PI_2),
                    _ => (c + Vec2::new(hx, mid), std::f32::consts::FRAC_PI_2),
                };
                self.cubes(p.x, p.y, yaw, &levels);
            }
        }
    }

    /// A school bus, maybe with another one stacked on its roof.
    fn bus(&mut self, x: f32, z: f32, yaw: f32, stacked: bool) {
        let mut boxes = vec![bx(Vec3::new(0.0, BUS_HALF.y, 0.0), BUS_HALF, 0.0)];
        if stacked {
            let slide = self.rng.range(-0.8, 0.8);
            let twist = self.rng.range(-0.12, 0.12);
            boxes.push(bx(Vec3::new(slide, 3.0 * BUS_HALF.y + 0.02, 0.0), BUS_HALF, twist));
        }
        let ob = Obstacle { kind: ObstacleKind::Bus, cover: Cover::Tall, pos: Vec3::ZERO, rot: Quat::IDENTITY, boxes, radius: BUS_HALF.x + 1.5 };
        self.push(ob, x, z, yaw);
    }

    /// A flush road deck from a to b, in `seg` m pieces.
    fn road(&mut self, a: Vec2, b: Vec2, seg: f32, half_width: f32) {
        let d = b - a;
        let len = d.length();
        let dir = d / len;
        // Whole pieces end to end, stretched to fill the run exactly.
        let n = (len / seg).round().max(1.0) as i32;
        let seg = len / n as f32;
        for k in 0..n {
            let t = (k as f32 + 0.5) * seg;
            let c = a + dir * t;
            let ob = Obstacle {
                kind: ObstacleKind::Highway,
                cover: Cover::Flat,
                pos: Vec3::ZERO,
                rot: Quat::IDENTITY,
                boxes: vec![bx(Vec3::new(0.0, -0.25, 0.0), Vec3::new(seg / 2.0 + 0.05, 0.3, half_width), 0.0)],
                radius: seg / 2.0,
            };
            self.push(ob, c.x, c.y, face(a, b));
        }
    }

    fn jit(&mut self, v: f32) -> f32 {
        self.rng.range(-v, v)
    }
}

/// Facing (yaw) from `p` toward `q`.
fn face(p: Vec2, q: Vec2) -> f32 {
    let d = q - p;
    (-d.y).atan2(d.x)
}

// ---- The dry river ----------------------------------------------------------

/// The river's centre line: it bulges south under the bridge (x = 0).
pub fn river_z(x: f32) -> f32 {
    RIVER_Z + RIVER_AMP * (x / RIVER_K).cos()
}
const RIVER_Z: f32 = 60.0;
const RIVER_AMP: f32 = 20.0;
const RIVER_K: f32 = 80.0;
/// Half width of the flat river bed.
pub const BED_HALF: f32 = 15.0;
/// How far down the bed is.
const DEPTH: f32 = 7.0;
/// Bank widths: steep (a cliff for tanks) except at the fords.
const BANK: f32 = 6.0;
const FORD_BANK: f32 = 32.0;
/// The fords: gentle ramps down into the bed on both banks.
pub const FORDS: [f32; 4] = [-205.0, -95.0, 100.0, 205.0];
/// Bridge deck half length (it reaches 4 m past each cliff top).
const BRIDGE_HALF: f32 = 25.0;

fn bank_width(x: f32) -> f32 {
    let f = FORDS.iter().map(|xf| 1.0 - smoothstep(10.0, 26.0, (x - xf).abs())).fold(0.0, f32::max);
    BANK + (FORD_BANK - BANK) * f
}

/// 1 in the river bed, 0 on the plain, the bank between.
fn river_prof(x: f32, z: f32) -> f32 {
    1.0 - smoothstep(BED_HALF, BED_HALF + bank_width(x), (z - river_z(x)).abs())
}

// ---- Places -------------------------------------------------------------------
const MARKET: Vec2 = Vec2::new(0.0, -30.0);
const MARKET_HALF: f32 = 36.0;
const CRUSHER: Vec2 = Vec2::new(-165.0, -150.0);
const BUSES: Vec2 = Vec2::new(165.0, -158.0);
const PILE: Vec2 = Vec2::new(-150.0, 182.0);
const PILE_HEIGHT: f32 = 2.5;
const TOWER: Vec2 = Vec2::new(165.0, 182.0);
const ROAD_HALF: f32 = 6.0;

/// The Rust Belt's ground storytelling: tracks from the market's gates to
/// each yard and down the fords, the convoy's track along the river bed,
/// packed ground in the market and the yards, silt in the bed, oil round
/// the crusher, the wrecks and the buses.
fn rust_marks(map: &Map) -> Vec<GroundMark> {
    let v = |pts: &[(f32, f32)]| pts.iter().map(|&(x, z)| Vec2::new(x, z)).collect::<Vec<_>>();
    let track = |pts: &[(f32, f32)]| GroundMark { kind: GroundMarkKind::Track, points: v(pts), width: 5.0 };
    let mut marks = vec![
        // West gate to the crusher.
        track(&[(-38.0, -30.0), (-80.0, -40.0), (-122.0, -70.0), (-150.0, -98.0), (-150.0, -112.0)]),
        // East gate to the bus graveyard.
        track(&[(38.0, -30.0), (80.0, -45.0), (118.0, -80.0), (150.0, -100.0)]),
        // Down the fords and across.
        track(&[(-205.0, -20.0), (-205.0, 20.0), (-205.0, 60.0), (-205.0, 100.0)]),
        track(&[(-95.0, 5.0), (-95.0, 40.0), (-95.0, 80.0), (-95.0, 120.0), (-120.0, 150.0)]),
        track(&[(100.0, 10.0), (100.0, 45.0), (100.0, 85.0), (100.0, 125.0), (130.0, 160.0)]),
        track(&[(205.0, -10.0), (205.0, 25.0), (205.0, 65.0), (205.0, 100.0)]),
        // Off the south road to the crane and the tower.
        track(&[(-8.0, 150.0), (-50.0, 160.0), (-100.0, 170.0)]),
        track(&[(8.0, 150.0), (60.0, 165.0), (130.0, 180.0)]),
    ];
    // The convoy's truck track along the bed.
    let bed: Vec<(f32, f32)> = (0..=15).map(|k| {
        let x = -255.0 + k as f32 * 30.0;
        (x, river_z(x))
    }).collect();
    marks.push(GroundMark { kind: GroundMarkKind::Track, points: v(&bed), width: 5.0 });
    let packed = |a: (f32, f32), b: (f32, f32), w: f32| GroundMark { kind: GroundMarkKind::Packed, points: v(&[a, b]), width: w };
    marks.extend([
        // The market, fence to fence.
        packed((MARKET.x - 30.0, MARKET.y), (MARKET.x + 30.0, MARKET.y), 68.0),
        // The crusher's yard.
        packed((CRUSHER.x - 40.0, CRUSHER.y + 5.0), (CRUSHER.x + 40.0, CRUSHER.y + 5.0), 60.0),
        // The bus lot.
        packed((BUSES.x - 55.0, BUSES.y), (BUSES.x + 55.0, BUSES.y), 92.0),
        // The tower's plaza.
        packed((TOWER.x - 12.0, TOWER.y), (TOWER.x + 12.0, TOWER.y), 40.0),
    ]);
    // Dried silt down the river bed.
    let silt: Vec<Vec2> = (0..=20).map(|k| {
        let x = -280.0 + k as f32 * 28.0;
        Vec2::new(x, river_z(x))
    }).collect();
    marks.push(GroundMark { kind: GroundMarkKind::Packed, points: silt, width: 2.0 * BED_HALF - 4.0 });
    let mut oil = vec![CRUSHER + Vec2::new(-12.0, 8.0), CRUSHER + Vec2::new(10.0, 8.0), MARKET + Vec2::new(-14.0, 12.0)];
    let mut big = vec![CRUSHER + Vec2::new(0.0, 9.0)];
    for o in &map.obstacles {
        let p = Vec2::new(o.pos.x, o.pos.z);
        match o.kind {
            ObstacleKind::CarWreck | ObstacleKind::FuelTank => oil.push(p),
            ObstacleKind::Bus if o.boxes.len() > 1 => oil.push(p + Vec2::new(3.0, 2.5)),
            ObstacleKind::CarTower => big.push(p),
            _ => {}
        }
    }
    marks.push(GroundMark { kind: GroundMarkKind::Oil, points: oil, width: 6.0 });
    marks.push(GroundMark { kind: GroundMarkKind::Oil, points: big, width: 11.0 });
    marks
}

impl Map {
    /// "The Rust Belt": a scrapyard city in a dry canyon. See the module
    /// docs for the layout.
    pub fn rust_belt(seed: u64) -> Map {
        let size = 600.0;
        let resolution = 151;
        let play_half = 285.0;
        let mut b = Builder { obstacles: Vec::new(), rng: Rng::new(seed ^ 0x7255_7BE1) };
        let s1 = seed.wrapping_mul(41).wrapping_add(7);
        let s2 = seed.wrapping_mul(191).wrapping_add(13);

        // ---- Terrain: the canyon floor, the wreck pile, the river ------------
        let base = move |x: f32, z: f32| -> f32 {
            let broad = (value_noise(s1, x / 150.0, z / 150.0) - 0.5) * 2.0;
            let small = (value_noise(s2, x / 30.0, z / 30.0) - 0.5) * 0.3;
            let pile = PILE_HEIGHT * (1.0 - smoothstep(6.0, 42.0, Vec2::new(x, z).distance(PILE)));
            broad + small + pile
        };
        // The bed's level: the plain along the river, smoothed, less the depth.
        let bed = move |x: f32| -> f32 {
            let mut s = 0.0;
            for k in -3..=3 {
                let xx = x + k as f32 * 12.0;
                s += base(xx, river_z(xx));
            }
            s / 7.0 - DEPTH
        };
        // The banks at the bridge: both ends level, the deck's height.
        let bridge_z = river_z(0.0);
        let bank_top = bed(0.0) + DEPTH + 0.2;
        // The road (N-S along x = 0): flat along its length, at the bridge's
        // height near the river.
        let road_level = move |z: f32| -> f32 {
            let mut s = 0.0;
            for k in -3..=3 {
                s += base(0.0, z + k as f32 * 10.0);
            }
            let near = 1.0 - smoothstep(32.0, 52.0, (z - bridge_z).abs());
            s / 7.0 + (bank_top - s / 7.0) * near
        };
        let corridor = move |x: f32, z: f32, h: f32| -> f32 {
            let w = 1.0 - smoothstep(ROAD_HALF + 2.0, ROAD_HALF + 12.0, x.abs());
            if w <= 0.0 || z.abs() > play_half {
                return h;
            }
            h + (road_level(z) - h) * w
        };
        let carve = move |x: f32, z: f32, h: f32| -> f32 {
            let p = river_prof(x, z);
            if p <= 0.0 {
                return h;
            }
            h + (bed(x) - h) * p
        };

        // ---- Spawns: round the edge, each with a little cover ahead ----------
        let spawn_xz = [
            Vec2::new(-60.0, -250.0),
            Vec2::new(110.0, -252.0),
            Vec2::new(252.0, -112.0),
            Vec2::new(255.0, 135.0),
            Vec2::new(115.0, 252.0),
            Vec2::new(-80.0, 252.0),
            Vec2::new(-255.0, 140.0),
            Vec2::new(-252.0, -95.0),
        ];
        let mut spawns = Vec::new();
        for (i, s) in spawn_xz.into_iter().enumerate() {
            let p = s + Vec2::new(b.jit(3.0), b.jit(3.0));
            spawns.push(Spawn { pos: Vec3::new(p.x, 0.0, p.y), yaw: face(p, Vec2::ZERO) });
            let toward = (-p).normalize();
            let side = Vec2::new(-toward.y, toward.x);
            let c = p + toward * 12.0 + side * 15.0;
            // Alternate: a sandbag line, a dead car.
            let k = if i % 2 == 0 { ObstacleKind::Sandbags } else { ObstacleKind::CarWreck };
            b.place(k, c.x, c.y, face(p, Vec2::ZERO) + std::f32::consts::FRAC_PI_2);
        }

        // ---- THE SALVAGE MARKET: the fenced outpost --------------------------
        let (mx, mz, h) = (MARKET.x, MARKET.y, MARKET_HALF);
        for side in 0..4 {
            for (a, c) in [(-h, -7.0), (7.0, h)] {
                let (p, q) = match side {
                    0 => (Vec2::new(mx + a, mz - h), Vec2::new(mx + c, mz - h)),
                    1 => (Vec2::new(mx + a, mz + h), Vec2::new(mx + c, mz + h)),
                    2 => (Vec2::new(mx - h, mz + a), Vec2::new(mx - h, mz + c)),
                    _ => (Vec2::new(mx + h, mz + a), Vec2::new(mx + h, mz + c)),
                };
                b.fence(p, q);
            }
        }
        // Tin shacks backed onto the fence: one in each corner (touching both
        // fences) and one by each gate, alcoves between them.
        let sh = SHACK_HALF;
        let inner = h - 0.32;
        for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            // Corner: long along X, against the N/S fence and the W/E fence.
            b.place(ObstacleKind::Shack, mx + sx * (inner - sh.x), mz + sz * (inner - sh.z), 0.0);
            // By the N/S gate (long along X, against the N/S fence).
            b.place(ObstacleKind::Shack, mx + sx * (7.5 + sh.x), mz + sz * (inner - sh.z), 0.0);
            // By the W/E gate (long along Z, against the W/E fence).
            b.place(ObstacleKind::Shack, mx + sx * (inner - sh.z), mz + sz * (7.5 + sh.x), std::f32::consts::FRAC_PI_2);
        }
        // Stalls in a ring round the courtyard, off the gate lines.
        for k in 0..8 {
            let a = (k as f32 + 0.5) * std::f32::consts::FRAC_PI_4;
            let r = 19.5 + if k % 2 == 0 { 0.0 } else { 1.0 };
            let p = MARKET + Vec2::new(a.cos(), a.sin()) * r;
            b.place(ObstacleKind::Stall, p.x, p.y, face(p, MARKET) + std::f32::consts::FRAC_PI_2);
        }
        // The lookout over the south gate.
        b.place(ObstacleKind::Tower, mx - 15.0, mz + h + 14.0, 0.1);
        // Hedgehogs and barriers out front of the gates.
        for (x, z, yaw) in [(mx - 14.0, mz - h - 12.0, 0.0), (mx + 14.0, mz + h + 12.0, 0.0), (mx - h - 12.0, mz - 13.0, 1.57), (mx + h + 12.0, mz + 13.0, 1.57)] {
            let j = b.jit(2.0);
            b.place(ObstacleKind::Barrier, x + j, z, yaw);
        }
        for (x, z) in [(mx - h - 10.0, mz - h - 10.0), (mx + h + 10.0, mz - h - 10.0), (mx - h - 10.0, mz + h + 10.0), (mx + h + 10.0, mz + h + 10.0)] {
            let (jx, jz) = (b.jit(2.0), b.jit(2.0));
            b.place(ObstacleKind::Hedgehog, x + jx, z + jz, 0.0);
        }

        // ---- The road, N-S through the market and over the bridge ------------
        let deck_a = bridge_z - BRIDGE_HALF;
        let deck_b = bridge_z + BRIDGE_HALF;
        b.road(Vec2::new(0.0, -272.0), Vec2::new(0.0, mz - h - 2.0), 12.0, ROAD_HALF);
        b.road(Vec2::new(0.0, mz + h + 2.0), Vec2::new(0.0, deck_a), 12.0, ROAD_HALF);
        b.road(Vec2::new(0.0, deck_b), Vec2::new(0.0, 272.0), 12.0, ROAD_HALF);
        let mut z = mz + h + 3.0;
        while z < 262.0 {
            // Poles down the road's east shoulder south of the market (not in the river).
            let in_market = (z - mz).abs() < h + 6.0;
            let in_river = (z - bridge_z).abs() < BRIDGE_HALF + 8.0;
            if !in_market && !in_river {
                b.place(ObstacleKind::PowerPole, ROAD_HALF + 11.0, z, 0.0);
            }
            z += 42.0;
        }
        // The north checkpoint and the south roadblock.
        for (x, z, k, yaw) in [
            (-13.0, -128.0, ObstacleKind::Sandbags, 0.0),
            (14.0, -118.0, ObstacleKind::Barrier, 0.0),
            (-14.0, 168.0, ObstacleKind::CarWreck, 0.4),
            (15.0, 178.0, ObstacleKind::CarWreck, -0.3),
            (-15.0, 186.0, ObstacleKind::Hedgehog, 0.0),
        ] {
            let j = b.jit(0.15);
            b.place(k, x, z, yaw + j);
        }

        // ---- THE CRUSHER: the machine in a maze of car cubes -----------------
        b.place(ObstacleKind::Crusher, CRUSHER.x, CRUSHER.y, 0.0);
        // Two rings of cube stacks round it, their doors staggered: a maze.
        b.cube_ring(CRUSHER, 32.0, 26.0, &[(2, 0.0), (3, -8.0), (0, 12.0)], 13.0);
        b.cube_ring(CRUSHER, 56.0, 46.0, &[(0, -24.0), (0, 28.0), (1, 15.0), (1, -32.0), (2, 12.0), (3, -14.0), (3, 26.0)], 13.0);
        // Inside: wrecks waiting their turn by the feed, the hydraulic fuel.
        b.place(ObstacleKind::CarWreck, CRUSHER.x + 14.0, CRUSHER.y + 14.5, 0.0);
        b.place(ObstacleKind::CarWreck, CRUSHER.x - 12.0, CRUSHER.y + 14.5, 0.1);
        b.place(ObstacleKind::FuelTank, CRUSHER.x - 2.0, CRUSHER.y - 14.5, 0.0);
        // A launch plate out of the yard toward the market.
        b.place(ObstacleKind::LaunchPlate, -96.0, -78.0, 0.6);

        // ---- THE BUS GRAVEYARD: long rows of rusted school buses ------------
        for (row, z) in [-202.0f32, -174.0, -146.0, -118.0].into_iter().enumerate() {
            let x0 = 106.0 + if row % 2 == 1 { 11.7 } else { 0.0 };
            let n = if row % 2 == 1 { 4 } else { 5 };
            for k in 0..n {
                // Two gaps per row; the odd bus stacked on another.
                if (row == 0 && k == 2) || (row == 2 && k == 3) {
                    continue;
                }
                let x = x0 + k as f32 * 23.4;
                let yaw = b.jit(0.02);
                let stacked = (row + k) % 4 == 1;
                b.bus(x, z, yaw, stacked);
            }
        }
        b.place(ObstacleKind::Tent, 214.0, -232.0, 0.0);
        b.place(ObstacleKind::CarWreck, 152.8, -202.0, 0.05);
        b.place(ObstacleKind::Pipe, 128.0, -132.0, 0.1);
        b.place(ObstacleKind::Sandbags, 102.0, -96.0, 0.5);

        // ---- THE MAGNET CRANE: over the pile of wrecks ---------------------------
        let crane_at = PILE + Vec2::new(-34.0, -20.0);
        b.place(ObstacleKind::MagnetCrane, crane_at.x, crane_at.y, face(crane_at, PILE));
        // The crowning wreck (the rest of the heap is junk you drive over).
        for (dx, dz, yaw) in [(0.0, 0.0, 0.4)] {
            let j = b.jit(0.2);
            b.place(ObstacleKind::CarWreck, PILE.x + dx, PILE.y + dz, yaw + j);
        }
        // Cube stacks round the pile's foot, open toward the tracks.
        b.cubes(PILE.x + 44.0, PILE.y + 24.0, 1.0, &[3, 2, 3]);
        b.cubes(PILE.x + 6.0, PILE.y + 48.0, 0.1, &[2, 3, 4, 3]);
        b.cubes(PILE.x - 46.0, PILE.y + 26.0, -0.9, &[3, 3, 2]);
        b.cubes(PILE.x + 46.0, PILE.y - 26.0, -0.7, &[2, 4, 3]);
        b.place(ObstacleKind::Shack, crane_at.x - 6.0, crane_at.y - 14.0, 0.5);
        b.place(ObstacleKind::Sandbags, PILE.x - 30.0, PILE.y + 5.0, 1.2);

        // ---- THE TOWER: the stack of cars in a ring of cubes ---------------------
        b.place(ObstacleKind::CarTower, TOWER.x, TOWER.y, 0.3);
        // The ring: five cube walls round the plaza, gaps between them.
        for k in 0..5 {
            let a = k as f32 / 5.0 * std::f32::consts::TAU + 0.35;
            let p = TOWER + Vec2::new(a.cos(), a.sin()) * 27.0;
            let tangent = a + std::f32::consts::FRAC_PI_2;
            let lv: &[u8] = [&[3, 4, 3, 2, 3][..], &[2, 3, 3, 2, 4][..], &[3, 3, 4, 3, 2][..], &[2, 4, 3, 3, 3][..], &[3, 2, 3, 4, 3][..]][k];
            b.cubes(p.x, p.y, -tangent, lv);
        }
        b.place(ObstacleKind::ContainerStack, TOWER.x + 52.0, TOWER.y - 30.0, 0.4);
        b.place(ObstacleKind::Shack, TOWER.x - 50.0, TOWER.y + 30.0, -0.3);
        b.place(ObstacleKind::CarWreck, TOWER.x - 44.0, TOWER.y - 12.0, 0.8);
        b.place(ObstacleKind::CarWreck, TOWER.x + 8.0, TOWER.y + 46.0, 2.1);
        b.place(ObstacleKind::Pipe, TOWER.x + 40.0, TOWER.y + 42.0, 0.6);

        // ---- THE DRY RIVER: wrecks and culvert pipes in the bed -------------------
        for (x, dz, k, yaw) in [
            (-160.0, 0.0, ObstacleKind::CarWreck, 0.2),
            (55.0, 0.0, ObstacleKind::Pipe, 0.1),
            (150.0, 0.0, ObstacleKind::CarWreck, 2.9),
            (245.0, 0.0, ObstacleKind::CarWreck, 0.4),
        ] {
            let j = b.jit(0.1);
            b.place(k, x, river_z(x) + dz, yaw + j);
        }

        // ---- The canyon: red rock cliffs along both walls (out of bounds) --------
        let mut x = -330.0;
        while x < 340.0 {
            for zs in [-1.0f32, 1.0] {
                let r = &mut b.rng;
                let main = Vec3::new(r.range(13.0, 18.0), r.range(16.0, 24.0), r.range(9.0, 12.0));
                let shoulder = Vec3::new(r.range(6.0, 10.0), r.range(4.0, 8.0), 5.0);
                let zc = zs * r.range(297.0, 301.0);
                let slide = r.range(-6.0, 6.0);
                let boxes = vec![
                    bx(Vec3::new(0.0, main.y - 3.0, 0.0), main, r.range(-0.15, 0.15)),
                    // The shoulder steps down toward the canyon floor.
                    bx(Vec3::new(slide, shoulder.y - 2.0, -zs * (zc.abs() - 289.0)), shoulder, r.range(-0.3, 0.3)),
                ];
                let jx = r.range(-5.0, 5.0);
                let ob = Obstacle { kind: ObstacleKind::Mesa, cover: Cover::Tall, pos: Vec3::ZERO, rot: Quat::IDENTITY, boxes, radius: 22.0 };
                b.push(ob, x + jx, zc, 0.0);
            }
            x += 27.0;
        }
        for (x, z) in [(-160.0, -258.0), (215.0, -262.0), (40.0, 262.0), (-215.0, 256.0), (232.0, 252.0)] {
            let yaw = b.jit(3.0);
            b.place(ObstacleKind::RockFormation, x, z, yaw);
        }

        // ---- Heights --------------------------------------------------------
        let ground0 = move |x: f32, z: f32| corridor(x, z, base(x, z));
        let mut pads: Vec<Pad> = Vec::new();
        pads.push(Pad { center: MARKET, inner: 44.0, outer: 62.0, target: ground0(MARKET.x, MARKET.y) });
        for zb in [deck_a - 4.0, deck_b + 4.0] {
            pads.push(Pad { center: Vec2::new(0.0, zb), inner: 10.0, outer: 22.0, target: bank_top });
        }
        for o in b.obstacles.iter().filter(|o| o.cover != Cover::Flat && o.kind != ObstacleKind::Fence) {
            let c = Vec2::new(o.pos.x, o.pos.z);
            // The pile keeps its slopes; the rim rock sits on the walls as they are.
            if c.distance(PILE) < 44.0 || c.y.abs() > play_half - 20.0 {
                continue;
            }
            pads.push(Pad { center: c, inner: o.radius * 0.8, outer: o.radius + 10.0, target: ground0(c.x, c.y) });
        }
        for s in &spawns {
            let c = Vec2::new(s.pos.x, s.pos.z);
            pads.push(Pad { center: c, inner: 6.0, outer: 18.0, target: ground0(c.x, c.y) });
        }
        let spacing = size / (resolution - 1) as f32;
        let mut heights = vec![0.0; resolution * resolution];
        for iz in 0..resolution {
            for ix in 0..resolution {
                let x = -size / 2.0 + ix as f32 * spacing;
                let z = -size / 2.0 + iz as f32 * spacing;
                let mut h = ground0(x, z);
                for p in &pads {
                    let d = p.center.distance(Vec2::new(x, z));
                    if d < p.outer {
                        let w = 1.0 - smoothstep(p.inner, p.outer, d);
                        h += (p.target - h) * w;
                    }
                }
                // The road wins over the pads, the river over everything.
                h = corridor(x, z, h);
                h = carve(x, z, h);
                // The canyon walls: high and steep north and south, lower
                // where the canyon runs on east and west.
                h += smoothstep(play_half - 12.0, size / 2.0, z.abs()) * 20.0;
                h += smoothstep(play_half - 12.0, size / 2.0, x.abs()) * 12.0;
                heights[iz * resolution + ix] = h;
            }
        }

        let mut map = Map {
            id: MapId::RustBelt,
            name: "The Rust Belt".to_string(),
            seed,
            size,
            resolution,
            heights,
            obstacles: b.obstacles,
            spawns,
            scrap_sites: Vec::new(),
            repair_sites: Vec::new(),
            props: Vec::new(),
            play_half,
            rare_sites: Vec::new(),
            strike_sites: Vec::new(),
            cranes: Vec::new(),
            outpost: Vec3::ZERO,
            regions: Vec::new(),
            convoy_route: Vec::new(),
            wells: Vec::new(),
            marks: Vec::new(),
        };
        let mut rng = b.rng;
        for i in 0..map.obstacles.len() {
            let o = &map.obstacles[i];
            let p = o.pos;
            let y = if p.z.abs() > play_half {
                // The canyon cliffs rise from the foot of the wall.
                map.height_at(p.x, p.z.signum() * (play_half - 13.0)) - 1.0
            } else if o.cover == Cover::Tall {
                // Tall cover rests on the highest ground under it (never sunk).
                o.boxes
                    .iter()
                    .filter(|bx| bx.center.y - bx.half.y < 4.0)
                    .map(|bx| {
                        let c = o.pos + o.rot * bx.center;
                        map.height_at(c.x, c.z)
                    })
                    .fold(map.height_at(p.x, p.z), f32::max)
            } else if o.cover == Cover::Low {
                // Low cover settles to the lowest ground under its corners
                // (on the wreck pile's slopes it stays under 1.2 m all round).
                let mut y = map.height_at(p.x, p.z);
                for bx in &o.boxes {
                    for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                        let c = o.pos + o.rot * (bx.center + bx.rot * Vec3::new(sx * bx.half.x, 0.0, sz * bx.half.z));
                        y = y.min(map.height_at(c.x, c.z));
                    }
                }
                y
            } else {
                map.height_at(p.x, p.z)
            };
            map.obstacles[i].pos.y = y;
        }
        // The bridge: on the river bed, its deck flush with both banks.
        {
            let bed_y = map.height_at(0.0, bridge_z);
            let top = map.height_at(0.0, deck_a + 1.0).max(map.height_at(0.0, deck_b - 1.0));
            let mut boxes = Vec::new();
            bridge_boxes(&mut boxes, top - bed_y, BRIDGE_HALF);
            map.obstacles.push(Obstacle {
                kind: ObstacleKind::Bridge,
                cover: Cover::Tall,
                pos: Vec3::new(0.0, bed_y, bridge_z),
                rot: Quat::IDENTITY,
                boxes,
                radius: BRIDGE_HALF + 1.0,
            });
        }
        for i in 0..map.spawns.len() {
            let p = map.spawns[i].pos;
            map.spawns[i].pos.y = map.height_at(p.x, p.z);
        }
        map.outpost = Vec3::new(MARKET.x, map.height_at(MARKET.x, MARKET.y), MARKET.y);
        let at3 = |m: &Map, p: Vec2| Vec3::new(p.x, m.height_at(p.x, p.y), p.y);
        map.regions = vec![
            Region { name: "The Salvage Market".into(), center: map.outpost, radius: 52.0 },
            Region { name: "The Dry River".into(), center: at3(&map, Vec2::new(0.0, bridge_z)), radius: 100.0 },
            Region { name: "The Crusher".into(), center: at3(&map, CRUSHER), radius: 85.0 },
            Region { name: "The Bus Graveyard".into(), center: at3(&map, BUSES), radius: 85.0 },
            Region { name: "The Magnet Crane".into(), center: at3(&map, PILE), radius: 80.0 },
            Region { name: "The Tower".into(), center: at3(&map, TOWER), radius: 75.0 },
        ];

        // Convoy: down the river bed from the west end, under the bridge (by
        // the north arch), up the east ford and on to the bus graveyard.
        let mut x: f32 = -200.0;
        while x < 200.0 {
            let dz = if x.abs() < 30.0 { -8.0 } else { 0.0 };
            map.convoy_route.push(at3(&map, Vec2::new(x, river_z(x) + dz)));
            x += 30.0;
        }
        for (x, z) in [(205.0, river_z(205.0)), (205.0, -12.0), (196.0, -50.0)] {
            map.convoy_route.push(at3(&map, Vec2::new(x, z)));
        }

        // Pickup sites, each nudged to the nearest free spot.
        let free = |m: &Map, p: Vec2, margin: f32| -> Vec2 {
            for r in 0..14 {
                for k in 0..12 {
                    let a = k as f32 * 0.5236;
                    let q = p + Vec2::new(a.cos(), a.sin()) * r as f32 * 2.5;
                    if !m.blocked_xz(q, margin) {
                        return q;
                    }
                }
            }
            p
        };
        let rz = river_z;
        let scrap: [(f32, f32); 49] = [
            // The market courtyard.
            (-10.0, -36.0), (10.0, -24.0), (0.0, -46.0), (-20.0, -14.0), (20.0, -46.0),
            // The crusher's maze.
            (-190.0, -160.0), (-143.0, -150.0), (-165.0, -186.0), (-178.0, -132.0), (-210.0, -186.0), (-120.0, -186.0), (-210.0, -142.0),
            // Between the bus rows.
            (130.0, -188.0), (176.0, -188.0), (150.0, -160.0), (196.0, -160.0), (120.0, -132.0), (170.0, -132.0), (216.0, -188.0),
            // The wreck pile and round it.
            (-150.0, 182.0), (-130.0, 200.0), (-174.0, 200.0), (-120.0, 160.0), (-200.0, 150.0), (-160.0, 222.0),
            // The tower's plaza and ring.
            (165.0, 164.0), (148.0, 192.0), (184.0, 196.0), (128.0, 150.0), (206.0, 162.0), (165.0, 228.0),
            // The river bed.
            (-90.0, rz(-90.0)), (-55.0, rz(-55.0) + 6.0), (-20.0, rz(-20.0) - 9.0), (25.0, rz(25.0) + 9.0), (62.0, rz(62.0) - 5.0), (95.0, rz(95.0)),
            // The roads and the open ground.
            (0.0, -118.0), (0.0, -205.0), (-80.0, -62.0), (82.0, -62.0), (0.0, 140.0), (0.0, 232.0),
            (-70.0, 125.0), (75.0, 128.0), (-250.0, 0.0), (250.0, 10.0), (-110.0, 250.0), (40.0, -238.0),
        ];
        for (i, (x, z)) in scrap.iter().enumerate() {
            let p = free(&map, Vec2::new(*x + rng.range(-2.0, 2.0), *z + rng.range(-2.0, 2.0)), 3.0);
            map.scrap_sites.push(ScrapSite { pos: at3(&map, p), kind: ScrapKind::from_index(i) });
        }
        for (x, z) in [(0.0, -40.0), (-150.0, -128.0), (150.0, -160.0), (-118.0, 170.0), (140.0, 164.0), (100.0, rz(100.0))] {
            let p = free(&map, Vec2::new(x, z), 4.0);
            map.repair_sites.push(at3(&map, p));
        }
        for (x, z) in [(PILE.x + 3.0, PILE.y + 2.0), (TOWER.x, TOWER.y + 16.0)] {
            let p = free(&map, Vec2::new(x, z), 4.0);
            map.rare_sites.push(at3(&map, p));
        }
        let p = free(&map, Vec2::new(-12.0, bridge_z + 2.0), 4.0);
        map.strike_sites.push(at3(&map, p));
        // Wellhead pumps: the crusher's yard (west), the river bed (the
        // middle, under the market's south gate) and the bus lot (east).
        for (x, z, name) in [(-150.0, -88.0, "the crusher"), (18.0, rz(18.0) + 4.0, "the river bed"), (150.0, -92.0, "the bus graveyard")] {
            let p = free(&map, Vec2::new(x, z), 5.0);
            map.wells.push(WellSite { pos: at3(&map, p), name: name.into() });
        }
        // Oil drums: the market, the crusher, the buses, the tower.
        for (c, n) in [(Vec2::new(-14.0, -18.0), 3), (CRUSHER + Vec2::new(-12.0, 12.0), 4), (Vec2::new(186.0, -132.0), 3), (TOWER + Vec2::new(10.0, 16.0), 3)] {
            for k in 0..n {
                let p = free(&map, c + Vec2::new(k as f32 * 1.3, (k % 2) as f32 * 1.2), 0.8);
                map.props.push(PropSpawn { kind: PropKindSpawn::OilDrum, pos: at3(&map, p) + Vec3::Y * 0.5 });
            }
        }
        map.marks = rust_marks(&map);
        map
    }
}
