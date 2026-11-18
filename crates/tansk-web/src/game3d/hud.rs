//! The 3D game's GUI, drawn on the 2D canvas over the WebGL view: the
//! original's lifebar, console, info screen and scoreboard, plus a
//! crosshair, weapon panel, minimap and name tags.

use tansk3d_sim::proto::{color_name, Phase, PlayerView, Power, Settings, View3d, Weapon};
use tansk3d_sim::terrain::{self, ARENA_D, ARENA_W, ZONE_RADIUS};
use tansk_sim::game::MAX_SHIELD_HEALTH;
use tansk_sim::V2;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d as Ctx, HtmlCanvasElement};

use super::render::{Heights, Scene};
use crate::assets::Images;
use crate::render::{draw_lifebar, hit, round_rect, slick_color, ConsoleMsg, LINK_BOX, MENU_BUTTON, RESTART_BUTTON, SCOREBOARD, SOUND_ICON, START_BUTTON};

pub const FONT: &str = "14px Verdana, sans-serif";
const MAP: (f64, f64, f64, f64) = (10.0, 10.0, 192.0, 144.0);

pub struct Hud3d<'a> {
    pub now: f64,
    pub me: Option<u32>,
    pub settings: Settings,
    pub code: Option<&'a str>,
    pub link: Option<&'a str>,
    pub host: bool,
    pub online: bool,
    pub connected: bool,
    pub show_info: bool,
    pub console: &'a [ConsoleMsg],
    pub chat_open: bool,
    pub mouse: V2,
    pub mouse_down: bool,
    pub sound_icon: &'static str,
    pub copied: bool,
    /// Looking from your turret (crosshair and gun panel shown).
    pub first_person: bool,
    /// The game wants the pointer and doesn't have it: "Click to play".
    pub click_to_play: bool,
    pub aim_pitch: f32,
    pub cam_yaw: f32,
    pub range: Option<f32>,
    pub scores: bool,
    pub debug: Option<Vec<String>>,
}

fn text(ctx: &Ctx, s: &str, x: f64, y: f64, color: &str) {
    ctx.set_fill_style_str(color);
    let _ = ctx.fill_text(s, x, y);
}

/// Text with a dark outline, readable over the 3D scene.
fn otext(ctx: &Ctx, s: &str, x: f64, y: f64, color: &str) {
    ctx.set_stroke_style_str("rgba(0,0,0,0.75)");
    ctx.set_line_width(3.0);
    ctx.set_line_join("round");
    let _ = ctx.stroke_text(s, x, y);
    ctx.set_line_width(1.0);
    text(ctx, s, x, y, color);
}

fn centered(ctx: &Ctx, s: &str, cx: f64, y: f64, color: &str) {
    let w = ctx.measure_text(s).map(|m| m.width()).unwrap_or(0.0);
    otext(ctx, s, cx - w / 2.0, y, color);
}

fn panel(ctx: &Ctx, x: f64, y: f64, w: f64, h: f64, color: &str) {
    ctx.set_fill_style_str(color);
    round_rect(ctx, x, y, w, h, 8.0);
    ctx.fill();
}

pub fn weapon_name(w: Weapon) -> &'static str {
    match w {
        Weapon::Default => "Cannon",
        Weapon::Bounce => "Bounce gun",
        Weapon::Flamethrower => "Flamethrower",
        Weapon::Shockwave => "Shockwave",
        Weapon::Shotgun => "Shotgun",
        Weapon::Slowspeedy => "Slowspeedy",
    }
}

fn power_name(p: Power) -> (&'static str, &'static str) {
    match p {
        Power::Health => ("Health", "health_powerup"),
        Power::Shield => ("Shield", "shield_powerup"),
        Power::Speed => ("Speed", "speed_powerup"),
        Power::FireRate => ("Fire rate", "firerate_powerup"),
        Power::Damage => ("Damage", "damage_powerup"),
    }
}

