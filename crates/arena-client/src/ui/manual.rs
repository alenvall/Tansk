//! The Workshop manual: the scrap tracks, their branches, every gun and
//! gadget, the combos, the golden-crate parts, the pickups and the chassis.
//! Every number comes from `arena_core`'s tuning and part tables, so the
//! manual says what the game does. Also "your build" for the pause card.

use std::fmt::Write as _;

use arena_core::map::MapId;
use arena_core::parts::{branch_options, tier1_part, PartKind, COMBOS, RARE_PARTS};
use arena_core::tuning::{GunStats, Tuning};
use arena_core::ScrapKind;

use super::esc;
use crate::game::names::{part_name, scrap_name};

const TRACKS: [ScrapKind; 4] = [ScrapKind::Steel, ScrapKind::Powder, ScrapKind::Volts, ScrapKind::Fuel];

fn key(k: ScrapKind) -> &'static str {
    match k {
        ScrapKind::Steel => "steel",
        ScrapKind::Powder => "powder",
        ScrapKind::Volts => "volts",
        ScrapKind::Fuel => "fuel",
    }
}

fn tagline(k: ScrapKind) -> &'static str {
    match k {
        ScrapKind::Steel => "Armour. Take more, shrug off splash, hit harder when you ram.",
        ScrapKind::Powder => "The main gun. Each tier swaps in a bigger one.",
        ScrapKind::Volts => "Gadgets on alt fire (right click) and a taller look-out.",
        ScrapKind::Fuel => "Boost (Shift), speed and the rams that come with it.",
    }
}

/// Rounds for reading: 3.5 stays, 35.0 shows as 35.
fn n(v: f32) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

fn pct(mult: f32) -> String {
    let p = ((mult - 1.0) * 100.0).round() as i64;
    if p >= 0 {
        format!("+{p}%")
    } else {
        format!("{p}%")
    }
}

fn gun_stats(t: &Tuning, p: PartKind) -> Option<&GunStats> {
    let g = &t.guns;
    Some(match p {
        PartKind::StubGun => &g.stub,
        PartKind::HeavyCannon => &g.heavy_cannon,
        PartKind::BigBore => &g.big_bore,
        PartKind::TwinBarrel => &g.twin_barrel,
        PartKind::LongGun => &g.long_gun,
        _ => return None,
    })
}

/// A main gun in plain words: damage, splash, reload, reach.
fn gun_line(g: &GunStats) -> String {
    let dmg = if g.shells_per_shot > 1 { format!("{} x {}", g.shells_per_shot, n(g.damage)) } else { n(g.damage) };
    let reach = g.shell_speed * g.lifetime;
    let mut s = format!("{dmg} damage, {} splash over {} m, reloads in {} s, reaches about {} m", n(g.splash_damage), n(g.splash_radius), n(g.reload), n((reach / 10.0).round() * 10.0));
    if g.falloff_start < reach {
        let _ = write!(s, " (hits fade past {} m)", n(g.falloff_start));
    }
    s
}

fn good_for(p: PartKind) -> &'static str {
    match p {
        PartKind::StubGun => "getting started. Grab Powder.",
        PartKind::HeavyCannon => "everything, a bit.",
        PartKind::BigBore => "crowds, cover and knocking tanks about.",
        PartKind::TwinBarrel => "knife fights round the containers.",
        PartKind::LongGun => "picking parts off from far away (Z to zoom).",
        _ => "",
    }
}

