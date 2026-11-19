//! Where do bots get stuck? Prints the worst 20 m cells.
//!
//!     cargo run --release -p arena-core --example stuckmap -- [sink|refinery]
use arena_core::sim::Sim;
use arena_core::*;
use std::collections::BTreeMap;
fn main() {
    let id = std::env::args().nth(1).and_then(|s| MapId::from_key(&s)).unwrap_or(MapId::Sink);
    let mut cells: BTreeMap<(i32, i32), u32> = BTreeMap::new();
    for seed in 0..6u64 {
        let mut sim = Sim::new(seed, Map::by_id(id, seed), Tuning::default());
        for k in 0..8 { sim.add_bot(arena_core::bots::BOT_NAMES[k], 0.5); }
        for tick in 0..(4 * 3600) {
            sim.step();
            sim.drain_events();
            if tick % 6 != 0 { continue; }
            for p in &sim.players {
                if !p.tank.alive || p.input.throttle.abs() < 0.4 { continue; }
                let b = sim.phys.body(p.tank.body);
                if b.linvel().length() < 0.5 {
                    let c = b.translation();
                    *cells.entry(((c.x / 20.0).floor() as i32, (c.z / 20.0).floor() as i32)).or_default() += 1;
                }
            }
        }
    }
    let mut v: Vec<_> = cells.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    for ((x, z), n) in v.iter().take(20) {
        let c = glam::Vec2::new(*x as f32 * 20.0 + 10.0, *z as f32 * 20.0 + 10.0);
        let map = Map::by_id(id, 0);
        let near: Vec<String> = map.obstacles.iter().filter(|o| glam::Vec2::new(o.pos.x, o.pos.z).distance(c) < 18.0).map(|o| format!("{:?}", o.kind)).collect();
        println!("cell ({:5.0},{:5.0}) {n:5}  near {:?}", c.x, c.y, near);
    }
}
