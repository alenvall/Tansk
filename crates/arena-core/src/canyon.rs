//! THE GORGE, laid out by hand: a deep, winding desert canyon at three
//! heights.
//!
//! ```text
//!                N (-z): the canyon's north wall
//!     RED MESA (NW, +26)                       EAGLE ROCK (NE, +26)
//!          \ ramp                                   ramp /
//!     THE NORTH RIM (+12): the miners' camp, ramps down into the gorge
//!   ~~~~~~~~~~ THE GORGE (the dry riverbed, 0 m) winds west to east ~~~~~~~~~~
//!           girder bridge ==== THE BUTTE (+12, the outpost) ==== girder bridge
//!     THE SOUTH RIM (+12): the old stamp mill, ramps down into the gorge
//!     TABLE TOP (SW, +26)                         THE ANVIL (SE, +26)
//!                S (+z): the canyon's south wall
//! ```
//!
//! Levels: the gorge floor (0 m), the rims (12 m), the mesas (26 m), joined by
//! wide ramps (dog-legged where they climb a mesa), girder bridges from the
//! rims to the butte, and spring pads (gorge to rim, rim to mesa). The butte
//! in the gorge's bend is the outpost: strong high ground, reached up a ramp
//! from each end of the gorge and over a bridge from each rim. Rock arches
//! and boulder piles on the floor, the cliffs themselves as cover. Same
//! rules as the other maps: low cover under 1.2 m, tall from 2.5 m, lanes
//! of 8 m or nothing, eight spawns round the edge. The seed only jitters
//! details.

use glam::{Quat, Vec2, Vec3};

use crate::map::*;
use crate::rng::{value_noise, Rng};

struct Builder {
    obstacles: Vec<Obstacle>,
    rng: Rng,
}