/// What a part does, in one line, with the game's numbers.
pub fn part_effect(t: &Tuning, p: PartKind) -> String {
    let a = &t.alt;
    let s = &t.special;
    let pt = &t.parts;
    match p {
        PartKind::StubGun | PartKind::HeavyCannon | PartKind::BigBore | PartKind::TwinBarrel | PartKind::LongGun => {
            gun_stats(t, p).map(gun_line).unwrap_or_default()
        }
        PartKind::SensorMast => format!("A look-out pole: you see {} m higher over cover.", n(pt.mast_sight)),
        PartKind::Coilgun => format!(
            "Hold alt fire {} s to charge: {} damage, punches through {} tanks, {} m.",
            n(a.coil_charge_time),
            n(a.coil_damage),
            a.coil_pierce,
            n(a.coil_range)
        ),
        PartKind::TeslaSpire => format!(
            "Alt fire arcs {} damage into a tank within {} m, then jumps to {} more close by.",
            n(a.tesla_damage),
            n(a.tesla_range),
            a.tesla_chain_count
        ),
        PartKind::FlankPlates => format!("Side plates. Every Steel tier adds {} hull.", n(pt.steel_health_per_tier)),
        PartKind::Bulwark => format!("A front slab and plough: rams hit {} harder.", pct(pt.bulwark_ram)),
        PartKind::SpacedArmour => format!("Plates all round: splash hurts {} as much.", format!("{}%", (pt.spaced_splash * 100.0).round())),
        PartKind::Nitro => "Unlocks the boost (Shift).".into(),
        PartKind::Afterburner => format!("{} fuel for long boosts; boosted rams {}.", pct(pt.afterburner_fuel), pct(pt.afterburner_ram)),
        PartKind::TwinEngines => format!("{} top speed, {} faster turret.", pct(pt.twin_engines_speed), pct(pt.twin_engines_traverse)),
        PartKind::Flamethrower => format!(
            "Alt fire: a {} m cone, {} damage a second, sets them burning ({} a second for {} s).",
            n(s.flame_range),
            n(s.flame_dps),
            n(s.burn_dps),
            n(s.burn_time)
        ),
        PartKind::ShieldGenerator => format!(
            "A {}-point bubble takes hits first; it refills {} s after the last one.",
            n(s.shield_hp),
            n(s.shield_recharge_delay)
        ),
        PartKind::RocketRack => format!(
            "Hold your aim on a tank {} s to lock, alt fire: {} homing rockets, {} damage each.",
            n(s.rocket_lock_time),
            s.rocket_count,
            n(s.rocket_damage)
        ),
        PartKind::SpikedRam => "Spikes on the bow: rams hurt more the faster you hit.".into(),
        PartKind::Mortar => format!(
            "F lobs a shell over cover: {} damage, {} splash over {} m. {} shells, they come back.",
            n(s.mortar_damage),
            n(s.mortar_splash),
            n(s.mortar_radius),
            s.mortar_ammo
        ),
        PartKind::Harpoon => format!("F fires a {} m line: hook a tank, a crate or a wall and reel in.", n(s.harpoon_range)),
        PartKind::SmokeLauncher => format!("F drops a {} m smoke screen for {} s. {} charges.", n(s.smoke_radius), n(s.smoke_life), s.smoke_charges),
        PartKind::MineLayer => format!(
            "F drops a mine behind you: {} damage, armed after {} s. {} at a time.",
            n(s.mine_damage),
            n(s.mine_arm_time),
            s.mine_max_active
        ),
        PartKind::RepairRig => format!("Patches {} hull a second once you've been out of the fight {} s.", n(s.repair_rig_hps), n(s.repair_rig_delay)),
        _ => String::new(),
    }
}

fn icon(k: ScrapKind) -> String {
    format!("<svg class=\"t-man-ic\" aria-hidden=\"true\"><use href=\"#i-{}\"/></svg>", key(k))
}

fn part_row(t: &Tuning, p: PartKind, label: &str) -> String {
    format!(
        "<div class=\"t-man-part\"><b>{}</b><em>{}</em><p>{}</p></div>",
        esc(part_name(p)),
        esc(label),
        esc(&part_effect(t, p))
    )
}

