//! THE REFINERY, laid out by hand: a sun-baked oil refinery on a salt flat.
//!
//! ```text
//!          N (-z)
//!   THE FLARE STACK (NW, on a mound)        THE CRACKING TOWERS (NE: columns, pipe racks)
//!   ================= service road (z = -60), power poles =================
//!   THE DERRICK FIELD (W: pumpjacks,   THE CONTROL HOUSE      THE TANK FARM (E: storage
//!     derricks, low berms)               (walled, centre)       tanks in rows: chain reactions)
//!   ======== THE LOADING YARD (S: two rail lines, tank cars, containers, a crane) ========
//!          S (+z)
//! ```
//!
//! Same rules as the Sink: low cover under 1.2 m, tall cover 2.5 m and up
//! (the pipe racks are tall *overhead*: tanks drive under them), lanes at
//! least 8 m wide, eight spawns round the edge, every region worth a visit.
//! The seed only jitters details.

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

    /// A straight concrete wall from a to b.
    fn wall(&mut self, a: Vec2, b: Vec2) {
        let d = b - a;
        let l = d.length();
        let c = (a + b) * 0.5;
        let ob = Obstacle {
            kind: ObstacleKind::Wall,
            cover: Cover::Tall,
            pos: Vec3::ZERO,
            rot: Quat::IDENTITY,
            boxes: vec![bx(Vec3::new(0.0, 1.35, 0.0), Vec3::new(l / 2.0 + 0.35, 1.55, 0.35), 0.0)],
            radius: l / 2.0 + 1.0,
        };
        self.push(ob, c.x, c.y, face(a, b));
    }

    /// Containers side by side (along local Z), each column `levels` high.
    fn containers(&mut self, x: f32, z: f32, yaw: f32, levels: &[u8]) {
        let half = Vec3::new(6.1, 1.3, 1.22);
        let cols = levels.len();
        let mut boxes = Vec::new();
        for (c, &n) in levels.iter().enumerate() {
            for k in 0..n {
                let jitter = self.rng.range(-0.04, 0.04);
                let slide = self.rng.range(-0.3, 0.3);
                boxes.push(bx(
                    Vec3::new(slide, half.y + 2.6 * k as f32, (c as f32 - (cols - 1) as f32 / 2.0) * 2.5),
                    half,
                    jitter,
                ));
            }
        }
        let ob = Obstacle { kind: ObstacleKind::ContainerStack, cover: Cover::Tall, pos: Vec3::ZERO, rot: Quat::IDENTITY, boxes, radius: 6.6 + cols as f32 * 1.3 };
        self.push(ob, x, z, yaw);
    }

    /// A pipe rack from a to b (overhead bundle, legs every ~14 m).
    fn rack(&mut self, a: Vec2, b: Vec2) {
        let len = (b - a).length();
        let mut boxes = Vec::new();
        pipe_rack_boxes(&mut boxes, len);
        let c = (a + b) * 0.5;
        let ob = Obstacle { kind: ObstacleKind::PipeRack, cover: Cover::Tall, pos: Vec3::ZERO, rot: Quat::IDENTITY, boxes, radius: len / 2.0 + 6.0 };
        self.push(ob, c.x, c.y, face(a, b));
    }

    /// A flush strip (road deck or rails) from a to b, in `seg` m pieces.
    fn strip(&mut self, kind: ObstacleKind, a: Vec2, b: Vec2, seg: f32, half_width: f32, skip: &[(f32, f32)]) {
        let d = b - a;
        let len = d.length();
        let dir = d / len;
        let n = (len / seg).floor() as i32;
        for k in 0..n {
            let t = (k as f32 + 0.5) * seg;
            if skip.iter().any(|&(s0, s1)| t > s0 && t < s1) {
                continue;
            }
            let c = a + dir * t;
            let ob = Obstacle {
                kind,
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

/// Distance from `p` to the segment a-b.
fn seg_dist(a: Vec2, b: Vec2, p: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    (a + ab * t).distance(p)
}

// The service road (E-W) and the two rail lines (E-W).
const ROAD_Z: f32 = -60.0;
const ROAD_HALF: f32 = 6.5;
const RAIL_Z: [f32; 2] = [198.0, 214.0];
const STRIP_X: f32 = 262.0;
// Places.
const FLARE: Vec2 = Vec2::new(-185.0, -185.0);
const FLARE_TOP: f32 = 24.0;
const FLARE_FOOT: f32 = 52.0;
const FLARE_HEIGHT: f32 = 4.5;
const TOWERS: Vec2 = Vec2::new(155.0, -175.0);
const FARM: Vec2 = Vec2::new(177.0, 40.0);
const DERRICKS: Vec2 = Vec2::new(-170.0, 50.0);
const YARD: Vec2 = Vec2::new(0.0, 215.0);
const CRANE: Vec2 = Vec2::new(60.0, 238.0);

/// Low berms in the derrick field and round the tank farm (a, b, height).
const BERMS: [(Vec2, Vec2, f32); 9] = [
    (Vec2::new(-235.0, -10.0), Vec2::new(-190.0, -20.0), 1.0),
    (Vec2::new(-150.0, 0.0), Vec2::new(-110.0, 15.0), 1.0),
    (Vec2::new(-240.0, 70.0), Vec2::new(-205.0, 95.0), 1.1),
    (Vec2::new(-160.0, 95.0), Vec2::new(-120.0, 85.0), 1.0),
    (Vec2::new(-215.0, 140.0), Vec2::new(-170.0, 150.0), 1.0),
    (Vec2::new(-110.0, 130.0), Vec2::new(-100.0, 165.0), 0.9),
    // Tank farm bunds, with gaps for the lanes.
    (Vec2::new(112.0, -30.0), Vec2::new(112.0, 30.0), 1.0),
    (Vec2::new(112.0, 60.0), Vec2::new(112.0, 115.0), 1.0),
    (Vec2::new(130.0, 118.0), Vec2::new(200.0, 118.0), 1.0),
];

impl Map {
    /// "The Refinery": storage tanks, cracking towers and derricks on a
    /// salt flat. See the module docs for the layout.
    pub fn refinery(seed: u64) -> Map {
        let size = 600.0;
        let resolution = 151;
        let play_half = 285.0;
        let mut b = Builder { obstacles: Vec::new(), rng: Rng::new(seed ^ 0x2EF1_4E21) };
        let s1 = seed.wrapping_mul(37).wrapping_add(3);
        let s2 = seed.wrapping_mul(173).wrapping_add(11);

        // ---- Terrain: a salt flat, the flare mound, low berms --------------
        let base = move |x: f32, z: f32| -> f32 {
            let p = Vec2::new(x, z);
            let broad = (value_noise(s1, x / 140.0, z / 140.0) - 0.5) * 2.2;
            let small = (value_noise(s2, x / 26.0, z / 26.0) - 0.5) * 0.35;
            let d = p.distance(FLARE);
            let mound = FLARE_HEIGHT * (1.0 - smoothstep(FLARE_TOP, FLARE_FOOT, d));
            let mut berm: f32 = 0.0;
            for (a, b2, h) in BERMS {
                let d = seg_dist(a, b2, p);
                berm = berm.max(h * (1.0 - smoothstep(1.0, 4.5, d)));
            }
            broad + small + mound + berm
        };
        // Flat corridors under the road and the rails.
        let along = move |x: f32, z0: f32| -> f32 {
            let mut s = 0.0;
            for k in -3..=3 {
                s += base(x + k as f32 * 10.0, z0);
            }
            s / 7.0
        };
        let corridor = move |x: f32, z: f32, h: f32| -> f32 {
            if x.abs() > STRIP_X + 14.0 {
                return h;
            }
            let road = 1.0 - smoothstep(ROAD_HALF + 1.0, ROAD_HALF + 12.0, (z - ROAD_Z).abs());
            let rail_mid = 0.5 * (RAIL_Z[0] + RAIL_Z[1]);
            let rails = 1.0 - smoothstep(12.0, 22.0, (z - rail_mid).abs());
            let mut h = h;
            if road > 0.0 {
                h += (along(x, ROAD_Z) - h) * road;
            }
            if rails > 0.0 {
                h += (along(x, rail_mid) - h) * rails;
            }
            h
        };

        // ---- Spawns: round the edge, each with a sandbag line ahead --------
        let spawn_xz = [
            Vec2::new(20.0, -242.0),
            Vec2::new(250.0, -84.0),
            Vec2::new(252.0, 132.0),
            Vec2::new(165.0, 240.0),
            Vec2::new(-100.0, 250.0),
            Vec2::new(-250.0, 175.0),
            Vec2::new(-252.0, -32.0),
            Vec2::new(-95.0, -252.0),
        ];
        let mut spawns = Vec::new();
        for s in spawn_xz {
            let p = s + Vec2::new(b.jit(3.0), b.jit(3.0));
            spawns.push(Spawn { pos: Vec3::new(p.x, 0.0, p.y), yaw: face(p, Vec2::ZERO) });
            let toward = (-p).normalize();
            let side = Vec2::new(-toward.y, toward.x);
            let c = p + toward * 11.0 + side * 15.0;
            b.place(ObstacleKind::Sandbags, c.x, c.y, face(p, Vec2::ZERO) + std::f32::consts::FRAC_PI_2);
        }

        // ---- THE CONTROL HOUSE: the walled compound (the outpost) ----------
        let h = 34.0;
        for side in 0..4 {
            for (a, c) in [(-h, -23.0), (-11.0, 11.0), (23.0, h)] {
                let (p, q) = match side {
                    0 => (Vec2::new(a, -h), Vec2::new(c, -h)),
                    1 => (Vec2::new(a, h), Vec2::new(c, h)),
                    2 => (Vec2::new(-h, a), Vec2::new(-h, c)),
                    _ => (Vec2::new(h, a), Vec2::new(h, c)),
                };
                b.wall(p, q);
            }
        }
        b.place(ObstacleKind::ControlHouse, -2.0, -24.0, 0.0);
        b.place(ObstacleKind::FuelTank, 24.0, -26.0, 0.0);
        b.containers(-26.0, 4.0, std::f32::consts::FRAC_PI_2, &[2]);
        b.containers(26.0, 10.0, std::f32::consts::FRAC_PI_2, &[1]);
        b.place(ObstacleKind::Tower, 28.0, 28.0, 0.2);
        b.place(ObstacleKind::Crates, -27.0, 28.0, 0.0);
        b.place(ObstacleKind::Crates, 10.0, 27.0, 0.0);
        b.place(ObstacleKind::Pipe, -10.0, 26.0, 0.0);
        for (x, z, yaw) in [(-17.0, -48.0, 0.0), (17.0, 48.0, 0.0), (-48.0, 17.0, 1.57), (48.0, -17.0, 1.57)] {
            let j = b.jit(3.0);
            b.place(ObstacleKind::Barrier, x + j, z, yaw);
        }
        for (x, z) in [(-44.0, -44.0), (44.0, 44.0), (-44.0, 44.0), (44.0, -44.0)] {
            let (jx, jz) = (b.jit(2.0), b.jit(2.0));
            b.place(ObstacleKind::Hedgehog, x + jx, z + jz, 0.0);
        }

        // ---- The service road and its power poles --------------------------
        let road_a = Vec2::new(-STRIP_X, ROAD_Z);
        let road_b = Vec2::new(STRIP_X, ROAD_Z);
        b.strip(ObstacleKind::Highway, road_a, road_b, 16.0, ROAD_HALF, &[]);
        let mut x = -STRIP_X + 18.0;
        while x < STRIP_X - 10.0 {
            // Clear of the pipe rack crossing and the spawns at the ends.
            b.place(ObstacleKind::PowerPole, x, ROAD_Z + 12.5, 0.0);
            x += 44.0;
        }
        for (x, k) in [(-150.0, ObstacleKind::Barrier), (-30.0, ObstacleKind::CarWreck), (70.0, ObstacleKind::Barrier), (200.0, ObstacleKind::CarWreck)] {
            let j = b.jit(5.0);
            b.place(k, x + j, ROAD_Z - 9.0, 0.0);
        }

        // ---- THE CRACKING TOWERS: columns and a lattice of pipe racks --------
        for (x, z) in [(128.0, -198.0), (146.0, -198.0), (137.0, -181.0), (200.0, -196.0), (216.0, -182.0), (176.0, -232.0), (110.0, -236.0)] {
            let (jx, jz) = (b.jit(1.0), b.jit(1.0));
            b.place(ObstacleKind::Column, x + jx, z + jz, 0.0);
        }
        // The main rack E-W, a branch N-S, and a short spur.
        b.rack(Vec2::new(62.0, -130.0), Vec2::new(242.0, -130.0));
        b.rack(Vec2::new(92.0, -248.0), Vec2::new(92.0, -146.0));
        b.rack(Vec2::new(160.0, -214.0), Vec2::new(238.0, -214.0));
        b.place(ObstacleKind::FuelTank, 175.0, -160.0, 0.0);
        b.place(ObstacleKind::FuelTank, 230.0, -160.0, 1.57);
        b.containers(60.0, -190.0, std::f32::consts::FRAC_PI_2, &[1, 1]);
        b.place(ObstacleKind::Pipe, 120.0, -150.0, 0.0);
        b.place(ObstacleKind::Pipe, 196.0, -240.0, 0.3);
        b.place(ObstacleKind::Crates, 158.0, -205.0, 0.0);
        b.place(ObstacleKind::Barrier, 68.0, -160.0, 1.2);

        // ---- THE TANK FARM: storage tanks in rows -----------------------------
        for (ix, x) in [136.0f32, 164.0, 192.0, 220.0].into_iter().enumerate() {
            for (iz, z) in [-6.0f32, 24.0, 54.0, 84.0].into_iter().enumerate() {
                // Two lanes through the farm: a gap in each.
                if (ix == 1 && iz == 2) || (ix == 3 && iz == 0) || (ix == 0 && iz == 3) || (ix == 2 && iz == 1 && b.rng.chance(0.5)) {
                    continue;
                }
                let (jx, jz) = (b.jit(0.8), b.jit(0.8));
                b.place(ObstacleKind::StorageTank, x + jx, z + jz, 0.0);
            }
        }
        b.place(ObstacleKind::Pipe, 150.0, 38.0, 1.57);
        b.place(ObstacleKind::Pipe, 206.0, 70.0, 0.0);
        b.place(ObstacleKind::Sandbags, 124.0, 102.0, 0.4);
        b.place(ObstacleKind::Barrier, 240.0, 40.0, 1.57);
        b.place(ObstacleKind::Crates, 178.0, 108.0, 0.0);

        // ---- THE DERRICK FIELD: pumpjacks, derricks, berms -------------------
        for (x, z) in [(-230.0, 20.0), (-180.0, 10.0), (-130.0, 40.0), (-200.0, 60.0), (-150.0, 120.0), (-235.0, 120.0), (-120.0, 80.0), (-190.0, 175.0), (-95.0, 0.0)] {
            let yaw = b.jit(3.1);
            let jx = b.jit(2.0);
            b.place(ObstacleKind::Pumpjack, x + jx, z, yaw);
        }
        for (x, z) in [(-165.0, 70.0), (-215.0, -40.0), (-130.0, 175.0)] {
            let yaw = b.jit(0.4);
            b.place(ObstacleKind::Derrick, x, z, yaw);
        }
        b.place(ObstacleKind::Pipe, -175.0, 35.0, 0.6);
        b.place(ObstacleKind::Pipe, -110.0, 110.0, 1.9);
        b.place(ObstacleKind::CarWreck, -205.0, 100.0, 0.5);
        b.place(ObstacleKind::Crates, -160.0, 145.0, 0.0);
        b.place(ObstacleKind::Sandbags, -240.0, 45.0, 1.4);
        b.place(ObstacleKind::LaunchPlate, -145.0, 20.0, 2.6);

        // ---- THE LOADING YARD: rails, tank cars, containers, a crane -----------
        for z in RAIL_Z {
            b.strip(ObstacleKind::Rails, Vec2::new(-STRIP_X, z), Vec2::new(STRIP_X, z), 16.0, 1.6, &[]);
        }
        // Two trains with gaps to cross between the cars.
        for (z, x0, n) in [(RAIL_Z[0], -110.0, 4), (RAIL_Z[1], 30.0, 3), (RAIL_Z[0], 120.0, 2), (RAIL_Z[1], -215.0, 2)] {
            for k in 0..n {
                let x = x0 + k as f32 * 14.6 + if k >= 2 { 14.0 } else { 0.0 };
                b.place(ObstacleKind::TankCar, x, z, 0.0);
            }
        }
        b.place(ObstacleKind::Crane, CRANE.x, CRANE.y, 0.0);
        for (x, z, lv) in [(-200.0, 245.0, [2u8, 1]), (-160.0, 246.0, [1, 3]), (-20.0, 246.0, [2, 2]), (-60.0, 176.0, [1, 2]), (80.0, 176.0, [2, 1]), (210.0, 176.0, [1, 1])] {
            let a = lv[0].min(1 + b.rng.below(3) as u8).max(1);
            b.containers(x, z, 0.0, &[a, lv[1]]);
        }
        b.place(ObstacleKind::Crates, 20.0, 182.0, 0.0);
        b.place(ObstacleKind::Barrier, -140.0, 183.0, 0.0);
        b.place(ObstacleKind::Sandbags, 120.0, 232.0, 0.0);
        b.place(ObstacleKind::Pipe, -100.0, 230.0, 0.2);

        // ---- THE FLARE STACK: the landmark on its mound ---------------------
        b.place(ObstacleKind::FlareStack, FLARE.x, FLARE.y, 0.3);
        b.place(ObstacleKind::FuelTank, FLARE.x + 14.0, FLARE.y + 8.0, 0.4);
        b.place(ObstacleKind::Pipe, FLARE.x + 30.0, FLARE.y + 22.0, 0.75);
        b.place(ObstacleKind::Sandbags, FLARE.x - 16.0, FLARE.y + 16.0, 0.8);
        b.place(ObstacleKind::Sandbags, FLARE.x + 10.0, FLARE.y - 18.0, 0.0);
        b.place(ObstacleKind::Crates, FLARE.x - 12.0, FLARE.y - 10.0, 0.0);
        b.place(ObstacleKind::LaunchPlate, FLARE.x + 40.0, FLARE.y + 38.0, -0.8);
        b.place(ObstacleKind::RockFormation, -240.0, -235.0, 0.4);
        b.place(ObstacleKind::RockFormation, -60.0, -150.0, 1.2);

        // Open-ground junk between the places.
        for (x, z, k) in [
            (-60.0, -105.0, ObstacleKind::CarWreck),
            (60.0, -95.0, ObstacleKind::Sandbags),
            (75.0, 60.0, ObstacleKind::Pipe),
            (-70.0, 80.0, ObstacleKind::Sandbags),
            (60.0, 120.0, ObstacleKind::CarWreck),
            (-60.0, 135.0, ObstacleKind::Barrier),
            (0.0, -130.0, ObstacleKind::Crates),
            (240.0, -20.0, ObstacleKind::Sandbags),
            (0.0, 100.0, ObstacleKind::Hedgehog),
        ] {
            let yaw = b.jit(3.0);
            b.place(k, x, z, yaw);
        }

        // ---- Heights --------------------------------------------------------
        let ground0 = move |x: f32, z: f32| corridor(x, z, base(x, z));
        let mut pads: Vec<Pad> = Vec::new();
        pads.push(Pad { center: Vec2::ZERO, inner: 44.0, outer: 64.0, target: ground0(0.0, 0.0) });
        // (Long pipe racks stand on the flat as it is.)
        for o in b.obstacles.iter().filter(|o| o.cover != Cover::Flat && o.kind != ObstacleKind::PipeRack) {
            let c = Vec2::new(o.pos.x, o.pos.z);
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
                // The road and rails win over pads: their decks sit flush.
                h = corridor(x, z, h);
                let edge = x.abs().max(z.abs());
                h += smoothstep(play_half - 15.0, size / 2.0, edge) * 16.0;
                heights[iz * resolution + ix] = h;
            }
        }

        let mut map = Map {
            id: MapId::Refinery,
            name: "The Refinery".to_string(),
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
        };
        let mut rng = b.rng;
        for i in 0..map.obstacles.len() {
            let o = &map.obstacles[i];
            let p = o.pos;
            let y = if o.cover == Cover::Tall && o.kind != ObstacleKind::Crane {
                // Tall cover rests on the highest ground under it (never sunk).
                o.boxes
                    .iter()
                    .filter(|bx| bx.center.y - bx.half.y < 4.0)
                    .map(|bx| {
                        let c = o.pos + o.rot * bx.center;
                        map.height_at(c.x, c.z)
                    })
                    .fold(map.height_at(p.x, p.z), f32::max)
            } else {
                map.height_at(p.x, p.z)
            };
            map.obstacles[i].pos.y = y;
        }
        for i in 0..map.spawns.len() {
            let p = map.spawns[i].pos;
            map.spawns[i].pos.y = map.height_at(p.x, p.z);
        }
        map.outpost = Vec3::new(0.0, map.height_at(0.0, 0.0), 0.0);
        let at3 = |m: &Map, p: Vec2| Vec3::new(p.x, m.height_at(p.x, p.y), p.y);
        map.regions = vec![
            Region { name: "The Control House".into(), center: map.outpost, radius: 50.0 },
            Region { name: "The Tank Farm".into(), center: at3(&map, FARM), radius: 85.0 },
            Region { name: "The Cracking Towers".into(), center: at3(&map, TOWERS), radius: 90.0 },
            Region { name: "The Derrick Field".into(), center: at3(&map, DERRICKS), radius: 95.0 },
            Region { name: "The Loading Yard".into(), center: at3(&map, YARD), radius: 110.0 },
            Region { name: "The Flare Stack".into(), center: at3(&map, FLARE), radius: 55.0 },
        ];

        // Convoy: out of the derrick field up a track to the service road, the
        // road west to east past the control house, then off the road down the
        // east side of the tank farm (its tanks go up in chains) to the unloading point.
        for (x, z) in [(-250.0, 95.0), (-245.0, 20.0)] {
            map.convoy_route.push(at3(&map, Vec2::new(x, z)));
        }
        let mut x = -STRIP_X + 22.0;

        while x < 226.0 {
            map.convoy_route.push(at3(&map, Vec2::new(x, ROAD_Z)));
            x += 36.0;
        }
        for (x, z) in [(226.0, ROAD_Z), (250.0, -32.0), (254.0, 12.0), (252.0, 60.0)] {
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
        let scrap: [(f32, f32); 48] = [
            // Control house courtyard.
            (-8.0, -6.0), (9.0, -7.0), (-9.0, 9.0), (10.0, 10.0), (0.0, 14.0),
            // Tank farm lanes.
            (150.0, 9.0), (178.0, 39.0), (206.0, 69.0), (150.0, 69.0), (234.0, 9.0), (122.0, 40.0), (206.0, -20.0),
            // Cracking towers, under the racks.
            (110.0, -130.0), (160.0, -130.0), (92.0, -200.0), (137.0, -215.0), (200.0, -165.0), (228.0, -214.0), (160.0, -250.0),
            // Derrick field.
            (-165.0, 50.0), (-210.0, 35.0), (-140.0, 95.0), (-225.0, 145.0), (-105.0, 55.0), (-180.0, -15.0),
            // Loading yard, between the rails.
            (-60.0, 206.0), (15.0, 206.0), (100.0, 206.0), (-150.0, 206.0), (190.0, 206.0), (-30.0, 230.0),
            // Flare stack.
            (-185.0, -165.0), (-205.0, -200.0), (-160.0, -200.0),
            // The road and open ground.
            (-110.0, -60.0), (40.0, -60.0), (140.0, -60.0), (-60.0, -170.0), (40.0, -170.0), (70.0, 90.0),
            (-60.0, 40.0), (60.0, 0.0), (-40.0, 150.0), (110.0, 150.0), (240.0, 100.0), (-250.0, -100.0), (-130.0, -110.0), (20.0, -215.0),
        ];
        for (i, (x, z)) in scrap.iter().enumerate() {
            let p = free(&map, Vec2::new(*x + rng.range(-2.0, 2.0), *z + rng.range(-2.0, 2.0)), 3.0);
            map.scrap_sites.push(ScrapSite { pos: at3(&map, p), kind: ScrapKind::from_index(i) });
        }
        for (x, z) in [(0.0, -8.0), (164.0, 39.0), (-170.0, 35.0), (0.0, 206.0), (150.0, -150.0)] {
            let p = free(&map, Vec2::new(x, z), 4.0);
            map.repair_sites.push(at3(&map, p));
        }
        for (x, z) in [(FLARE.x + 6.0, FLARE.y + 8.0), (178.0, 9.0)] {
            let p = free(&map, Vec2::new(x, z), 4.0);
            map.rare_sites.push(at3(&map, p));
        }
        let p = free(&map, Vec2::new(-160.0, 60.0), 4.0);
        map.strike_sites.push(at3(&map, p));
        // Wellhead derricks: the derrick field (west), the loading yard (the
        // middle, between the rails) and the west edge of the tank farm (east).
        for (x, z, name) in [(-150.0, 60.0, "the derrick field"), (-12.0, 206.0, "the loading yard"), (112.0, 36.0, "the tank farm")] {
            let p = free(&map, Vec2::new(x, z), 5.0);
            map.wells.push(WellSite { pos: at3(&map, p), name: name.into() });
        }
        for (i, o) in map.obstacles.iter().enumerate() {
            if o.kind == ObstacleKind::Crane {
                let anchor = o.pos + o.rot * Vec3::new(15.0, 23.0, 0.0);
                let ground = map.height_at(anchor.x, anchor.z);
                map.cranes.push(CraneSpawn { obstacle: i, anchor, rope: anchor.y - ground - 2.6 });
            }
        }
        // Oil drums: the compound, the tank farm, the derricks, the yard.
        for (c, n) in [(Vec2::new(-14.0, -14.0), 3), (Vec2::new(150.0, 98.0), 4), (Vec2::new(-180.0, 20.0), 3), (Vec2::new(-80.0, 228.0), 3), (Vec2::new(120.0, -150.0), 3)] {
            for k in 0..n {
                let p = free(&map, c + Vec2::new(k as f32 * 1.3, (k % 2) as f32 * 1.2), 0.8);
                map.props.push(PropSpawn { kind: PropKindSpawn::OilDrum, pos: at3(&map, p) + Vec3::Y * 0.5 });
            }
        }
        map
    }
}