impl Builder {
    fn place(&mut self, kind: ObstacleKind, x: f32, z: f32, yaw: f32) -> usize {
        let mut ob = make_obstacle(kind, &mut self.rng);
        ob.pos = Vec3::new(x, 0.0, z);
        ob.rot = Quat::from_rotation_y(yaw);
        self.obstacles.push(ob);
        self.obstacles.len() - 1
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

// ---- The heights ---------------------------------------------------------------

/// The rims either side of the gorge.
pub const RIM: f32 = 12.0;
/// The mesa tops.
pub const MESA: f32 = 26.0;

/// The gorge's centre line: it swings south in the west, north in the east.
pub fn gorge_z(x: f32) -> f32 {
    40.0 * (x / 95.0).sin()
}

/// Half width of the gorge floor: wider round the butte in the middle.
pub fn gorge_half(x: f32) -> f32 {
    48.0 + 24.0 * (1.0 - smoothstep(45.0, 115.0, x.abs()))
}

/// The butte (the outpost) in the gorge: flat top radius, cliff foot radius.
pub const BUTTE: Vec2 = Vec2::ZERO;
const BUTTE_TOP: f32 = 30.0;
const BUTTE_FOOT: f32 = 37.0;
/// The cliff between the floor and the rims, m.
const CLIFF: f32 = 7.0;

/// The four mesas: centre, half extents (a squarish rounded table).
const MESAS: [(Vec2, Vec2, &str); 4] = [
    (Vec2::new(-150.0, -205.0), Vec2::new(58.0, 38.0), "Red Mesa"),
    (Vec2::new(155.0, -205.0), Vec2::new(55.0, 38.0), "Eagle Rock"),
    (Vec2::new(-160.0, 200.0), Vec2::new(55.0, 38.0), "Table Top"),
    (Vec2::new(155.0, 205.0), Vec2::new(58.0, 38.0), "The Anvil"),
];

/// A ramp: a polyline of (point, height) the ground is cut or filled to,
/// `RAMP_HALF` m either side.
struct Ramp(&'static [(f32, f32, f32)]);
const RAMP_HALF: f32 = 13.0;

/// Gorge to rim, the butte's two ramps, rim to mesa (dog-legged).
const RAMPS: [Ramp; 10] = [
    // The butte: up from the gorge floor at each end.
    Ramp(&[(84.0, 6.0, 0.0), (26.0, 0.0, RIM)]),
    Ramp(&[(-84.0, -6.0, 0.0), (-26.0, 0.0, RIM)]),
    // Gorge floor up to the north rim (west, east) and the south rim (west, east).
    Ramp(&[(-112.0, -40.0, 0.0), (-112.0, -112.0, RIM)]),
    Ramp(&[(132.0, 10.0, 0.0), (132.0, -62.0, RIM)]),
    Ramp(&[(-132.0, -8.0, 0.0), (-132.0, 64.0, RIM)]),
    Ramp(&[(112.0, 42.0, 0.0), (112.0, 112.0, RIM)]),
    // Rim up to each mesa: along its face, then a dog-leg onto the top.
    Ramp(&[(-40.0, -178.0, RIM), (-74.0, -178.0, RIM + 6.0), (-100.0, -196.0, MESA)]),
    Ramp(&[(48.0, -178.0, RIM), (82.0, -178.0, RIM + 6.0), (108.0, -196.0, MESA)]),
    Ramp(&[(-52.0, 176.0, RIM), (-86.0, 176.0, RIM + 6.0), (-112.0, 192.0, MESA)]),
    Ramp(&[(46.0, 178.0, RIM), (80.0, 178.0, RIM + 6.0), (106.0, 196.0, MESA)]),
];

/// The terrain before the ramps: floor, rims, butte, mesas.
fn levels(x: f32, z: f32) -> f32 {
    let off = (z - gorge_z(x)).abs();
    let hw = gorge_half(x);
    let rim = RIM * smoothstep(hw, hw + CLIFF, off);
    let butte = RIM * (1.0 - smoothstep(BUTTE_TOP, BUTTE_FOOT, Vec2::new(x, z).distance(BUTTE)));
    let mut h = rim.max(butte);
    for (c, half, _) in MESAS {
        let d = Vec2::new((x - c.x) / half.x, (z - c.y) / half.y);
        let s = (d.x.powi(4) + d.y.powi(4)).powf(0.25);
        let edge = CLIFF / half.x.min(half.y);
        if s < 1.0 + edge {
            // The mesa rises off the rim, not the floor.
            h = h.max(RIM + (MESA - RIM) * (1.0 - smoothstep(1.0, 1.0 + edge, s)));
        }
    }
    h
}

/// The ramps cut and fill the ground to their slope.
fn with_ramps(x: f32, z: f32, mut h: f32) -> f32 {
    let p = Vec2::new(x, z);
    for Ramp(pts) in RAMPS.iter() {
        let mut best = (f32::MAX, 0.0);
        for w in pts.windows(2) {
            let (a, b) = (Vec2::new(w[0].0, w[0].1), Vec2::new(w[1].0, w[1].1));
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
            let d = (a + ab * t).distance(p);
            if d < best.0 {
                best = (d, w[0].2 + (w[1].2 - w[0].2) * t);
            }
        }
        let wgt = 1.0 - smoothstep(RAMP_HALF, RAMP_HALF + 3.5, best.0);
        if wgt > 0.0 {
            h += (best.1 - h) * wgt;
        }
    }
    h
}

/// The ground storytelling: tracks along the gorge and up every ramp,
/// packed ground on the butte and the camps, oil by the trailers.
fn gorge_marks(map: &Map) -> Vec<GroundMark> {
    let mut marks = Vec::new();
    let floor: Vec<Vec2> = (0..=18).map(|k| {
        let x = -275.0 + k as f32 * 30.0;
        // Round the butte by its south channel.
        let z = if x.abs() < 70.0 { gorge_z(x) + 54.0 * (1.0 - (x / 70.0).powi(2)).sqrt() } else { gorge_z(x) };
        Vec2::new(x, z)
    }).collect();
    marks.push(GroundMark { kind: GroundMarkKind::Track, points: floor, width: 5.0 });
    for Ramp(pts) in RAMPS.iter() {
        marks.push(GroundMark { kind: GroundMarkKind::Track, points: pts.iter().map(|p| Vec2::new(p.0, p.1)).collect(), width: 5.0 });
    }
    let packed = |a: (f32, f32), b: (f32, f32), w: f32| GroundMark { kind: GroundMarkKind::Packed, points: vec![Vec2::new(a.0, a.1), Vec2::new(b.0, b.1)], width: w };
    marks.push(packed((-14.0, 0.0), (14.0, 0.0), 44.0));
    marks.push(packed((-20.0, -150.0), (20.0, -150.0), 40.0));
    marks.push(packed((-20.0, 152.0), (20.0, 152.0), 40.0));
    let oil: Vec<Vec2> = map.obstacles.iter().filter(|o| matches!(o.kind, ObstacleKind::FuelTrailer | ObstacleKind::CarWreck | ObstacleKind::FuelTank)).map(|o| Vec2::new(o.pos.x, o.pos.z)).collect();
    marks.push(GroundMark { kind: GroundMarkKind::Oil, points: oil, width: 6.0 });
    marks
}

impl Map {
    /// "The Gorge": see the module docs for the layout.
    pub fn gorge(seed: u64) -> Map {
        let size = 600.0;
        let resolution = 151;
        let play_half = 285.0;
        let mut b = Builder { obstacles: Vec::new(), rng: Rng::new(seed ^ 0x6077_6E15) };
        let s1 = seed.wrapping_mul(53).wrapping_add(5);

        // ---- Spawns: round the edge, each with a little cover ahead ----------
        let spawn_xz = [
            Vec2::new(-40.0, -262.0),
            Vec2::new(60.0, -264.0),
            Vec2::new(262.0, -112.0),
            Vec2::new(262.0, 26.0),
            Vec2::new(40.0, 264.0),
            Vec2::new(-104.0, 264.0),
            Vec2::new(-262.0, 140.0),
            Vec2::new(-262.0, -4.0),
        ];
        let mut spawns = Vec::new();
        for (i, s) in spawn_xz.into_iter().enumerate() {
            let p = s + Vec2::new(b.jit(2.0), b.jit(2.0));
            spawns.push(Spawn { pos: Vec3::new(p.x, 0.0, p.y), yaw: face(p, Vec2::ZERO) });
            let toward = (-p).normalize();
            let side = Vec2::new(-toward.y, toward.x);
            let c = p + toward * 12.0 + side * 15.0;
            let k = if i % 2 == 0 { ObstacleKind::Sandbags } else { ObstacleKind::RockFormation };
            let yaw = face(p, Vec2::ZERO) + std::f32::consts::FRAC_PI_2;
            if k == ObstacleKind::RockFormation {
                // A lone boulder pile, set off to the side.
                let c = p + toward * 10.0 + side * 24.0;
                b.place(k, c.x, c.y, yaw);
            } else {
                b.place(k, c.x, c.y, yaw);
            }
        }

        // ---- THE BUTTE: the outpost on its table in the gorge -----------------
        // Sandbag lines round the rim on the diagonals (gaps at the two ramps
        // and the two bridges); the courtyard stays open.
        for k in 0..4 {
            let a = (k as f32 + 0.5) * std::f32::consts::FRAC_PI_2;
            let p = BUTTE + Vec2::new(a.cos(), a.sin()) * 25.0;
            b.place(ObstacleKind::Sandbags, p.x, p.y, face(p, BUTTE) + std::f32::consts::FRAC_PI_2);
        }

        // ---- THE GORGE floor: arches, boulder piles, a wrecked mine train -------
        for x in [-170.0f32, 195.0, 160.0] {
            // An arch straddling the floor: its legs either side of the
            // middle, a 13 m lane under the span, the walls well clear.
            let z = gorge_z(x);
            let slope = 40.0 / 95.0 * (x / 95.0).cos();
            let flow = (-slope).atan2(1.0);
            let j = b.jit(0.05);
            b.place(ObstacleKind::RockArch, x, z, flow + std::f32::consts::FRAC_PI_2 + j);
        }
        for (x, dz) in [(-140.0f32, 14.0f32), (-245.0, -22.0), (60.0, -40.0), (230.0, 25.0)] {
            let yaw = b.jit(3.0);
            b.place(ObstacleKind::RockFormation, x, gorge_z(x) + dz, yaw);
        }
        for (x, dz, k, yaw) in [
            (-95.0f32, 20.0f32, ObstacleKind::CarWreck, 0.4f32),
            (-30.0, 50.0, ObstacleKind::Crates, 0.0),
            (95.0, -30.0, ObstacleKind::CarWreck, 2.2),
            (200.0, 30.0, ObstacleKind::Pipe, 0.2),
            (-245.0, 20.0, ObstacleKind::Pipe, 1.4),
        ] {
            b.place(k, x, gorge_z(x) + dz, yaw);
        }

        // ---- THE NORTH RIM: the miners' camp ------------------------------------
        b.place(ObstacleKind::Tent, -20.0, -150.0, 0.0);
        b.place(ObstacleKind::Tent, 22.0, -150.0, 0.0);
        b.place(ObstacleKind::WaterTower, 0.0, -172.0, 0.0);
        b.place(ObstacleKind::Windmill, 34.0, -128.0, 0.4);
        b.place(ObstacleKind::Crates, -2.0, -132.0, 0.0);
        b.place(ObstacleKind::RadioMast, 40.0, -168.0, 0.3);
        b.place(ObstacleKind::Sandbags, -50.0, -120.0, 0.6);
        b.place(ObstacleKind::RockFormation, -230.0, -110.0, 1.0);
        b.place(ObstacleKind::RockFormation, 230.0, -165.0, 2.0);

        // ---- THE SOUTH RIM: the old stamp mill ---------------------------------
        b.place(ObstacleKind::Silo, -18.0, 152.0, 0.0);
        b.place(ObstacleKind::ContainerStack, 22.0, 150.0, 1.57);
        b.place(ObstacleKind::PowerPole, 0.0, 175.0, 0.0);
        b.place(ObstacleKind::Tower, -45.0, 150.0, 0.2);
        b.place(ObstacleKind::Sandbags, 52.0, 122.0, 2.4);
        b.place(ObstacleKind::RockFormation, 235.0, 115.0, 0.5);
        b.place(ObstacleKind::RockFormation, -230.0, 70.0, 1.7);

        // ---- The mesas: a sandbag nest and rock on each top --------------------
        for (c, _, _) in MESAS {
            let j = b.jit(0.4);
            b.place(ObstacleKind::Sandbags, c.x + 25.0, c.y + c.y.signum() * 20.0, j);
            b.place(ObstacleKind::RockFormation, c.x - 30.0, c.y - c.y.signum() * 15.0, j * 3.0);
        }

        // ---- The canyon walls: red rock beyond the play area --------------------
        let mut x = -330.0;
        while x < 340.0 {
            for zs in [-1.0f32, 1.0] {
                let r = &mut b.rng;
                let main = Vec3::new(r.range(13.0, 18.0), r.range(20.0, 28.0), r.range(9.0, 12.0));
                let zc = zs * r.range(297.0, 301.0);
                let boxes = vec![bx(Vec3::new(0.0, main.y - 3.0, 0.0), main, r.range(-0.15, 0.15))];
                let jx = r.range(-5.0, 5.0);
                b.obstacles.push(Obstacle { kind: ObstacleKind::Mesa, cover: Cover::Tall, pos: Vec3::new(x + jx, 0.0, zc), rot: Quat::IDENTITY, boxes, radius: 22.0 });
            }
            x += 27.0;
        }

        // ---- Heights --------------------------------------------------------
        let spacing = size / (resolution - 1) as f32;
        let mut heights = vec![0.0; resolution * resolution];
        for iz in 0..resolution {
            for ix in 0..resolution {
                let x = -size / 2.0 + ix as f32 * spacing;
                let z = -size / 2.0 + iz as f32 * spacing;
                let mut h = with_ramps(x, z, levels(x, z));
                // A little life in the flats (not enough to tip a ramp).
                h += (value_noise(s1, x / 60.0, z / 60.0) - 0.5) * 0.5;
                // The canyon walls rise beyond the play area: high north and
                // south, lower where the gorge runs on east and west.
                h += rim(play_half, size, z.abs(), 30.0);
                h += rim(play_half, size, x.abs(), 14.0);
                heights[iz * resolution + ix] = h;
            }
        }

        let mut map = Map {
            id: MapId::Canyon,
            name: "The Gorge".to_string(),
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
            springs: Vec::new(),
        };
        let mut rng = b.rng;
        for i in 0..map.obstacles.len() {
            let o = &map.obstacles[i];
            let p = o.pos;
            let y = if p.z.abs() > play_half {
                map.height_at(p.x, p.z.signum() * (play_half - 13.0)) - 1.0
            } else if o.cover == Cover::Tall {
                o.boxes
                    .iter()
                    .filter(|bx| bx.center.y - bx.half.y < 4.0)
                    .map(|bx| {
                        let c = o.pos + o.rot * bx.center;
                        map.height_at(c.x, c.z)
                    })
                    .fold(map.height_at(p.x, p.z), f32::max)
            } else if o.cover == Cover::Low {
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
        // The girder bridges, rim to butte across each channel: on the floor
        // under their middles, decks flush with both ends.
        for zs in [-1.0f32, 1.0] {
            let rim_edge = zs * (gorge_half(0.0) + CLIFF + 3.0);
            let butte_edge = zs * (BUTTE_TOP - 3.0);
            let mid = (rim_edge + butte_edge) * 0.5;
            let half_len = (rim_edge - butte_edge).abs() * 0.5;
            let floor_y = map.height_at(0.0, mid);
            let top = map.height_at(0.0, rim_edge).max(map.height_at(0.0, butte_edge));
            let mut boxes = Vec::new();
            girder_bridge_boxes(&mut boxes, top - floor_y, half_len);
            map.obstacles.push(Obstacle { kind: ObstacleKind::GirderBridge, cover: Cover::Tall, pos: Vec3::new(0.0, floor_y, mid), rot: Quat::IDENTITY, boxes, radius: half_len + 1.0 });
        }
        // Spectacle: springs from the rims up onto Red Mesa, Table Top and the
        // Anvil (the gorge floor stays the convoy's); fuel trailers at the
        // camps and in the gorge.
        map.add_spring(Vec2::new(-40.0, -138.0), Vec2::new(-140.0, -205.0));
        map.add_spring(Vec2::new(-70.0, 150.0), Vec2::new(-150.0, 200.0));
        map.add_spring(Vec2::new(205.0, 140.0), Vec2::new(165.0, 190.0));
        for (x, z, yaw) in [(-60.0, -150.0, 0.0), (70.0, 150.0, std::f32::consts::FRAC_PI_2), (-220.0, gorge_z(-220.0) - 25.0, 0.2)] {
            map.add_trailer(Vec2::new(x, z), yaw);
        }
        for i in 0..map.spawns.len() {
            let p = map.spawns[i].pos;
            map.spawns[i].pos.y = map.height_at(p.x, p.z);
        }
        map.outpost = Vec3::new(BUTTE.x, map.height_at(BUTTE.x, BUTTE.y), BUTTE.y);
        map.aim_spawns();
        let at3 = |m: &Map, p: Vec2| Vec3::new(p.x, m.height_at(p.x, p.y), p.y);
        let gz = gorge_z;
        map.regions = vec![
            Region { name: "The Butte".into(), center: map.outpost, radius: 45.0 },
            Region { name: "The West Gorge".into(), center: at3(&map, Vec2::new(-180.0, gz(-180.0))), radius: 80.0 },
            Region { name: "The East Gorge".into(), center: at3(&map, Vec2::new(180.0, gz(180.0))), radius: 80.0 },
            Region { name: "The North Rim".into(), center: at3(&map, Vec2::new(0.0, -150.0)), radius: 60.0 },
            Region { name: "The South Rim".into(), center: at3(&map, Vec2::new(0.0, 152.0)), radius: 60.0 },
        ];
        for (c, _, name) in MESAS {
            map.regions.push(Region { name: name.into(), center: at3(&map, c), radius: 50.0 });
        }

        // Convoy: down the gorge from the west end, round the butte by its
        // south channel, to the unloading point past it in the east gorge.
        let mut x: f32 = -200.0;
        while x <= 70.0 {
            let z = if x.abs() < 60.0 { 52.0 } else { gz(x) };
            map.convoy_route.push(at3(&map, Vec2::new(x, z)));
            x += 30.0;
        }
        map.convoy_route.push(at3(&map, Vec2::new(85.0, gz(85.0))));

        // Pickup sites, each nudged to the nearest free spot.
        let free = |m: &Map, p: Vec2, margin: f32| -> Vec2 {
            for r in 0..14 {
                for k in 0..12 {
                    let a = k as f32 * 0.5236;
                    let q = p + Vec2::new(a.cos(), a.sin()) * r as f32 * 2.5;
                    if !m.blocked_xz(q, margin) && m.normal_at(q.x, q.y).y > 0.9 {
                        return q;
                    }
                }
            }
            p
        };
        let mut scrap: Vec<(f32, f32)> = vec![
            // The butte.
            (-8.0, -6.0), (8.0, 8.0), (0.0, -14.0), (12.0, -4.0),
            // The north and south rims.
            (-20.0, -128.0), (20.0, -128.0), (0.0, -195.0), (-60.0, -135.0), (60.0, -135.0),
            (-20.0, 130.0), (20.0, 130.0), (0.0, 195.0), (-60.0, 135.0), (60.0, 135.0),
            // The rims' far ends.
            (-230.0, -150.0), (230.0, -120.0), (-230.0, 120.0), (230.0, 150.0),
        ];
        for x in [-250.0f32, -215.0, -180.0, -145.0, -110.0, 110.0, 145.0, 180.0, 215.0, 250.0] {
            // Along the gorge floor, both ends.
            scrap.push((x, gz(x) + if (x as i32 / 5) % 2 == 0 { 14.0 } else { -14.0 }));
        }
        // The channels round the butte.
        scrap.extend([(-50.0, -55.0), (50.0, -50.0), (-50.0, 55.0), (50.0, 55.0)]);
        // The mesa tops.
        for (c, _, _) in MESAS {
            scrap.extend([(c.x, c.y), (c.x + 25.0, c.y + 12.0), (c.x - 22.0, c.y - 12.0)]);
        }
        for (i, (x, z)) in scrap.iter().enumerate() {
            let p = free(&map, Vec2::new(*x + rng.range(-2.0, 2.0), *z + rng.range(-2.0, 2.0)), 3.0);
            map.scrap_sites.push(ScrapSite { pos: at3(&map, p), kind: ScrapKind::from_index(i) });
        }
        for (x, z) in [(0.0, 10.0), (-180.0, gz(-180.0)), (180.0, gz(180.0)), (0.0, -140.0), (0.0, 140.0)] {
            let p = free(&map, Vec2::new(x, z), 4.0);
            map.repair_sites.push(at3(&map, p));
        }
        for (x, z) in [(MESAS[0].0.x + 10.0, MESAS[0].0.y), (MESAS[3].0.x - 10.0, MESAS[3].0.y)] {
            let p = free(&map, Vec2::new(x, z), 4.0);
            map.rare_sites.push(at3(&map, p));
        }
        let p = free(&map, Vec2::new(MESAS[1].0.x, MESAS[1].0.y + 8.0), 4.0);
        map.strike_sites.push(at3(&map, p));
        // Wellhead: the west gorge, the butte (the middle), the east gorge.
        for (x, z, name) in [(-185.0, gz(-185.0), "the west gorge"), (0.0, 0.0, "the butte"), (185.0, gz(185.0), "the east gorge")] {
            let p = free(&map, Vec2::new(x, z), 5.0);
            map.wells.push(WellSite { pos: at3(&map, p), name: name.into() });
        }
        for (c, n) in [(Vec2::new(-10.0, -165.0), 3), (Vec2::new(10.0, 165.0), 3), (Vec2::new(-12.0, 12.0), 2)] {
            for k in 0..n {
                let p = free(&map, c + Vec2::new(k as f32 * 1.3, (k % 2) as f32 * 1.2), 0.8);
                map.props.push(PropSpawn { kind: PropKindSpawn::OilDrum, pos: at3(&map, p) + Vec3::Y * 0.5 });
            }
        }
        map.marks = gorge_marks(&map);
        map
    }
}
