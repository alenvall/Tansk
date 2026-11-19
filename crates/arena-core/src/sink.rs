//! THE SINK, laid out by hand: deliberate places linked by open ground.
//!
//! ```text
//!        N (-z)            MESA RIM along the edges
//!   PLATEAU (NW, +5 m, ramps S and E, launch plates)
//!   ============ HIGHWAY 9 (roadblock W, gas station N side, collapse E) ============
//!                 THE OUTPOST (walled compound, centre)
//!   THE YARD (SW, container maze + crane)            THE BEACH (E, ship hulls, pier, big dunes)
//!        S (+z)
//! ```
//!
//! The seed only jitters details (small offsets, stack heights, rotations);
//! the structure is fixed so the places stay recognisable. The level rule
//! holds throughout: low cover under 1.2 m, tall cover 2.5 m and up, lanes at
//! least 8 m wide.

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
        if ob.kind == ObstacleKind::ShipHull {
            ob.rot *= Quat::from_rotation_x(self.rng.range(-0.09, 0.09));
        }
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
        self.push(ob, c.x, c.y, (-d.y).atan2(d.x));
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
        let ob = Obstacle {
            kind: ObstacleKind::ContainerStack,
            cover: Cover::Tall,
            pos: Vec3::ZERO,
            rot: Quat::IDENTITY,
            boxes,
            radius: 6.6 + cols as f32 * 1.3,
        };
        self.push(ob, x, z, yaw);
    }

    fn silo(&mut self, x: f32, z: f32, r: f32, h: f32) {
        let ob = Obstacle {
            kind: ObstacleKind::Silo,
            cover: Cover::Tall,
            pos: Vec3::ZERO,
            rot: Quat::IDENTITY,
            boxes: vec![bx(Vec3::Y * (h - 0.5), Vec3::new(r, h, r), 0.0)],
            radius: r + 2.0,
        };
        self.push(ob, x, z, 0.0);
    }

    fn jit(&mut self, v: f32) -> f32 {
        self.rng.range(-v, v)
    }
}

/// The Sink's ground storytelling: dirt tracks with ruts out of each gate of
/// the outpost to the places, packed ground in the compounds, oil stains
/// where fuel is handled. `hw(t, lat)` is a point along Highway 9.
fn sink_marks(map: &Map, hw: &dyn Fn(f32, f32) -> Vec2) -> Vec<GroundMark> {
    let v = |pts: &[(f32, f32)]| pts.iter().map(|&(x, z)| Vec2::new(x, z)).collect::<Vec<_>>();
    let track = |pts: &[(f32, f32)]| GroundMark { kind: GroundMarkKind::Track, points: v(pts), width: 5.0 };
    let mut marks = vec![
        // West gate to the yard.
        track(&[(-40.0, 18.0), (-70.0, 24.0), (-96.0, 44.0), (-112.0, 70.0), (-122.0, 96.0)]),
        // North gate up to the highway.
        track(&[(-18.0, -40.0), (-22.0, -70.0), (-30.0, -100.0)]),
        // East gate out to the beach camp.
        track(&[(40.0, -18.0), (78.0, -8.0), (118.0, 4.0), (150.0, 22.0), (175.0, 30.0)]),
        // South gate down to the south spawns, past the ambushed car.
        track(&[(18.0, 40.0), (28.0, 88.0), (44.0, 140.0), (36.0, 190.0), (26.0, 222.0)]),
        // The highway up the plateau's south ramp.
        track(&[(-136.0, -108.0), (-137.0, -140.0), (-138.0, -168.0), (-150.0, -196.0)]),
        // The west loop: the yard up past the roadblock to the plateau's foot.
        track(&[(-205.0, 60.0), (-220.0, 10.0), (-212.0, -50.0), (-196.0, -104.0)]),
        // The beach camp down to the pier.
        track(&[(175.0, 30.0), (186.0, -10.0), (192.0, -50.0), (180.0, -86.0)]),
        // The yard round its crane.
        track(&[(-122.0, 96.0), (-134.0, 112.0), (-150.0, 104.0), (-168.0, 118.0), (-176.0, 150.0)]),
    ];
    let packed = |a: (f32, f32), b: (f32, f32), w: f32| GroundMark { kind: GroundMarkKind::Packed, points: v(&[a, b]), width: w };
    marks.extend([
        // The outpost courtyard, wall to wall.
        packed((-33.0, 0.0), (33.0, 0.0), 68.0),
        // The gas station forecourt and back lot.
        packed((GAS.x - 26.0, GAS.y - 4.0), (GAS.x + 26.0, GAS.y - 3.0), 36.0),
        // The yard's floor.
        packed((YARD.x - 52.0, YARD.y - 6.0), (YARD.x + 48.0, YARD.y - 6.0), 100.0),
        // The beach camp.
        packed((150.0, 26.0), (172.0, 30.0), 20.0),
    ]);
    // The roadblock: a churned turning area either side of the chicane.
    for lat in [-16.0f32, 16.0] {
        let (a, b) = (hw(190.0, lat), hw(232.0, lat));
        marks.push(GroundMark { kind: GroundMarkKind::Packed, points: vec![a, b], width: 12.0 });
    }
    // Oil: the fuel tank, the silos, the pumps, the yard's drums.
    let mut oil = vec![Vec2::new(8.0, -22.0), Vec2::new(30.0, -26.0), GAS + Vec2::new(0.0, 7.0), GAS + Vec2::new(-12.0, -18.0), YARD + Vec2::new(-10.0, -26.0)];
    for o in &map.obstacles {
        if o.kind == ObstacleKind::CarWreck {
            oil.push(Vec2::new(o.pos.x, o.pos.z));
        }
    }
    marks.push(GroundMark { kind: GroundMarkKind::Oil, points: oil, width: 5.0 });
    marks
}