/// The whole manual's body.
pub fn manual_html() -> String {
    let t = Tuning::default();
    let up = &t.upgrades;
    let mut h = String::new();

    // How growing works.
    let _ = write!(
        h,
        "<section class=\"t-man-sec\" id=\"t-man-grow\"><h3>How your machine grows</h3><ul class=\"t-man-rules\">\
         <li><b>{} of a kind</b> bolts on the next tier of that track (a Hauler needs {}). Tracks go to tier {}.</li>\
         <li>At <b>tier 2</b> you pick a branch: <kbd>1</kbd><kbd>2</kbd><kbd>3</kbd> within {} s, or the first one sticks.</li>\
         <li><b>Tier {}</b> makes your branch part a Mk II: guns {} damage and {} reload, Volts gadgets {} damage, every Mk II part {} tougher.</li>\
         <li><b>Two tracks at tier 2</b> unlock a combo part: it bolts on with your next upgrade.</li>\
         <li>Parts can be <b>shot off</b>, and you lose that tier: the scrap falls out for anyone. Wrecked, you keep tier {} in every track.</li>\
         <li>Scrap for a maxed track patches {} hull instead.</li></ul></section>",
        up.scrap_per_tier,
        t.chassis.hauler.scrap_per_tier,
        up.max_tier,
        n(up.branch_choice_time),
        up.max_tier,
        pct(t.guns.mk2_damage),
        pct(t.guns.mk2_reload),
        pct(t.alt.mk2_damage),
        pct(t.parts.mk2_health),
        up.respawn_keep_tier,
        n(up.overflow_heal),
    );

    // The tracks.
    h.push_str("<section class=\"t-man-sec\" id=\"t-man-tracks\"><h3>The four scrap tracks</h3><div class=\"t-man-grid\">");
    for k in TRACKS {
        let _ = write!(
            h,
            "<article class=\"t-man-card plate\" data-kind=\"{}\"><header>{}<div><h4>{}</h4><p>{}</p></div></header>",
            key(k),
            icon(k),
            scrap_name(k),
            tagline(k)
        );
        h.push_str(&part_row(&t, tier1_part(k), "Tier 1"));
        let opts = branch_options(k);
        let _ = write!(h, "<div class=\"t-man-branch\">Tier 2: pick one of {}</div>", opts.len());
        for (i, p) in opts.iter().enumerate() {
            h.push_str(&part_row(&t, *p, &format!("Branch {}{}", i + 1, if i == 0 { " (default)" } else { "" })));
        }
        let _ = write!(h, "<div class=\"t-man-branch\">Tier {}: your branch part goes Mk II</div></article>", up.max_tier);
    }
    h.push_str("</div></section>");

    // The guns.
    h.push_str("<section class=\"t-man-sec\" id=\"t-man-guns\"><h3>Main guns <span>left click</span></h3><div class=\"t-man-grid\">");
    for p in [PartKind::StubGun, PartKind::HeavyCannon, PartKind::BigBore, PartKind::TwinBarrel, PartKind::LongGun] {
        let _ = write!(
            h,
            "<article class=\"t-man-card plate\" data-kind=\"powder\"><header>{}<div><h4>{}</h4></div></header><p>{}.</p><p class=\"t-man-good\">Good for: {}</p></article>",
            icon(ScrapKind::Powder),
            esc(part_name(p)),
            esc(&part_effect(&t, p)),
            good_for(p)
        );
    }
    h.push_str("</div></section>");

    // Alt fire.
    let a = &t.alt;
    let _ = write!(
        h,
        "<section class=\"t-man-sec\" id=\"t-man-alt\"><h3>Second guns <span>right click</span></h3><div class=\"t-man-grid\">\
         <article class=\"t-man-card plate\"><header><div><h4>Coaxial MG</h4></div></header><p>Everyone has one: {} rounds a second, {} damage each, {} m.</p>\
         <p class=\"t-man-good\">Good for: finishing, and chewing parts off.</p></article>",
        n(a.mg_rate),
        n(a.mg_damage),
        n(a.mg_range)
    );
    for p in [PartKind::Coilgun, PartKind::TeslaSpire, PartKind::Flamethrower, PartKind::RocketRack] {
        let k = if matches!(p, PartKind::Flamethrower | PartKind::RocketRack) { ScrapKind::Powder } else { ScrapKind::Volts };
        let _ = write!(
            h,
            "<article class=\"t-man-card plate\" data-kind=\"{}\"><header>{}<div><h4>{}</h4></div></header><p>{}</p></article>",
            key(k),
            icon(k),
            esc(part_name(p)),
            esc(&part_effect(&t, p))
        );
    }
    h.push_str("</div></section>");

    // Combos.
    h.push_str("<section class=\"t-man-sec\" id=\"t-man-combos\"><h3>Combos <span>two tracks at tier 2</span></h3><div class=\"t-man-grid\">");
    for (x, y, p) in COMBOS {
        let _ = write!(
            h,
            "<article class=\"t-man-card plate\"><header>{}{}<div><h4>{}</h4><p>{} + {}</p></div></header><p>{}</p></article>",
            icon(x),
            icon(y),
            esc(part_name(p)),
            scrap_name(x),
            scrap_name(y),
            esc(&part_effect(&t, p))
        );
    }
    h.push_str("</div></section>");

    // Golden crates.
    let s = &t.special;
    h.push_str("<section class=\"t-man-sec\" id=\"t-man-rare\"><h3>Golden crates <span>F to use</span></h3><div class=\"t-man-grid\">");
    for p in RARE_PARTS {
        let _ = write!(
            h,
            "<article class=\"t-man-card plate t-man-gold\"><header><div><h4>{}</h4><p>Rare part</p></div></header><p>{}</p></article>",
            esc(part_name(p)),
            esc(&part_effect(&t, p))
        );
    }
    let _ = write!(
        h,
        "<article class=\"t-man-card plate t-man-gold\"><header><div><h4>Air strike</h4><p>Flare crate</p></div></header>\
         <p>F throws the flare (up to {} m): {} s later {} bombs walk a {} m line, {} damage each. Get clear.</p></article>",
        n(s.strike_max_range),
        n(s.strike_delay),
        s.strike_bombs,
        n(s.strike_length),
        n(s.strike_damage)
    );
    let _ = write!(
        h,
        "</div><p class=\"t-man-note\">Golden crates turn up every {}-{} s, flare crates every {} s. One rare part at a time; a new one replaces the old.</p></section>",
        n(s.rare_respawn_min),
        n(s.rare_respawn_max),
        n(s.strike_respawn)
    );

    // Pickups.
    let pk = &t.pickups;
    let _ = write!(
        h,
        "<section class=\"t-man-sec\" id=\"t-man-pickups\"><h3>On the ground</h3><div class=\"t-man-grid\">\
         <article class=\"t-man-card plate\"><header><div><h4>Scrap</h4><p>Steel, Powder, Volts, Fuel</p></div></header><p>Piles come back {} s after they're taken. Shot-off parts and wrecks spill theirs (it lasts {} s).</p></article>\
         <article class=\"t-man-card plate\"><header><div><h4>Wrench</h4><p>Repair kit</p></div></header><p>Patches {}% of your hull over {} s. Back {} s after it's taken.</p></article>\
         {}\
         </div></section>",
        n(pk.site_respawn),
        n(pk.dropped_lifetime),
        (pk.repair_heal * 100.0).round(),
        n(pk.repair_duration),
        n(pk.repair_respawn),
        powerups_html(&t),
    );

    // Chassis.
    h.push_str("<section class=\"t-man-sec\" id=\"t-man-chassis\"><h3>Chassis <span>Garage</span></h3><div class=\"t-man-grid\">");
    for (name, c, line) in [
        ("Scout", &t.chassis.scout, "Light and quick, thin skin. No front or skirt armour."),
        ("Brawler", &t.chassis.brawler, "The all-rounder."),
        ("Hauler", &t.chassis.hauler, "Slow turret, carries the most: fewer scrap per tier."),
    ] {
        let _ = write!(
            h,
            "<article class=\"t-man-card plate\"><header><div><h4>{name}</h4></div></header><p>{line}</p><p>{} hull, {} top speed, {} turret, {} scrap per tier.</p></article>",
            n(c.health),
            pct(c.speed_mult / c.mass_mult.powf(t.tank.mass_speed_exponent)),
            pct(c.traverse_mult / t.chassis.brawler.traverse_mult),
            c.scrap_per_tier
        );
    }
    h.push_str("</div></section>");
    h
}

