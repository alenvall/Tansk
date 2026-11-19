//! Pickups that read as pickups: every one bobs and turns inside a soft
//! glowing bubble, over a light column and a ground ring, all in its kind's
//! colour, so salvage never gets lost among the props. Plus the iconic
//! models for the golden crates and the timed power-ups.

use arena_core::{PickupKind, PowerUpKind, ScrapKind};
use glam::{Mat4, Quat, Vec2, Vec3};

use crate::meshkit::{MeshBuilder, Paint};
use crate::shield::{Glass, GlassInstance, Shell, Style};
use crate::world::RenderWorld;

/// Pickups further than this from the eye get no aura.
const AURA_RANGE: f32 = 170.0;

/// Each kind's aura colour (linear, emitted).
pub fn colour(kind: PickupKind) -> Vec3 {
    match kind {
        PickupKind::Scrap(ScrapKind::Steel) => Vec3::new(0.75, 0.85, 1.0),
        PickupKind::Scrap(ScrapKind::Powder) => Vec3::new(1.0, 0.28, 0.1),
        PickupKind::Scrap(ScrapKind::Volts) => Vec3::new(0.25, 0.6, 1.0),
        PickupKind::Scrap(ScrapKind::Fuel) => Vec3::new(1.0, 0.72, 0.1),
        PickupKind::Repair => Vec3::new(0.3, 1.0, 0.4),
        PickupKind::Rare(_) => Vec3::new(1.0, 0.7, 0.2) * 1.3,
        PickupKind::AirStrike => Vec3::new(1.0, 0.22, 0.1) * 1.2,
        PickupKind::PowerUp(p) => powerup_colour(p) * 1.4,
    }
}

pub fn powerup_colour(p: PowerUpKind) -> Vec3 {
    match p {
        PowerUpKind::Overcharge => Vec3::new(1.0, 0.18, 0.12),
        PowerUpKind::Nitro => Vec3::new(0.2, 0.7, 1.0),
        PowerUpKind::Bubble => crate::shield::SHIELD_AMBER,
        PowerUpKind::Gatling => Vec3::new(1.0, 0.85, 0.25),
        PowerUpKind::Magnet => Vec3::new(0.85, 0.3, 1.0),
    }
}

/// How the pickup is drawn: the mesh key and its builder.
pub fn mesh_for(kind: PickupKind) -> (&'static str, fn() -> MeshBuilder) {
    match kind {
        PickupKind::Scrap(ScrapKind::Steel) => ("pickup:0", || crate::props::pickup(0)),
        PickupKind::Scrap(ScrapKind::Powder) => ("pickup:1", || crate::props::pickup(1)),
        PickupKind::Scrap(ScrapKind::Volts) => ("pickup:2", || crate::props::pickup(2)),
        PickupKind::Scrap(ScrapKind::Fuel) => ("pickup:3", || crate::props::pickup(3)),
        PickupKind::Repair => ("pickup:4", || crate::props::pickup(4)),
        PickupKind::Rare(_) => ("pickup:rare", golden_crate),
        PickupKind::AirStrike => ("pickup:flare", flare_crate),
        PickupKind::PowerUp(PowerUpKind::Overcharge) => ("powerup:overcharge", overcharge),
        PickupKind::PowerUp(PowerUpKind::Nitro) => ("powerup:nitro", nitro),
        PickupKind::PowerUp(PowerUpKind::Bubble) => ("powerup:bubble", bubble),
        PickupKind::PowerUp(PowerUpKind::Gatling) => ("powerup:gatling", gatling),
        PickupKind::PowerUp(PowerUpKind::Magnet) => ("powerup:magnet", magnet),
    }
}

/// Power-ups are bigger and float higher (rare, worth the detour).
pub fn size_of(kind: PickupKind) -> f32 {
    match kind {
        PickupKind::PowerUp(_) => 1.5,
        PickupKind::Rare(_) | PickupKind::AirStrike => 1.2,
        _ => 1.0,
    }
}

/// The bob over the ground and the turn of a pickup at `time`.
pub fn pose(id: u16, pos: Vec3, kind: PickupKind, time: f32) -> Mat4 {
    let s = size_of(kind);
    let bob = (time * 2.0 + id as f32).sin() * 0.12 * s + 0.3 * s;
    Mat4::from_scale_rotation_translation(Vec3::splat(s), Quat::from_rotation_y(time * 1.2 + id as f32), pos + Vec3::Y * bob)
}