/// The minimap's terrain, shaded once.
pub fn minimap_canvas(h: &Heights) -> Option<HtmlCanvasElement> {
    let doc = web_sys::window()?.document()?;
    let c: HtmlCanvasElement = doc.create_element("canvas").ok()?.dyn_into().ok()?;
    let (w, d) = (128u32, 96u32);
    c.set_width(w);
    c.set_height(d);
    let ctx: Ctx = c.get_context("2d").ok()??.dyn_into().ok()?;
    let light = [-0.45f32, 0.85, -0.35];
    for j in 0..d {
        for i in 0..w {
            let (x, z) = ((i as f32 + 0.5) * 8.0, (j as f32 + 0.5) * 8.0);
            let n = h.normal(x, z);
            let lam = (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]).max(0.0);
            let ht = ((h.height(x, z) - 10.0) / 90.0).clamp(0.0, 1.0);
            let k = 0.5 + 0.55 * lam;
            let (r, g, b) = (190.0 + 40.0 * ht, 172.0 + 40.0 * ht, 132.0 + 50.0 * ht);
            ctx.set_fill_style_str(&format!("rgb({},{},{})", (r * k) as u8, (g * k) as u8, (b * k) as u8));
            ctx.fill_rect(i as f64, j as f64, 1.0, 1.0);
        }
    }
    Some(c)
}