/// Facing (yaw) from `p` toward `q`.
fn face(p: Vec2, q: Vec2) -> f32 {
    let d = q - p;
    (-d.y).atan2(d.x)
}

// The highway line.
const HW_A: Vec2 = Vec2::new(-262.0, -118.0);
const HW_B: Vec2 = Vec2::new(262.0, -96.0);
const HW_HALF_WIDTH: f32 = 6.5;
// Plateau (NW high ground).
const PLAT_C: Vec2 = Vec2::new(-160.0, -207.0);
const PLAT_H: Vec2 = Vec2::new(42.0, 28.0);
const PLAT_HEIGHT: f32 = 5.0;
// Regions.
const YARD: Vec2 = Vec2::new(-150.0, 125.0);
const GAS: Vec2 = Vec2::new(92.0, -135.0);

impl Map {
    /// "The Sink": a dried lake bed fought over by Scrappers. See the module docs for the layout.
    pub fn the_sink(seed: u64) -> Map {
        let size = 600.0;
        let resolution = 151;
        let play_half = 285.0;
        let mut b = Builder { obstacles: Vec::new(), rng: Rng::new(seed) };
        let s1 = seed.wrapping_mul(31).wrapping_add(1);
        let s2 = seed.wrapping_mul(131).wrapping_add(7);
        let s3 = seed.wrapping_mul(1031).wrapping_add(13);
        let dune_angle = 0.6 + b.rng.range(-0.2, 0.2);
        let dune_dir = Vec2::new(dune_angle.cos(), dune_angle.sin());

        let hw_dir = (HW_B - HW_A).normalize();
        let hw_normal = Vec2::new(-hw_dir.y, hw_dir.x);
        let hw_len = (HW_B - HW_A).length();
        let hw_mid = (HW_A + HW_B) * 0.5;

        // ---- Terrain -------------------------------------------------------
        let base = move |x: f32, z: f32| -> f32 {
            let p = Vec2::new(x, z);
            let warp = (value_noise(s1, x / 90.0, z / 90.0) - 0.5) * 60.0;
            let u = p.dot(dune_dir) + warp;
            // Bigger, longer dunes on the beach for hull-down fighting; gentle elsewhere.
            let beach = smoothstep(140.0, 200.0, x);
            let ridge = 0.5 + 0.5 * (u / (9.5 + 9.0 * beach)).sin();
            let dunes = ridge.powf(1.6) * (1.4 + 0.9 * beach);
            let broad = (value_noise(s2, x / 150.0, z / 150.0) - 0.5) * 5.0;
            let small = (value_noise(s3, x / 30.0, z / 30.0) - 0.5) * 0.6;
            let r = p.length() / 300.0;
            let basin = -2.0 * (1.0 - r * r).max(0.0);
            // The plateau: steep sides, two ramps (south, east).
            let l = p - PLAT_C;
            let dx = l.x.abs() - PLAT_H.x;
            let dz = l.y.abs() - PLAT_H.y;
            let outside = dx.max(dz);
            let ramp_s = l.y > 0.0 && (x > -150.0 && x < -122.0) && dz > dx;
            let ramp_e = l.x > 0.0 && (z > -218.0 && z < -192.0) && dx > dz;
            let w = if ramp_s || ramp_e { 36.0 } else { 6.0 };
            let plateau = PLAT_HEIGHT * (1.0 - smoothstep(0.0, w, outside));
            // The plateau top is flat: no dunes on it.
            let on_top = 1.0 - smoothstep(-6.0, 2.0, outside);
            (dunes + small) * (1.0 - on_top) + broad + basin + plateau
        };
        let road_h = move |along: f32| -> f32 {
            let mut s = 0.0;
            for k in -3..=3 {
                let p = HW_A + hw_dir * (along + k as f32 * 10.0);
                s += base(p.x, p.y);
            }
            s / 7.0
        };

        // ---- Spawns: spread round the map, each with a sandbag line ahead ----
        let spawn_xz = [
            Vec2::new(-20.0, -200.0),
            Vec2::new(178.0, -178.0),
            Vec2::new(248.0, -35.0),
            Vec2::new(190.0, 212.0),
            Vec2::new(25.0, 228.0),
            Vec2::new(-150.0, 240.0),
            Vec2::new(-248.0, 40.0),
            Vec2::new(-238.0, -150.0),
        ];
        let mut spawns = Vec::new();
        for s in spawn_xz {
            let p = s + Vec2::new(b.jit(4.0), b.jit(4.0));
            let yaw = face(p, Vec2::ZERO);
            spawns.push(Spawn { pos: Vec3::new(p.x, 0.0, p.y), yaw });
            let toward = (-p).normalize();
            let side = Vec2::new(-toward.y, toward.x);
            let c = p + toward * 10.0 + side * 15.0;
            b.place(ObstacleKind::Sandbags, c.x, c.y, face(p, Vec2::ZERO) + std::f32::consts::FRAC_PI_2);
        }

        // ---- THE OUTPOST: walled compound at the centre ----------------------
        let h = 36.0;
        for side in 0..4 {
            // Each side: segments [-36,-24], [-12,12], [24,36]; gates at +-18.
            for (a, c) in [(-h, -24.0), (-12.0, 12.0), (24.0, h)] {
                let (p, q) = match side {
                    0 => (Vec2::new(a, -h), Vec2::new(c, -h)),
                    1 => (Vec2::new(a, h), Vec2::new(c, h)),
                    2 => (Vec2::new(-h, a), Vec2::new(-h, c)),
                    _ => (Vec2::new(h, a), Vec2::new(h, c)),
                };
                b.wall(p, q);
            }
        }
        // NE corner: three yellow silos and a fuel tank (the hazards).
        b.silo(27.5, -32.0, 2.2, 4.8);
        b.silo(32.0, -27.5, 2.2, 4.8);
        b.silo(32.5, -32.5, 1.9, 4.2);
        b.place(ObstacleKind::FuelTank, 8.0, -25.0, 0.0);
        // NW corner: the watchtower over a sandbag nest.
        b.place(ObstacleKind::Tower, -29.0, -29.0, 0.3);
        // West lane: containers, one and two high.
        let lv = 1 + b.rng.below(2) as u8;
        b.containers(-25.0, -6.0, std::f32::consts::FRAC_PI_2, &[2]);
        b.containers(-25.0, 16.0, std::f32::consts::FRAC_PI_2, &[lv]);
        // East: two quonset tents.
        b.place(ObstacleKind::Tent, 24.0, 2.0, std::f32::consts::FRAC_PI_2);
        b.place(ObstacleKind::Tent, 24.0, 20.0, std::f32::consts::FRAC_PI_2);
        // South: a container and sandbag lines, crates and pallets.
        // (Clear of the south wall: a 14 m lane behind it.)
        b.containers(0.0, 20.0, 0.0, &[1]);
        b.place(ObstacleKind::Crates, -29.0, 30.0, 0.0);
        b.place(ObstacleKind::Crates, 30.0, 30.0, 0.0);
        b.place(ObstacleKind::Crates, -6.0, -27.0, 0.0);
        b.place(ObstacleKind::Crates, -14.0, -28.0, 0.0);
        // Outside the gates: hedgehogs, barriers, car wrecks.
        for (x, z, yaw) in [(-18.0, -50.0, 0.0), (18.0, 52.0, 0.0), (-52.0, 18.0, 1.57), (52.0, -18.0, 1.57)] {
            let j = b.jit(3.0);
            b.place(ObstacleKind::Barrier, x + j, z, yaw);
        }
        for (x, z) in [(-44.0, -44.0), (46.0, 44.0), (-46.0, 40.0), (44.0, -46.0), (8.0, -50.0), (-50.0, -8.0)] {
            let (jx, jz) = (b.jit(2.0), b.jit(2.0));
            b.place(ObstacleKind::Hedgehog, x + jx, z + jz, 0.0);
        }
        b.place(ObstacleKind::CarWreck, 58.0, 10.0, 0.4);
        b.place(ObstacleKind::CarWreck, -10.0, 58.0, 1.9);

        // ---- HIGHWAY 9 ----------------------------------------------------
        let seg_len = 16.0;
        let hw_yaw = (-hw_dir.y).atan2(hw_dir.x);
        let nseg = (hw_len / seg_len) as i32;
        // The collapse: no deck between these distances along the road.
        let collapse = (395.0, 432.0);
        for k in 0..nseg {
            let t = (k as f32 + 0.5) * seg_len;
            if t > collapse.0 && t < collapse.1 {
                continue;
            }
            let c = HW_A + hw_dir * t;
            let pitch = ((road_h(t + seg_len / 2.0) - road_h(t - seg_len / 2.0)) / seg_len).atan();
            b.obstacles.push(Obstacle {
                kind: ObstacleKind::Highway,
                cover: Cover::Flat,
                pos: Vec3::new(c.x, road_h(t), c.y),
                rot: Quat::from_rotation_y(hw_yaw) * Quat::from_rotation_z(pitch),
                boxes: vec![bx(Vec3::new(0.0, -0.25, 0.0), Vec3::new(seg_len / 2.0 + 0.05, 0.3, HW_HALF_WIDTH), 0.0)],
                radius: seg_len / 2.0,
            });
        }
        let at = |t: f32, lat: f32| HW_A + hw_dir * t + hw_normal * lat;
        // Barriers and hedgehogs at the lips of the collapse.
        for (t, lat) in [(collapse.0 - 4.0, -3.0), (collapse.1 + 4.0, 3.0)] {
            let p = at(t, lat);
            b.place(ObstacleKind::Barrier, p.x, p.y, hw_yaw + std::f32::consts::FRAC_PI_2);
        }
        for k in 0..4 {
            let j = b.jit(8.0);
            let p = at(collapse.0 + 8.0 + k as f32 * 7.0, j);
            b.place(ObstacleKind::Hedgehog, p.x, p.y, 0.0);
        }
        // The roadblock (west): a container chicane with hedgehogs and barriers.
        let rb = 200.0;
        let c1 = at(rb, -4.5);
        b.containers(c1.x, c1.y, hw_yaw + std::f32::consts::FRAC_PI_2, &[1]);
        let c2 = at(rb + 20.0, 4.5);
        b.containers(c2.x, c2.y, hw_yaw + std::f32::consts::FRAC_PI_2, &[1]);
        for (dt, lat) in [(-8.0, 11.0), (-6.0, 15.0), (28.0, -11.0), (30.0, -15.0)] {
            let p = at(rb + dt, lat);
            b.place(ObstacleKind::Hedgehog, p.x, p.y, 0.0);
        }
        let p = at(rb + 10.0, -15.0);
        b.place(ObstacleKind::Barrier, p.x, p.y, hw_yaw);
        let p = at(rb + 10.0, 15.0);
        b.place(ObstacleKind::Barrier, p.x, p.y, hw_yaw);
        // Wrecked car lines.
        for (t0, n) in [(95.0, 4), (275.0, 3), (470.0, 3)] {
            for k in 0..n {
                // A slalom: alternate sides of the road, 12 m apart.
                let j = if k % 2 == 0 { -4.0 } else { 4.0 } + b.jit(0.8);
                let p = at(t0 + k as f32 * 12.0, j);
                let j = b.jit(0.9);
                b.place(ObstacleKind::CarWreck, p.x, p.y, hw_yaw + j);
            }
        }
        // Power poles along the south shoulder (none across the roadblock).
        let mut t = 20.0;
        while t < hw_len - 10.0 {
            if (t - rb - 10.0).abs() > 25.0 {
                let p = at(t, 12.5);
                b.place(ObstacleKind::PowerPole, p.x, p.y, hw_yaw);
            }
            t += 44.0;
        }
        // Barrier lines on the north shoulder, with gaps.
        for t in [60.0, 150.0, 300.0, 480.0] {
            let j = b.jit(6.0);
            let p = at(t + j, -9.0);
            b.place(ObstacleKind::Barrier, p.x, p.y, hw_yaw);
        }
        // The gas station ruin, north of the road.
        let gi = b.place(ObstacleKind::GasStation, GAS.x, GAS.y, hw_yaw);
        // The canopy's back pillars stand against the shop front (no narrow
        // slot between them and the shop), the roof runs back to meet it.
        {
            let g = &mut b.obstacles[gi];
            // The roof also goes up to 6.1 m: tall masts used to catch its edge.
            if let Some(roof) = g.boxes.get_mut(1) {
                roof.center.z = 4.5;
                roof.half.z = 8.0;
                roof.center.y = 6.4;
                roof.half.x = 5.6;
            }
            for pillar in g.boxes.iter_mut().skip(2) {
                if pillar.center.z < 5.0 {
                    pillar.center.z = -2.7;
                }
                pillar.center.x = pillar.center.x.signum() * 5.2;
                pillar.center.y = 3.0;
                pillar.half.y = 3.1;
            }
        }
        b.place(ObstacleKind::CarWreck, GAS.x - 24.0, GAS.y - 2.0, 0.7);
        b.place(ObstacleKind::Crates, GAS.x - 20.0, GAS.y - 14.0, 0.0);
        // The landmark: a water tower over the shop's back lot.
        b.place(ObstacleKind::WaterTower, GAS.x + 19.0, GAS.y - 13.0, 0.0);
        // The roadblock's guard post: a watchtower on the south shoulder.
        let p = at(rb - 8.0, 23.0);
        b.place(ObstacleKind::Tower, p.x, p.y, hw_yaw + 0.2);

        // ---- THE YARD: container maze around the crane ---------------------
        b.place(ObstacleKind::Crane, YARD.x, YARD.y, 0.0);
        let tip = YARD + Vec2::new(15.0, 0.0);
        for ix in 0..6 {
            for iz in 0..8 {
                // Lanes at least 12 m wide both ways.
                let x = -215.0 + ix as f32 * 25.0;
                let z = 66.0 + iz as f32 * 17.0;
                let p = Vec2::new(x, z);
                // Keep the crane's swing and its tower clear, two cells always open
                // (small squares in the maze), and some more at random.
                let open = (ix == 1 && iz == 5) || (ix == 4 && iz == 2);
                if open || p.distance(tip) < 30.0 || p.distance(YARD) < 14.0 || b.rng.chance(0.3) {
                    continue;
                }
                let a = 1 + b.rng.below(3) as u8;
                if b.rng.chance(0.4) {
                    let c = 1 + b.rng.below(2) as u8;
                    b.containers(x, z, 0.0, &[a, c]);
                } else {
                    b.containers(x, z, 0.0, &[a]);
                }
            }
        }
        b.place(ObstacleKind::Crates, YARD.x + 18.0, YARD.y - 62.0, 0.0);
        b.place(ObstacleKind::Pipe, YARD.x + 30.0, YARD.y - 75.0, 0.3);

        // ---- THE BEACH: beached hulls, the pier, big dunes ------------------
        let j = b.jit(0.1);
        b.place(ObstacleKind::ShipHull, 210.0, 45.0, 0.35 + j);
        let j = b.jit(0.1);
        b.place(ObstacleKind::ShipHull, 222.0, 160.0, -0.45 + j);
        let j = b.jit(0.1);
        b.place(ObstacleKind::PierRuin, 195.0, -78.0, j);
        let j = b.jit(0.1);
        b.place(ObstacleKind::PierRuin, 240.0, -70.0, 0.2 + j);
        b.place(ObstacleKind::CarWreck, 165.0, 132.0, 1.2);
        // The salvagers' camp on the beach: a wind pump (the landmark), a tent, crates.
        b.place(ObstacleKind::Windmill, 166.0, 18.0, 0.3);
        b.place(ObstacleKind::Tent, 152.0, 38.0, 1.2);
        b.place(ObstacleKind::Crates, 160.0, 24.0, 0.0);
        b.place(ObstacleKind::Pipe, 245.0, 95.0, 1.1);

        // ---- MESA RIM and open rocks --------------------------------------
        for (x, z) in [(-60.0, -262.0), (140.0, -258.0), (-266.0, 150.0), (-60.0, 266.0), (100.0, 266.0), (268.0, -150.0)] {
            let yaw = b.jit(0.4);
            b.place(ObstacleKind::Mesa, x, z, yaw);
        }
        for (x, z) in [(-85.0, -58.0), (76.0, 78.0), (118.0, -36.0), (-48.0, 152.0), (48.0, -168.0)] {
            let yaw = b.jit(3.0);
            b.place(ObstacleKind::RockFormation, x, z, yaw);
        }
        // Plateau: launch plates at its foot, sandbags on its lip.
        b.place(ObstacleKind::LaunchPlate, 180.0, 108.0, 0.0); // beach dune jump (first plate)
        b.place(ObstacleKind::LaunchPlate, PLAT_C.x + 5.0, PLAT_C.y + PLAT_H.y + 13.0, std::f32::consts::FRAC_PI_2);
        b.place(ObstacleKind::LaunchPlate, PLAT_C.x - PLAT_H.x - 14.0, PLAT_C.y - 5.0, 0.0);
        b.place(ObstacleKind::Sandbags, PLAT_C.x - 10.0, PLAT_C.y + PLAT_H.y - 4.0, 0.0);
        b.place(ObstacleKind::Sandbags, PLAT_C.x + PLAT_H.x - 5.0, PLAT_C.y - 8.0, 1.57);
        b.place(ObstacleKind::Crates, PLAT_C.x - 5.0, PLAT_C.y - 20.0, 0.0);
        // The landmark: a guyed radio mast on the top, a lookout tower on
        // its south-west corner over the yard.
        b.place(ObstacleKind::RadioMast, PLAT_C.x - 28.0, PLAT_C.y - 18.0, 0.4);
        b.place(ObstacleKind::Tower, PLAT_C.x - 30.0, PLAT_C.y + 4.0, 0.5);
        // Out on the open ground: not sprinkled, but a few deliberate spots
        // on the tracks between the places (and long empty sightlines between).
        for (x, z, k, yaw) in [
            // The west checkpoint, where the yard track leaves the outpost road.
            (-96.0, 34.0, ObstacleKind::CarWreck, 0.5),
            (-88.0, 22.0, ObstacleKind::Sandbags, 0.4),
            // The east checkpoint on the beach track.
            (104.0, 8.0, ObstacleKind::Sandbags, -0.3),
            (112.0, -4.0, ObstacleKind::Barrier, 1.2),
            // An ambushed car on the south track, its cargo spilt.
            (38.0, 124.0, ObstacleKind::CarWreck, 1.3),
            (46.0, 132.0, ObstacleKind::Crates, 0.0),
            // A pipe dump at the yard's gate.
            (-76.0, 50.0, ObstacleKind::Pipe, 0.2),
            // A supply cache below the north spawn.
            (-14.0, -160.0, ObstacleKind::Crates, 0.0),
        ] {
            let j = b.jit(0.2);
            b.place(k, x, z, yaw + j);
        }

        // ---- Heights ------------------------------------------------------
        let road_blend = move |x: f32, z: f32, h: f32| -> f32 {
            let p = Vec2::new(x, z);
            let along = (p - HW_A).dot(hw_dir);
            let d = (p - HW_A).dot(hw_normal).abs();
            if along > -12.0 && along < hw_len + 12.0 && d < HW_HALF_WIDTH + 14.0 {
                let w = (1.0 - smoothstep(HW_HALF_WIDTH + 1.0, HW_HALF_WIDTH + 14.0, d))
                    * (1.0 - smoothstep(hw_len * 0.5, hw_len * 0.5 + 12.0, (along - hw_len * 0.5).abs()));
                h + (road_h(along) - h) * w
            } else {
                h
            }
        };
        let ground0 = move |x: f32, z: f32| -> f32 { road_blend(x, z, base(x, z)) };
        let mut pads: Vec<Pad> = Vec::new();
        for (c, inner, outer) in [(Vec2::ZERO, 44.0, 64.0), (YARD, 70.0, 92.0), (GAS, 18.0, 32.0)] {
            pads.push(Pad { center: c, inner, outer, target: ground0(c.x, c.y) });
        }
        for o in b.obstacles.iter().filter(|o| o.kind != ObstacleKind::Highway) {
            let c = Vec2::new(o.pos.x, o.pos.z);
            let r = if o.kind == ObstacleKind::Mesa { o.radius * 0.5 } else { o.radius * 0.8 };
            // Hulls get a wide flat apron: room to drive round them clear of the dunes.
            let apron = if o.kind == ObstacleKind::ShipHull { 20.0 } else { 10.0 };
            pads.push(Pad { center: c, inner: r, outer: o.radius + apron, target: ground0(c.x, c.y) });
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
                // The road wins over pads: its deck must sit flush.
                h = road_blend(x, z, h);
                let edge = x.abs().max(z.abs());
                h += rim(play_half, size, edge, 16.0);
                heights[iz * resolution + ix] = h;
            }
        }

