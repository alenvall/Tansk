//! The map file format of the original (`data/map_standard.txt`): lines
//! `+kind, x, y[, rotation]`, kind 1 a horizontal wall, 2 a vertical wall,
//! 3 a tank spawn point. Everything else is a comment.

use crate::geom::{Aabb, V2};

pub const STANDARD: &str = include_str!("../../../assets/map_standard.txt");

pub const HORIZONTAL_WALL: (f32, f32) = (150.0, 15.0);
pub const VERTICAL_WALL: (f32, f32) = (15.0, 150.0);

#[derive(Clone, Copy, Debug)]
pub enum MapEntity {
    Wall(Aabb),
    SpawnPoint { pos: V2, rotation: f64 },
}

pub fn parse(src: &str) -> Vec<MapEntity> {
    let mut out = Vec::new();
    for line in src.lines() {
        let Some(rest) = line.trim().strip_prefix('+') else { continue };
        let p: Vec<&str> = rest.split(',').map(str::trim).collect();
        if p.len() < 3 {
            continue;
        }
        let (Ok(x), Ok(y)) = (p[1].parse::<f32>(), p[2].parse::<f32>()) else { continue };
        let pos = V2::new(x, y);
        match p[0] {
            "1" => out.push(MapEntity::Wall(Aabb::centered(pos, HORIZONTAL_WALL.0, HORIZONTAL_WALL.1))),
            "2" => out.push(MapEntity::Wall(Aabb::centered(pos, VERTICAL_WALL.0, VERTICAL_WALL.1))),
            "3" => {
                let rotation = p.get(3).and_then(|r| r.parse().ok()).unwrap_or(0.0);
                out.push(MapEntity::SpawnPoint { pos, rotation });
            }
            _ => {}
        }
    }
    out
}

pub fn standard() -> Vec<MapEntity> {
    parse(STANDARD)
}

#[cfg(test)]
mod tests {
    #[test]
    fn standard_map() {
        let m = super::standard();
        let walls = m.iter().filter(|e| matches!(e, super::MapEntity::Wall(_))).count();
        let spawns = m.len() - walls;
        assert_eq!(walls, 16);
        assert_eq!(spawns, 4);
    }
}