pub fn draw(ctx: &Ctx, imgs: &Images, scene: &Scene, minimap: Option<&HtmlCanvasElement>, v: Option<&View3d>, hud: &Hud3d) {
    ctx.set_global_alpha(1.0);
    ctx.clear_rect(0.0, 0.0, 1024.0, 768.0);
    ctx.set_text_baseline("top");
    ctx.set_font(FONT);
    let players: &[PlayerView] = v.map(|v| v.players.as_slice()).unwrap_or(&[]);
    let me = hud.me.and_then(|id| players.iter().find(|p| p.id == id));
    let my_tank = v.and_then(|v| v.tanks.iter().find(|t| Some(t.player) == hud.me));

    // Name tags and health bars over the other tanks.
    if let Some(v) = v {
        let mut tags: Vec<(f64, f64, f32, &str, u8, f32)> = Vec::new();
        for t in &v.tanks {
            if Some(t.player) == hud.me && hud.first_person {
                continue;
            }
            let Some((x, y, dist)) = scene.project([t.pos[0], t.pos[1] + 66.0, t.pos[2]]) else { continue };
            if dist > 1400.0 || !(-50.0..1074.0).contains(&x) || !(-50.0..818.0).contains(&y) {
                continue;
            }
            let name = players.iter().find(|p| p.id == t.player).map(|p| p.name.as_str()).unwrap_or("?");
            tags.push((x, y, dist, name, t.color, t.health));
        }
        tags.sort_by(|a, b| b.2.total_cmp(&a.2));
        for (x, y, dist, name, color, health) in tags {
            let a = (1.0 - (dist as f64 - 700.0) / 700.0).clamp(0.35, 1.0);
            ctx.set_global_alpha(a);
            ctx.set_font("bold 13px Verdana, sans-serif");
            centered(ctx, name, x, y - 22.0, slick_color(color));
            let (bw, bh) = (46.0, 6.0);
            ctx.set_fill_style_str("rgba(0,0,0,0.7)");
            ctx.fill_rect(x - bw / 2.0 - 1.0, y - 5.0, bw + 2.0, bh + 2.0);
            ctx.set_fill_style_str("#ff2a1a");
            ctx.fill_rect(x - bw / 2.0, y - 4.0, bw * (health as f64 / 100.0).clamp(0.0, 1.0), bh);
        }
        ctx.set_global_alpha(1.0);
        ctx.set_font(FONT);
    }

    if hud.first_person && !hud.show_info {
        draw_crosshair(ctx, hud);
    }

    draw_console(ctx, hud);
    if let Some(m) = minimap {
        draw_minimap(ctx, imgs, m, v, hud);
    }

    if let Some(v) = v {
        if v.phase == Phase::Countdown && v.delay_ms > 0 && !hud.show_info {
            ctx.set_font("bold 22px Verdana, sans-serif");
            centered(ctx, &format!("Round starts in: {} seconds!", v.delay_ms / 1000 + 1), 512.0, 120.0, "#fff");
            ctx.set_font(FONT);
        }
        if v.phase == Phase::Playing && v.round_ms_left > 0 {
            let s = v.round_ms_left / 1000;
            otext(ctx, &format!("{}:{:02}", s / 60, s % 60), 930.0, 16.0, "#fff");
        }
        if matches!(v.phase, Phase::Playing | Phase::Countdown) && my_tank.is_none() && me.is_some() && !hud.show_info {
            ctx.set_font("bold 22px Verdana, sans-serif");
            let msg = if me.is_some_and(|m| m.eliminated) { "Eliminated - spectating" } else { "Destroyed! Waiting to respawn..." };
            if v.phase == Phase::Playing {
                centered(ctx, msg, 512.0, 300.0, "#ff4030");
            }
            ctx.set_font(FONT);
        }
    }

    if hud.show_info {
        draw_info(ctx, players, me, hud);
    }

    if let (Some(me), Some(t)) = (me, my_tank) {
        let shield = t.shield.filter(|s| *s <= 100.0).map(|s| s as f64 / MAX_SHIELD_HEALTH);
        draw_lifebar(ctx, t.health as f64 / 100.0, shield);
        if hud.settings.koth {
            let (x, y) = (412.0, 60.0);
            let frac = (me.score as f64 / hud.settings.score_limit.max(1) as f64).min(1.0);
            ctx.set_fill_style_str("rgba(0,0,0,0.5)");
            round_rect(ctx, x, y, 200.0, 10.0, 5.0);
            ctx.fill();
            ctx.set_fill_style_str("#00ff00");
            round_rect(ctx, x, y, 200.0 * frac, 10.0, 5.0);
            ctx.fill();
            ctx.set_stroke_style_str("#000");
            round_rect(ctx, x, y, 200.0, 10.0, 5.0);
            ctx.stroke();
            let c = terrain::zone_center();
            let inside = ((t.pos[0] - c[0]).powi(2) + (t.pos[2] - c[2]).powi(2)).sqrt() < ZONE_RADIUS;
            ctx.set_font("12px Verdana, sans-serif");
            otext(ctx, &format!("Hill {}/{}", me.score, hud.settings.score_limit), 620.0, 57.0, if inside { "#7fff5f" } else { "#fff" });
            ctx.set_font(FONT);
        }
        if hud.first_person {
            draw_gun_panel(ctx, imgs, t.weapon, t.ammo, t.power);
        }
    }

    if hud.online && hud.host && v.is_some_and(|v| v.phase == Phase::Lobby) {
        let (x, y, w, h) = START_BUTTON;
        let hover = hit(START_BUTTON, hud.mouse);
        ctx.set_fill_style_str(if hover { "rgba(255,255,255,0.95)" } else { "rgba(255,255,255,0.7)" });
        ctx.fill_rect(x, y, w, h);
        ctx.set_stroke_style_str("#ff0000");
        ctx.stroke_rect(x + 0.5, y + 0.5, w, h);
        text(ctx, "Start!", x + 45.0, y + 4.0, "#ff0000");
    }

    if hud.online && !hud.connected {
        ctx.set_font("bold 22px Verdana, sans-serif");
        centered(ctx, "Connecting...", 512.0, 360.0, "#fff");
        ctx.set_font(FONT);
    }

    let game_over = v.is_some_and(|v| v.phase == Phase::GameOver);
    if game_over || (hud.scores && !hud.show_info) {
        draw_scoreboard(ctx, imgs, players, hud, game_over);
    } else if hud.click_to_play {
        ctx.set_fill_style_str("rgba(0,0,0,0.45)");
        ctx.fill_rect(0.0, 330.0, 1024.0, 92.0);
        ctx.set_font("bold 28px Verdana, sans-serif");
        centered(ctx, "Click to play", 512.0, 342.0, "#fff");
        ctx.set_font(FONT);
        centered(ctx, "WASD drive  -  mouse aim  -  click fire  -  Enter chat  -  Tab scores  -  Esc menu", 512.0, 388.0, "#ddd");
    }

    if let Some(img) = imgs.get(hud.sound_icon) {
        let _ = ctx.draw_image_with_html_image_element(&img, SOUND_ICON.0, SOUND_ICON.1);
    }
    if let Some(lines) = &hud.debug {
        for (i, l) in lines.iter().enumerate() {
            otext(ctx, l, 18.0, 280.0 + 20.0 * i as f64, "#fff");
        }
    }
}