/// The timed power-ups (glowing pickups at a few spots per map).
fn powerups_html(t: &Tuning) -> String {
    use arena_core::powerups::PowerUpKind;
    let p = &t.powerups;
    let mut h = String::new();
    for k in PowerUpKind::ALL {
        let what = match k {
            PowerUpKind::Overcharge => format!("Your hits do {} damage.", pct(p.overcharge_damage_mult)),
            PowerUpKind::Nitro => format!("{} speed, and the boost never runs dry.", pct(p.nitro_speed_mult)),
            PowerUpKind::Bubble => format!("A bubble soaks {} damage before it pops.", n(p.bubble_hp)),
            PowerUpKind::Gatling => format!("The main gun reloads in {}% of the time.", (p.gatling_reload_mult * 100.0).round()),
            PowerUpKind::Magnet => format!("Loose scrap within {} m drifts to you.", n(p.magnet_radius)),
            #[allow(unreachable_patterns)]
            _ => String::new(),
        };
        let _ = write!(
            h,
            "<article class=\"t-man-card plate t-man-glow\"><header><div><h4>{}</h4><p>Power-up, {} s</p></div></header><p>{}</p></article>",
            k.name(),
            n(k.duration(p)),
            esc(&what)
        );
    }
    let _ = write!(
        h,
        "<p class=\"t-man-note\">Power-ups glow at {} spots per map from {} s in, and come back {}-{} s after they're taken.</p>",
        p.spots,
        n(p.first_spawn),
        n(p.respawn_min),
        n(p.respawn_max)
    );
    h
}

