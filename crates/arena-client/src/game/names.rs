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