fn draw_crosshair(ctx: &Ctx, hud: &Hud3d) {
    let (cx, cy) = (512.0, 384.0);
    let seg = |x0: f64, y0: f64, x1: f64, y1: f64| {
        ctx.begin_path();
        ctx.move_to(x0, y0);
        ctx.line_to(x1, y1);
        ctx.stroke();
    };
    for (color, width) in [("rgba(0,0,0,0.6)", 4.0), ("#fff", 2.0)] {
        ctx.set_stroke_style_str(color);
        ctx.set_line_width(width);
        seg(cx - 16.0, cy, cx - 6.0, cy);
        seg(cx + 6.0, cy, cx + 16.0, cy);
        seg(cx, cy - 16.0, cx, cy - 6.0);
        seg(cx, cy + 6.0, cx, cy + 16.0);
    }
    ctx.set_fill_style_str("#fff");
    ctx.fill_rect(cx - 1.0, cy - 1.0, 2.0, 2.0);
    ctx.set_line_width(1.0);
    // The barrel's elevation, and where a shell would come down.
    ctx.set_font("12px Verdana, sans-serif");
    otext(ctx, &format!("{:+.1}\u{b0}", hud.aim_pitch), cx + 22.0, cy + 6.0, "#fff");
    if let Some(r) = hud.range {
        otext(ctx, &format!("{:.0} m", r / 10.0), cx + 22.0, cy + 22.0, "#ffd080");
    }
    // A compass heading over the crosshair.
    let heading = ((hud.cam_yaw + 90.0).rem_euclid(360.0)).round() as i32 % 360;
    centered(ctx, &format!("{heading:03}"), cx, 16.0 + 70.0, "rgba(255,255,255,0.8)");
    ctx.set_font(FONT);
}

fn draw_gun_panel(ctx: &Ctx, imgs: &Images, weapon: Weapon, ammo: i32, power: Option<Power>) {
    let (x, y, w, h) = (804.0, 700.0, 210.0, 58.0);
    panel(ctx, x, y, w, h, "rgba(0,0,0,0.45)");
    if let Some(img) = imgs.get(tansk3d_sim::proto::PickupKind::Weapon(weapon).sprite()) {
        let _ = ctx.draw_image_with_html_image_element_and_dw_and_dh(&img, x + 10.0, y + 9.0, 40.0, 40.0);
    }
    ctx.set_font("bold 15px Verdana, sans-serif");
    otext(ctx, weapon_name(weapon), x + 60.0, y + 10.0, "#fff");
    ctx.set_font(FONT);
    let ammo_s = if weapon == Weapon::Default || ammo < 0 { "\u{221e}".to_string() } else { ammo.to_string() };
    otext(ctx, &format!("Ammo: {ammo_s}"), x + 60.0, y + 32.0, "#ffd080");
    if let Some(p) = power {
        let (name, sprite) = power_name(p);
        let (px, py) = (x - 150.0, y);
        panel(ctx, px, py, 140.0, h, "rgba(0,0,0,0.45)");
        if let Some(img) = imgs.get(sprite) {
            let _ = ctx.draw_image_with_html_image_element_and_dw_and_dh(&img, px + 10.0, py + 11.0, 35.0, 35.0);
        }
        otext(ctx, name, px + 52.0, py + 21.0, "#9fff9f");
    }
}