        let mut map = Map {
            id: MapId::Sink,
            name: "The Sink".to_string(),
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
            if map.obstacles[i].kind == ObstacleKind::Highway {
                continue;
            }
            let p = map.obstacles[i].pos;
            map.obstacles[i].pos.y = map.height_at(p.x, p.z);
            if map.obstacles[i].cover == Cover::Tall && map.obstacles[i].kind != ObstacleKind::Crane {
                // Tall cover rests on the highest ground under them (never sunk below 2.6 m of cover).
                let o = &map.obstacles[i];
                let top = o.boxes.iter().map(|bx| { let c = o.pos + o.rot * bx.center; map.height_at(c.x, c.z) }).fold(f32::MIN, f32::max);
                map.obstacles[i].pos.y = top;
            }
        }
        for i in 0..map.spawns.len() {
            let p = map.spawns[i].pos;
            map.spawns[i].pos.y = map.height_at(p.x, p.z);
        }
        map.outpost = Vec3::new(0.0, map.height_at(0.0, 0.0), 0.0);
        map.aim_spawns();
        let at3 = |m: &Map, p: Vec2| Vec3::new(p.x, m.height_at(p.x, p.y), p.y);
        let hw_mid3 = at3(&map, hw_mid);
        let plat3 = at3(&map, PLAT_C);
        map.regions = vec![
            Region { name: "The Outpost".into(), center: map.outpost, radius: 50.0 },
            Region { name: "Highway 9".into(), center: hw_mid3, radius: 270.0 },
            Region { name: "The Gas Station".into(), center: at3(&map, GAS), radius: 30.0 },
            Region { name: "The Yard".into(), center: at3(&map, YARD), radius: 80.0 },
            Region { name: "The Beach".into(), center: at3(&map, Vec2::new(215.0, 70.0)), radius: 110.0 },
            Region { name: "The Plateau".into(), center: plat3, radius: 50.0 },
        ];