/// Queues every pickup's aura: the bubble round it, the column and the ring.
pub fn auras(glass: &mut Glass, world: &RenderWorld, eye: Vec3, ground: &dyn Fn(f32, f32) -> f32) {
    for p in &world.pickups {
        let d = p.pos.distance(eye);
        if d > AURA_RANGE {
            continue;
        }
        // Faint up close, firmer far away, gone at the edge of range.
        let fade = (1.0 - ((d - AURA_RANGE * 0.8) / (AURA_RANGE * 0.2)).clamp(0.0, 1.0)) * (0.55 + 0.45 * (d / 40.0).min(1.0));
        let c = colour(p.kind);
        let s = size_of(p.kind);
        let seed = p.id as f32 * 0.137;
        let item = pose(p.id, p.pos, p.kind, world.time).transform_point3(Vec3::Y * 0.35);
        let g = ground(p.pos.x, p.pos.z).min(p.pos.y);
        let breathe = 1.0 + 0.04 * (world.time * 2.6 + seed * 6.0).sin();
        let bubble = Mat4::from_scale_rotation_translation(Vec3::splat(0.95 * s * breathe), Quat::from_rotation_y(world.time * 0.5 + seed), item);
        glass.push(Shell::Sphere, GlassInstance::new(bubble, Style::Aura, c * 1.6, fade, 1.0, seed));
        let column = Mat4::from_scale_rotation_translation(Vec3::new(0.8 * s, 4.5 * s, 0.8 * s), Quat::IDENTITY, Vec3::new(p.pos.x, g, p.pos.z));
        glass.push(Shell::Column, GlassInstance::new(column, Style::Beam, c, fade, 1.0, seed));
        let ring = Mat4::from_scale_rotation_translation(Vec3::splat(1.5 * s), Quat::from_rotation_y(world.time * 0.3), Vec3::new(p.pos.x, g + 0.06, p.pos.z));
        glass.push(Shell::Ring, GlassInstance::new(ring, Style::GroundRing, c * 1.5, fade, 1.0, seed));
    }
}

fn at(p: Vec3) -> Mat4 {
    Mat4::from_translation(p)
}

fn along_x(p: Vec3) -> Mat4 {
    Mat4::from_rotation_translation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2), p)
}

/// A golden crate (a rare part inside): riveted gold-painted steel.
pub fn golden_crate() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let gold = Paint::painted(0xe0a828, 0.25);
    let trim = Paint::steel(0x3a3632, 0.4);
    m.bevel_box(Vec3::new(0.42, 0.3, 0.32), 0.05, 2, gold, at(Vec3::Y * 0.3));
    for x in [-0.36f32, 0.36] {
        m.bevel_box(Vec3::new(0.04, 0.32, 0.34), 0.015, 1, trim, at(Vec3::new(x, 0.3, 0.0)));
    }
    m.bevel_box(Vec3::new(0.12, 0.08, 0.02), 0.01, 1, Paint::glow(0xffd070, 2.0), at(Vec3::new(0.0, 0.34, 0.33)));
    m
}

/// A flare crate (one air strike): red, a flare tube strapped on top.
pub fn flare_crate() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.bevel_box(Vec3::new(0.42, 0.26, 0.3), 0.05, 2, Paint::painted(0xb8281c, 0.35), at(Vec3::Y * 0.26));
    m.bevel_box(Vec3::new(0.43, 0.04, 0.31), 0.015, 1, Paint::with(crate::meshkit::Material::Hazard, 0xe8b419, 0.6), at(Vec3::Y * 0.42));
    m.cylinder(0.07, 0.35, 0.02, 1, 8, Paint::painted(0xe8e0d0, 0.2), along_x(Vec3::new(0.0, 0.6, 0.0)));
    m.cylinder(0.075, 0.06, 0.02, 1, 8, Paint::glow(0xff4020, 3.0), along_x(Vec3::new(0.38, 0.6, 0.0)));
    m
}

/// Overcharge: a powder keg with a fizzing red fuse cap.
fn overcharge() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let profile = [
        (0.0f32, 0.0f32, Vec2::NEG_Y),
        (0.26, 0.0, Vec2::new(0.4, -0.9)),
        (0.32, 0.3, Vec2::X),
        (0.26, 0.6, Vec2::new(0.4, 0.9)),
        (0.0, 0.6, Vec2::Y),
    ];
    m.lathe(&profile, 12, true, Paint::with(crate::meshkit::Material::Wood, 0x7a4a26, 0.7), Mat4::IDENTITY);
    for y in [0.1f32, 0.5] {
        m.cylinder(0.3, 0.025, 0.01, 1, 12, Paint::steel(0x2a2725, 0.4), at(Vec3::Y * y));
    }
    m.cylinder(0.08, 0.06, 0.02, 1, 8, Paint::glow(0xff3018, 3.0), at(Vec3::Y * 0.66));
    m
}