/// "Your build" for the pause card: each track's tier and parts, combos and
/// the rare part, one line each.
pub fn build_html(tiers: [u8; 4], parts: &[(PartKind, u8)]) -> String {
    let t = Tuning::default();
    let mut h = String::new();
    for k in TRACKS {
        let tier = tiers[k.index()];
        let on: Vec<&(PartKind, u8)> =
            parts.iter().filter(|(p, _)| crate::game::names::track_of(*p) == Some(k.index()) && *p != PartKind::StubGun).collect();
        let _ = write!(h, "<div class=\"t-build-row\" data-kind=\"{}\">{}<b>{} {}</b>", key(k), icon(k), scrap_name(k), super::roman(tier));
        if on.is_empty() {
            let _ = write!(h, "<p>{}</p>", if tier == 0 { "Nothing yet: three scrap of a kind." } else { "Shot off." });
        }
        for (p, mk) in on {
            let _ = write!(h, "<p><i>{}{}</i> {}</p>", esc(part_name(*p)), if *mk >= 2 { " Mk II" } else { "" }, esc(&part_effect(&t, *p)));
        }
        h.push_str("</div>");
    }
    for (p, _) in parts.iter().filter(|(p, _)| COMBOS.iter().any(|c| c.2 == *p) || RARE_PARTS.contains(p)) {
        let _ = write!(h, "<div class=\"t-build-row t-build-extra\"><b>{}</b><p>{}</p></div>", esc(part_name(*p)), esc(&part_effect(&t, *p)));
    }
    h
}

/// A short map name for tight pickers ("Rust Belt").
pub fn short_map(m: MapId) -> &'static str {
    m.name().trim_start_matches("The ")
}

/// A line under a map's card.
#[allow(unreachable_patterns)]
pub fn map_blurb(m: MapId) -> &'static str {
    match m {
        MapId::Sink => "Dry lake bed, beached ships",
        MapId::Refinery => "Tanks that go boom",
        MapId::RustBelt => "Scrapyard canyon, a dry river",
        MapId::Canyon => "A deep winding gorge, three levels",
        _ => "Fresh off the survey",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manual_covers_everything() {
        let h = manual_html();
        for k in TRACKS {
            for p in branch_options(k) {
                assert!(h.contains(part_name(*p)), "{p:?}");
                assert!(!part_effect(&Tuning::default(), *p).is_empty(), "{p:?}");
            }
        }
        for p in RARE_PARTS.iter().chain(COMBOS.iter().map(|c| &c.2)) {
            assert!(h.contains(part_name(*p)) && !part_effect(&Tuning::default(), *p).is_empty(), "{p:?}");
        }
        assert!(h.contains("Air strike") && h.contains("Wrench") && h.contains("Hauler") && h.contains("Overcharge"));
        // The numbers are the game's.
        let t = Tuning::default();
        assert!(h.contains(&format!("{} damage", n(t.guns.big_bore.damage))));
        let b = build_html([0, 2, 1, 0], &[(PartKind::BigBore, 1), (PartKind::SensorMast, 1), (PartKind::Mortar, 1)]);
        assert!(b.contains("Big Bore") && b.contains("Sensor mast") && b.contains("Mortar") && b.contains("Powder II"));
    }
}
