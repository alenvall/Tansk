//! What things are called on the HUD, in the kill feed and on the radio.

use arena_core::events::Weapon;
use arena_core::parts::{AltKind, GunKind, PartKind};
use arena_core::ScrapKind;

use crate::ui;

pub fn part_name(p: PartKind) -> &'static str {
    use PartKind::*;
    match p {
        StubGun => "Stub gun",
        HeavyCannon => "Heavy cannon",
        BigBore => "Big Bore",
        TwinBarrel => "Twin Barrel",
        LongGun => "Long Gun",
        SensorMast => "Sensor mast",
        Coilgun => "Coilgun",
        TeslaSpire => "Tesla Spire",
        FlankPlates => "Flank plates",
        Bulwark => "Bulwark",
        SpacedArmour => "Spaced armour",
        Nitro => "Nitro tank",
        Afterburner => "Afterburner",
        TwinEngines => "Twin engines",
        Flamethrower => "Flamethrower",
        ShieldGenerator => "Shield generator",
        RocketRack => "Rocket rack",
        SpikedRam => "Spiked ram",
        Mortar => "Mortar",
        Harpoon => "Harpoon",
        SmokeLauncher => "Smoke launcher",
        MineLayer => "Mine layer",
        RepairRig => "Repair rig",
        RigCannons => "twin cannons",
        RigRockets => "rocket pods",
        RigFlamer => "flamer",
        RigPlateLeft => "left plate",
        RigPlateRight => "right plate",
        RigPlateFront => "front plate",
        RigEngineLeft => "left engine",
        RigEngineRight => "right engine",
        RigMast => "mast",
        #[allow(unreachable_patterns)]
        _ => "War rig part",
    }
}

/// One line for the branch card.
pub fn part_blurb(p: PartKind) -> &'static str {
    use PartKind::*;
    match p {
        BigBore => "Huge slow shells. Splash.",
        TwinBarrel => "Two barrels at once. Up close.",
        LongGun => "Fast flat shots, deep scope.",
        Coilgun => "Hold to charge. Pierces.",
        TeslaSpire => "Arcs between nearby tanks.",
        Bulwark => "Front slab, ram plow. Heavy.",
        SpacedArmour => "All-round plates, lighter.",
        Afterburner => "Long boosts. Ramming hurts.",
        TwinEngines => "Top speed, quicker turret.",
        _ => "",
    }
}

pub fn gun_part(g: GunKind) -> PartKind {
    match g {
        GunKind::Stub => PartKind::StubGun,
        GunKind::HeavyCannon => PartKind::HeavyCannon,
        GunKind::BigBore => PartKind::BigBore,
        GunKind::TwinBarrel => PartKind::TwinBarrel,
        GunKind::LongGun => PartKind::LongGun,
        #[allow(unreachable_patterns)]
        _ => PartKind::HeavyCannon,
    }
}

pub fn weapon_name(w: Weapon) -> &'static str {
    match w {
        Weapon::Gun(g) => match g {
            GunKind::Stub => "stub gun",
            GunKind::HeavyCannon => "heavy cannon",
            GunKind::BigBore => "Big Bore",
            GunKind::TwinBarrel => "Twin Barrel",
            GunKind::LongGun => "Long Gun",
            #[allow(unreachable_patterns)]
            _ => "rig cannons",
        },
        Weapon::Alt(a) => match a {
            AltKind::Mg => "MG",
            AltKind::Coilgun => "coilgun",
            AltKind::Tesla => "tesla",
            AltKind::Flamethrower => "flamer",
            AltKind::Rockets => "rockets",
        },
        Weapon::Ram => "ram",
        Weapon::World => "the Sink",
        Weapon::Special(p) => part_name(p),
        Weapon::AirStrike => "air strike",
        Weapon::Hazard => "fuel blast",
        Weapon::Burn => "fire",
        Weapon::Storm => "the storm",
    }
}

pub fn scrap_name(k: ScrapKind) -> &'static str {
    match k {
        ScrapKind::Steel => "Steel",
        ScrapKind::Powder => "Powder",
        ScrapKind::Volts => "Volts",
        ScrapKind::Fuel => "Fuel",
    }
}

pub fn ui_scrap(k: ScrapKind) -> ui::ScrapKind {
    match k {
        ScrapKind::Steel => ui::ScrapKind::Steel,
        ScrapKind::Powder => ui::ScrapKind::Powder,
        ScrapKind::Volts => ui::ScrapKind::Volts,
        ScrapKind::Fuel => ui::ScrapKind::Fuel,
    }
}

/// A tier-2 branch option (picked on a card).
pub fn is_branch(p: PartKind) -> bool {
    use PartKind::*;
    matches!(p, BigBore | TwinBarrel | LongGun | Coilgun | TeslaSpire | Bulwark | SpacedArmour | Afterburner | TwinEngines)
}

/// Which track a part grows on (combos and rares: none).
pub fn track_of(p: PartKind) -> Option<usize> {
    use PartKind::*;
    match p {
        FlankPlates | Bulwark | SpacedArmour => Some(0),
        StubGun | HeavyCannon | BigBore | TwinBarrel | LongGun => Some(1),
        SensorMast | Coilgun | TeslaSpire => Some(2),
        Nitro | Afterburner | TwinEngines => Some(3),
        _ => None,
    }
}

/// Compass point of a flat direction (north is -Z).
pub fn compass(dx: f32, dz: f32) -> &'static str {
    let a = dx.atan2(-dz).to_degrees();
    let a = if a < 0.0 { a + 360.0 } else { a };
    const P: [&str; 8] = ["north", "north-east", "east", "south-east", "south", "south-west", "west", "north-west"];
    P[(((a + 22.5) / 45.0) as usize) % 8]
}

/// Where something is, for the radio: the named region it sits in
/// ("on the Plateau"), else a compass direction from `from` ("to the north").
pub fn place(map: &arena_core::Map, pos: glam::Vec3, from: glam::Vec3, moving: bool) -> String {
    let flat = |v: glam::Vec3| glam::Vec2::new(v.x, v.z);
    let inside = map
        .regions
        .iter()
        .filter(|r| flat(r.center).distance(flat(pos)) < r.radius)
        .min_by(|a, b| flat(a.center).distance(flat(pos)).total_cmp(&flat(b.center).distance(flat(pos))));
    if let Some(r) = inside {
        return format!("{} {}", region_preposition(&r.name), region_noun(&r.name));
    }
    let d = pos - from;
    if moving {
        format!("heading {}", compass(d.x, d.z))
    } else {
        format!("to the {}", compass(d.x, d.z))
    }
}

fn region_preposition(name: &str) -> &'static str {
    let n = name.to_lowercase();
    if n.contains("highway") || n.contains("beach") || n.contains("plateau") {
        "on"
    } else if n.contains("yard") {
        "in"
    } else {
        "at"
    }
}

/// "Highway 9" stays bare; the rest take "the".
fn region_noun(name: &str) -> String {
    if name.chars().any(|c| c.is_ascii_digit()) || name.to_lowercase().starts_with("the ") {
        name.to_string()
    } else {
        format!("the {name}")
    }
}