fn draw_minimap(ctx: &Ctx, imgs: &Images, m: &HtmlCanvasElement, v: Option<&View3d>, hud: &Hud3d) {
    let (x, y, w, h) = MAP;
    let (sx, sy) = (w / ARENA_W as f64, h / ARENA_D as f64);
    ctx.set_global_alpha(0.9);
    let _ = ctx.draw_image_with_html_canvas_element_and_dw_and_dh(m, x, y, w, h);
    ctx.set_global_alpha(1.0);
    ctx.set_fill_style_str("#333");
    for wall in terrain::walls() {
        ctx.fill_rect(x + wall.min[0] as f64 * sx, y + wall.min[2] as f64 * sy, ((wall.max[0] - wall.min[0]) as f64 * sx).max(2.0), ((wall.max[2] - wall.min[2]) as f64 * sy).max(2.0));
    }
    ctx.set_stroke_style_str("#000");
    ctx.set_line_width(2.0);
    ctx.stroke_rect(x, y, w, h);
    ctx.set_line_width(1.0);
    let Some(v) = v else { return };
    if let Some(z) = v.zone {
        ctx.set_fill_style_str("rgba(255,210,40,0.45)");
        ctx.begin_path();
        let _ = ctx.arc(x + z[0] as f64 * sx, y + z[2] as f64 * sy, ZONE_RADIUS as f64 * sx, 0.0, std::f64::consts::TAU);
        ctx.fill();
    }
    for p in &v.pickups {
        if let Some(img) = imgs.get(p.kind.sprite()) {
            let _ = ctx.draw_image_with_html_image_element_and_dw_and_dh(&img, x + p.pos[0] as f64 * sx - 5.0, y + p.pos[2] as f64 * sy - 5.0, 10.0, 10.0);
        }
    }
    for t in &v.tanks {
        let mine = Some(t.player) == hud.me;
        let (tx, ty) = (x + t.pos[0] as f64 * sx, y + t.pos[2] as f64 * sy);
        ctx.save();
        let _ = ctx.translate(tx, ty);
        if mine && hud.first_person {
            // Your view cone.
            let _ = ctx.rotate((hud.cam_yaw as f64).to_radians());
            ctx.set_fill_style_str("rgba(255,255,255,0.25)");
            ctx.begin_path();
            ctx.move_to(0.0, 0.0);
            let _ = ctx.arc(0.0, 0.0, 40.0, -0.6, 0.6);
            ctx.close_path();
            ctx.fill();
            let _ = ctx.rotate(((t.yaw - hud.cam_yaw) as f64).to_radians());
        } else {
            let _ = ctx.rotate((t.yaw as f64).to_radians());
        }
        ctx.set_fill_style_str(slick_color(t.color));
        ctx.set_stroke_style_str(if mine { "#fff" } else { "#000" });
        ctx.begin_path();
        ctx.move_to(7.0, 0.0);
        ctx.line_to(-5.0, -5.0);
        ctx.line_to(-2.0, 0.0);
        ctx.line_to(-5.0, 5.0);
        ctx.close_path();
        ctx.fill();
        ctx.stroke();
        ctx.restore();
    }
}

/// ClientState.renderGUI's "waiting for the host" screen, on a panel.
fn draw_info(ctx: &Ctx, players: &[PlayerView], me: Option<&PlayerView>, hud: &Hud3d) {
    let s = hud.settings;
    let black = "#000";
    ctx.set_fill_style_str("rgba(212,212,212,0.88)");
    round_rect(ctx, 270.0, 80.0, 484.0, 520.0, 10.0);
    ctx.fill();
    ctx.set_stroke_style_str("#000");
    ctx.set_line_width(3.0);
    round_rect(ctx, 270.0, 80.0, 484.0, 520.0, 10.0);
    ctx.stroke();
    ctx.set_line_width(1.0);
    if let Some(code) = hud.code {
        ctx.set_font("bold 22px Verdana, sans-serif");
        text(ctx, &format!("Game code: {code}"), 400.0, 96.0, black);
        ctx.set_font(FONT);
        if let Some(link) = hud.link {
            let (x, y, w, h) = LINK_BOX;
            ctx.set_fill_style_str(if hit(LINK_BOX, hud.mouse) { "rgba(255,255,255,0.95)" } else { "rgba(255,255,255,0.6)" });
            ctx.fill_rect(x, y, w, h);
            let label = if hud.copied { "Link copied!".to_string() } else { format!("{link}  (click to copy)") };
            text(ctx, &label, x + 8.0, y + 4.0, black);
        }
    }
    if hud.host {
        text(ctx, "You are the host: press Start! when everyone is in.", 330.0, 175.0, black);
    } else {
        text(ctx, "Waiting for host to start the game...", 350.0, 175.0, black);
    }
    if let Some(me) = me {
        text(ctx, "You are:", 450.0, 195.0, black);
        ctx.set_font("bold 14px Verdana, sans-serif");
        otext(ctx, &color_name(me.color).to_uppercase(), 528.0, 195.0, slick_color(me.color));
        ctx.set_font(FONT);
    }
    if s.koth {
        text(ctx, "KING OF THE HILL", 435.0, 235.0, black);
        text(ctx, "Score points by holding the hilltop zone!", 330.0, 255.0, black);
    } else {
        text(ctx, "STANDARD", 465.0, 235.0, black);
        text(ctx, "The last one alive gets the most points!", 340.0, 255.0, black);
    }
    text(ctx, &format!("Scorelimit: {}", s.score_limit), 445.0, 315.0, black);
    text(ctx, &format!("Rounds: {}", s.rounds), 445.0, 335.0, black);
    text(ctx, &format!("Player lives: {}", s.player_lives()), 445.0, 355.0, black);
    text(ctx, &format!("Spawn time: {}", s.spawn_time_ms / 1000), 445.0, 375.0, black);
    text(ctx, &format!("Round time: {}", s.round_time_ms / 1000), 445.0, 395.0, black);
    text(ctx, &format!("Game time: {}", s.game_time_ms / 1000), 445.0, 415.0, black);
    text(ctx, &format!("Players: {}/4", players.len()), 445.0, 455.0, black);
    ctx.set_font("bold 14px Verdana, sans-serif");
    for (i, p) in players.iter().enumerate() {
        otext(ctx, &p.name, 445.0, 475.0 + 20.0 * i as f64, slick_color(p.color));
    }
    ctx.set_font(FONT);
    text(ctx, "Press 'Enter' to chat...", 420.0, 575.0, black);
}