        // Convoy: Highway 9, west to east: past the car slalom, through the
        // roadblock chicane, round the collapse, out past the beach.
        let mut t = 10.0;
        while t < hw_len - 10.0 {
            map.convoy_route.push(at3(&map, HW_A + hw_dir * t));
            t += 36.0;
        }
        map.convoy_route.push(at3(&map, HW_A + hw_dir * (hw_len - 10.0)));

        // Pickup sites: every region gets a reason to visit. Each site is nudged

        // to the nearest free spot.
        let free = |m: &Map, p: Vec2, margin: f32| -> Vec2 {
            for r in 0..12 {
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
        let scrap: [(f32, f32); 47] = [
            // Outpost courtyard.
            (-8.0, -6.0), (9.0, -7.0), (-9.0, 8.0), (10.0, 9.0), (0.0, -14.0),
            // Highway and gas station.
            (92.0, -113.0), (74.0, -142.0), (60.0, -108.0), (-62.0, -100.0), (-150.0, -122.0), (165.0, -105.0), (225.0, -96.0), (116.0, -124.0),
            // The yard.
            (-138.0, 108.0), (-172.0, 92.0), (-118.0, 160.0), (-200.0, 140.0), (-100.0, 95.0), (-160.0, 175.0),
            // The beach.
            (180.0, 0.0), (198.0, 100.0), (238.0, 122.0), (176.0, 174.0), (242.0, 10.0), (175.0, 60.0),
            // The plateau.
            (-172.0, -212.0), (-145.0, -200.0), (-185.0, -195.0),
            // Open ground.
            (60.0, -60.0), (-62.0, 42.0), (82.0, 142.0), (-102.0, -170.0), (20.0, 100.0), (-112.0, -20.0),
            (130.0, 40.0), (40.0, 192.0), (-205.0, 5.0), (-222.0, 190.0), (142.0, -140.0), (-32.0, -92.0),
            (-60.0, -150.0), (110.0, 230.0), (-120.0, 230.0), (240.0, -200.0), (0.0, 160.0), (-240.0, -80.0), (70.0, -230.0),
        ];
        for (i, (x, z)) in scrap.iter().enumerate() {
            let p = free(&map, Vec2::new(*x + rng.range(-2.0, 2.0), *z + rng.range(-2.0, 2.0)), 3.0);
            map.scrap_sites.push(ScrapSite { pos: at3(&map, p), kind: ScrapKind::from_index(i) });
        }
        for (x, z) in [(0.0, 8.0), (96.0, -126.0), (-150.0, 165.0), (200.0, 20.0)] {
            let p = free(&map, Vec2::new(x, z), 4.0);
            map.repair_sites.push(at3(&map, p));
        }
        for (x, z) in [(-160.0, -207.0), (-170.0, 104.0)] {
            let p = free(&map, Vec2::new(x, z), 4.0);
            map.rare_sites.push(at3(&map, p));
        }
        let p = free(&map, Vec2::new(205.0, 105.0), 4.0);
        map.strike_sites.push(at3(&map, p));
        // Wellhead derricks: the gas station forecourt (north), the outpost
        // courtyard (the middle) and the beach dunes (east).
        for (x, z, name) in [(80.0, -128.0, "the gas station"), (0.0, -4.0, "the outpost"), (190.0, 92.0, "the beach")] {
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
        // Oil drums: gas station, outpost, yard, roadblock.
        let rb_p = HW_A + hw_dir * (rb + 10.0) + hw_normal * 19.0;
        for (c, n) in [(GAS + Vec2::new(-12.0, -18.0), 4), (Vec2::new(-14.0, -20.0), 3), (YARD + Vec2::new(-10.0, -26.0), 3), (rb_p, 3)] {
            for k in 0..n {
                let p = free(&map, c + Vec2::new(k as f32 * 1.3, (k % 2) as f32 * 1.2), 0.8);
                map.props.push(PropSpawn { kind: PropKindSpawn::OilDrum, pos: at3(&map, p) + Vec3::Y * 0.5 });
            }
        }
        map.marks = sink_marks(&map, &|t, lat| at(t, lat));
        map
    }
}