/// Nitro: a blue pressure bottle with a brass valve.
fn nitro() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.cylinder(0.18, 0.32, 0.12, 2, 12, Paint::painted(0x2a6ad0, 0.2), at(Vec3::Y * 0.34));
    m.cylinder(0.185, 0.03, 0.01, 1, 12, Paint::glow(0x60c0ff, 2.0), at(Vec3::Y * 0.5));
    m.cylinder(0.05, 0.08, 0.02, 1, 8, Paint::steel(0xc89a40, 0.3), at(Vec3::Y * 0.74));
    m.bevel_box(Vec3::new(0.12, 0.025, 0.025), 0.01, 1, Paint::painted(0xc8281c, 0.2), at(Vec3::Y * 0.82));
    m
}

/// Bubble: a little field generator: a steel drum with a hot amber dome.
fn bubble() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.cylinder(0.3, 0.12, 0.04, 2, 10, Paint::steel(0x4a4440, 0.4), at(Vec3::Y * 0.12));
    let dome = [(0.24f32, 0.0f32, Vec2::X), (0.2, 0.14, Vec2::new(0.7, 0.7)), (0.0, 0.24, Vec2::Y)];
    m.lathe(&dome, 10, true, Paint::glow(0xff9030, 2.2), at(Vec3::Y * 0.24));
    for k in 0..3 {
        let a = k as f32 * 2.094;
        m.bevel_box(Vec3::new(0.05, 0.16, 0.03), 0.01, 1, Paint::steel(0x2a2725, 0.4), Mat4::from_rotation_translation(Quat::from_rotation_y(a), Vec3::new(0.3 * a.cos(), 0.2, -0.3 * a.sin())));
    }
    m
}

/// Gatling: a six-barrel cluster on a breech block.
fn gatling() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.bevel_box(Vec3::new(0.16, 0.16, 0.16), 0.03, 2, Paint::painted(0x3a3e34, 0.4), at(Vec3::new(-0.3, 0.3, 0.0)));
    for k in 0..6 {
        let a = k as f32 * std::f32::consts::TAU / 6.0;
        m.cylinder(0.04, 0.32, 0.01, 1, 6, Paint::steel(0x54504a, 0.35), along_x(Vec3::new(0.15, 0.3 + 0.1 * a.cos(), 0.1 * a.sin())));
    }
    m.cylinder(0.15, 0.03, 0.01, 1, 10, Paint::glow(0xffd040, 2.0), along_x(Vec3::new(0.4, 0.3, 0.0)));
    m
}

/// Magnet: a red horseshoe with bright steel tips.
fn magnet() -> MeshBuilder {
    let mut m = MeshBuilder::new();
    let red = Paint::painted(0xc8281c, 0.2);
    for k in 0..7 {
        let a = std::f32::consts::PI * k as f32 / 6.0;
        let p = Vec3::new(0.24 * a.cos(), 0.3 + 0.24 * a.sin(), 0.0);
        m.bevel_box(Vec3::new(0.07, 0.07, 0.08), 0.02, 1, red, Mat4::from_rotation_translation(Quat::from_rotation_z(a), p));
    }
    for x in [-0.24f32, 0.24] {
        m.bevel_box(Vec3::new(0.07, 0.12, 0.08), 0.02, 1, red, at(Vec3::new(x, 0.2, 0.0)));
        m.bevel_box(Vec3::new(0.075, 0.05, 0.085), 0.015, 1, Paint::glow(0xd8c0ff, 1.6), at(Vec3::new(x, 0.06, 0.0)));
    }
    m
}

/// Tanks under a timed power-up glow faintly in its colour: a soft halo
/// and a ring on the sand round them (the Bubble is the shield itself).
pub fn buff_glows(glass: &mut Glass, world: &RenderWorld) {
    for &(player, kind) in &world.buffs {
        if kind == PowerUpKind::Bubble {
            continue;
        }
        let Some(t) = world.tanks.iter().find(|t| t.player == player && t.alive) else { continue };
        let c = powerup_colour(kind);
        let seed = player as f32 * 0.29;
        let pulse = 0.75 + 0.25 * (world.time * 4.0 + seed * 6.0).sin();
        let halo = Mat4::from_scale_rotation_translation(Vec3::new(4.2, 2.4, 3.4), t.rot, t.pos + Vec3::Y * 0.3);
        glass.push(Shell::Sphere, GlassInstance::new(halo, Style::Aura, c * 1.2, 0.4 * pulse, 1.0, seed));
        let ring = Mat4::from_scale_rotation_translation(Vec3::splat(4.6), Quat::from_rotation_y(world.time * 0.8), t.pos - Vec3::Y * (crate::tank::HULL_Y - 0.08));
        glass.push(Shell::Ring, GlassInstance::new(ring, Style::GroundRing, c * 1.5, 0.8, 1.0, seed));
    }
}