/// Console.renderMessages: the latest lines, hidden after a while.
fn draw_console(ctx: &Ctx, hud: &Hud3d) {
    const ROWS: usize = 9;
    const HIDE_MS: f64 = 10_000.0;
    let start = hud.console.len().saturating_sub(ROWS);
    for (i, m) in hud.console[start..].iter().enumerate() {
        let age = hud.now - m.at;
        let alpha = if hud.chat_open { 1.0 } else { (1.0 - (age - HIDE_MS) / 1000.0).clamp(0.0, 1.0) };
        if alpha <= 0.0 {
            continue;
        }
        let rows_from_bottom = (hud.console.len() - start - i) as f64;
        let y = 725.0 - 20.0 * rows_from_bottom;
        ctx.set_global_alpha(alpha);
        otext(ctx, &format!("[{}]", m.stamp), 14.0, y, "#fff");
        let color = if m.color == "#000000" { "#fff" } else { m.color };
        otext(ctx, &m.text, 74.0, y, color);
    }
    ctx.set_global_alpha(1.0);
}

fn draw_scoreboard(ctx: &Ctx, imgs: &Images, players: &[PlayerView], hud: &Hud3d, buttons: bool) {
    let (x, y, w, h) = SCOREBOARD;
    ctx.set_fill_style_str("rgba(212,212,212,0.95)");
    ctx.fill_rect(x, y, w, h);
    ctx.set_stroke_style_str("#000");
    ctx.set_line_width(7.0);
    ctx.stroke_rect(x, y, w, h);
    ctx.set_line_width(1.0);
    ctx.set_font("22px Verdana, sans-serif");
    let mut sorted: Vec<&PlayerView> = players.iter().collect();
    sorted.sort_by(|a, b| b.score.cmp(&a.score));
    for (i, p) in sorted.iter().enumerate() {
        let ty = y + 10.0 + 30.0 * i as f64;
        otext(ctx, &format!("{}: {}", p.name, p.score), x + 20.0, ty, slick_color(p.color));
        if !hud.settings.koth && !buttons {
            ctx.set_font(FONT);
            text(ctx, &format!("lives {}", p.lives.max(0)), x + 260.0, ty + 6.0, "#333");
            ctx.set_font("22px Verdana, sans-serif");
        }
    }
    ctx.set_font(FONT);
    if buttons {
        draw_button(ctx, imgs, "button_menu", MENU_BUTTON, hud);
        if !hud.online || hud.host {
            draw_button(ctx, imgs, "button_restart", RESTART_BUTTON, hud);
        }
    }
}

fn draw_button(ctx: &Ctx, imgs: &Images, name: &str, r: (f64, f64, f64, f64), hud: &Hud3d) {
    let state = if hit(r, hud.mouse) {
        if hud.mouse_down {
            "_pressed"
        } else {
            "_hover"
        }
    } else {
        ""
    };
    if let Some(img) = imgs.get(&format!("{name}{state}")).or_else(|| imgs.get(name)) {
        let _ = ctx.draw_image_with_html_image_element(&img, r.0, r.1);
    }
}
