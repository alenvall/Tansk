//! Drawing a frame: the original's four render layers, then its GUI
//! (Lifebar, the king-of-the-hill bar, Console, Scoreboard, SoundSwitch).

use tansk_sim::game::{MAX_SHIELD_HEALTH, ZONE_POS};
use tansk_sim::map::{self, MapEntity, HORIZONTAL_WALL};
use tansk_sim::proto::{color_name, GameView, Phase, PickupKind, PlayerView, Settings, TankView};
use tansk_sim::V2;
use web_sys::CanvasRenderingContext2d as Ctx;

use crate::assets::Images;

pub const W: f64 = 1024.0;
pub const H: f64 = 768.0;

pub const SCOREBOARD: (f64, f64, f64, f64) = (337.0, 209.0, 350.0, 350.0);
pub const MENU_BUTTON: (f64, f64, f64, f64) = (352.0, 499.0, 150.0, 50.0);
pub const RESTART_BUTTON: (f64, f64, f64, f64) = (522.0, 499.0, 150.0, 50.0);
pub const START_BUTTON: (f64, f64, f64, f64) = (470.0, 733.0, 140.0, 23.0);
pub const SOUND_ICON: (f64, f64, f64, f64) = (984.0, 10.0, 30.0, 30.0);
pub const LINK_BOX: (f64, f64, f64, f64) = (300.0, 130.0, 424.0, 24.0);

pub fn hit(r: (f64, f64, f64, f64), p: V2) -> bool {
    let (x, y) = (p.x as f64, p.y as f64);
    x >= r.0 && x <= r.0 + r.2 && y >= r.1 && y <= r.1 + r.3
}

pub struct ConsoleMsg {
    pub stamp: String,
    pub text: String,
    pub color: &'static str,
    pub at: f64,
}

pub struct Explosion {
    pub pos: V2,
    pub started: f64,
}

pub struct Hud<'a> {
    pub now: f64,
    pub me: Option<u32>,
    pub settings: Settings,
    pub code: Option<&'a str>,
    pub link: Option<&'a str>,
    pub host: bool,
    pub online: bool,
    pub show_info: bool,
    pub connected: bool,
    pub console: &'a [ConsoleMsg],
    pub chat_open: bool,
    pub explosions: &'a [Explosion],
    pub mouse: V2,
    pub mouse_down: bool,
    pub sound_icon: &'static str,
    pub own_turret: Option<f32>,
    pub debug: Option<Vec<String>>,
    pub copied: bool,
}

/// Slick's named colours, as the original used them for players.
pub fn slick_color(c: u8) -> &'static str {
    match color_name(c) {
        "blue" => "#0000ff",
        "red" => "#ff0000",
        "yellow" => "#ffff00",
        "green" => "#00ff00",
        "white" => "#ffffff",
        "pink" => "#ffafaf",
        _ => "#000000",
    }
}

fn text(ctx: &Ctx, s: &str, x: f64, y: f64, color: &str) {
    ctx.set_fill_style_str(color);
    let _ = ctx.fill_text(s, x, y);
}

fn sprite_centered(ctx: &Ctx, imgs: &Images, name: &str, c: V2, rot_deg: f64) {
    let Some(img) = imgs.get(name) else { return };
    let (w, h) = (img.natural_width() as f64, img.natural_height() as f64);
    ctx.save();
    let _ = ctx.translate(c.x as f64, c.y as f64);
    if rot_deg != 0.0 {
        let _ = ctx.rotate(rot_deg.to_radians());
    }
    let _ = ctx.draw_image_with_html_image_element(&img, -w / 2.0, -h / 2.0);
    ctx.restore();
}

fn round_rect(ctx: &Ctx, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    ctx.begin_path();
    ctx.move_to(x + r, y);
    let _ = ctx.arc_to(x + w, y, x + w, y + h, r);
    let _ = ctx.arc_to(x + w, y + h, x, y + h, r);
    let _ = ctx.arc_to(x, y + h, x, y, r);
    let _ = ctx.arc_to(x, y, x + w, y, r);
    ctx.close_path();
}

pub fn draw(ctx: &Ctx, imgs: &Images, v: Option<&GameView>, hud: &Hud) {
    ctx.set_global_alpha(1.0);
    ctx.set_text_baseline("top");
    ctx.set_font("14px Verdana, sans-serif");
    match imgs.get("map") {
        Some(map) => {
            let _ = ctx.draw_image_with_html_image_element(&map, 0.0, 0.0);
        }
        None => {
            ctx.set_fill_style_str("#7a8a5a");
            ctx.fill_rect(0.0, 0.0, W, H);
        }
    }
    let statics = map::standard();

    // FIRST: spawn points and the zone.
    for e in &statics {
        if let MapEntity::SpawnPoint { pos, rotation } = e {
            sprite_centered(ctx, imgs, "tank_spawnpoint", *pos, *rotation);
        }
    }
    let zone = v.and_then(|v| v.zone);
    if let Some(z) = zone {
        sprite_centered(ctx, imgs, "king_of_the_hill_zone", z, 0.0);
    }

    // SECOND: tanks.
    let tanks: &[TankView] = v.map(|v| v.tanks.as_slice()).unwrap_or(&[]);
    for t in tanks {
        sprite_centered(ctx, imgs, &format!("tank_{}", color_name(t.color)), t.pos, t.theta as f64 + 90.0);
    }

    // THIRD: turrets, walls, power-ups.
    for t in tanks {
        let angle = match (hud.own_turret, hud.me) {
            (Some(a), Some(me)) if me == t.player => a,
            _ => t.turret_angle,
        };
        if let Some(img) = imgs.get(&format!("turret_{}", color_name(t.color))) {
            ctx.save();
            let _ = ctx.translate(t.turret_pos.x as f64, t.turret_pos.y as f64);
            let _ = ctx.rotate((angle as f64).to_radians());
            let _ = ctx.draw_image_with_html_image_element(&img, -16.875, -16.875);
            ctx.restore();
        }
    }
    for e in &statics {
        if let MapEntity::Wall(r) = e {
            let name = if r.w == HORIZONTAL_WALL.0 { "horizontal_wall" } else { "vertical_wall" };
            if let Some(img) = imgs.get(name) {
                let _ = ctx.draw_image_with_html_image_element(&img, r.x as f64, r.y as f64);
            }
        }
    }
    if let Some(v) = v {
        for p in &v.pickups {
            if matches!(p.kind, PickupKind::Power(_)) {
                sprite_centered(ctx, imgs, p.kind.sprite(), p.pos, 0.0);
            }
        }

        // FOURTH: projectiles, weapons, shields.
        for p in &v.projectiles {
            sprite_centered(ctx, imgs, p.kind.sprite(), p.pos, p.theta as f64 + 90.0);
        }
        for p in &v.pickups {
            if matches!(p.kind, PickupKind::Weapon(_)) {
                sprite_centered(ctx, imgs, p.kind.sprite(), p.pos, 0.0);
            }
        }
        for t in &v.tanks {
            if t.shield.is_some() {
                sprite_centered(ctx, imgs, "shield", t.pos, 0.0);
            }
        }
    }

    // AnimationHandler: 133x133 frames, 50 ms each.
    if let Some(img) = imgs.get("explosion_animation") {
        for e in hud.explosions {
            let frame = ((hud.now - e.started) / 50.0) as i32;
            if !(0..36).contains(&frame) {
                continue;
            }
            let (sx, sy) = ((frame % 6) as f64 * 133.0, (frame / 6) as f64 * 133.0);
            let _ = ctx.draw_image_with_html_image_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                &img,
                sx,
                sy,
                133.0,
                133.0,
                e.pos.x as f64 - 48.0,
                e.pos.y as f64 - 62.5,
                133.0,
                133.0,
            );
        }
    }

    draw_gui(ctx, imgs, v, hud);
}

fn draw_gui(ctx: &Ctx, imgs: &Images, v: Option<&GameView>, hud: &Hud) {
    let players: &[PlayerView] = v.map(|v| v.players.as_slice()).unwrap_or(&[]);
    let me = hud.me.and_then(|id| players.iter().find(|p| p.id == id));

    draw_console(ctx, hud);

    if let Some(v) = v {
        if v.phase == Phase::Countdown && v.delay_ms > 0 {
            text(ctx, "Round starts in: ", 440.0, 75.0, "#000");
            text(ctx, &format!("{} seconds!", v.delay_ms / 1000 + 1), 465.0, 95.0, "#000");
        }
    }

    if hud.show_info {
        draw_info(ctx, players, me, hud);
    }

    if let (Some(me), Some(v)) = (me, v) {
        if hud.settings.koth {
            let z = v.zone.unwrap_or(ZONE_POS);
            let frac = (me.score as f64 / hud.settings.score_limit.max(1) as f64).min(1.0);
            ctx.set_fill_style_str("#00ff00");
            round_rect(ctx, z.x as f64 - 42.0, z.y as f64 - 60.0, 75.0 * frac, 10.0, 5.0);
            ctx.fill();
            ctx.set_stroke_style_str("#000");
            ctx.set_line_width(1.0);
            round_rect(ctx, z.x as f64 - 42.0, z.y as f64 - 60.0, 75.0, 10.0, 5.0);
            ctx.stroke();
        }
        if let Some(t) = v.tanks.iter().find(|t| t.player == me.id) {
            let shield = t.shield.filter(|s| *s <= 100.0).map(|s| s as f64 / MAX_SHIELD_HEALTH);
            draw_lifebar(ctx, t.health as f64 / 100.0, shield);
        }
    }

    // The host's start button, where the original server window had it.
    if hud.online && hud.host && v.is_some_and(|v| v.phase == Phase::Lobby) {
        let (x, y, w, h) = START_BUTTON;
        let hover = hit(START_BUTTON, hud.mouse);
        ctx.set_fill_style_str(if hover { "rgba(255,255,255,0.7)" } else { "rgba(255,255,255,0.4)" });
        ctx.fill_rect(x, y, w, h);
        ctx.set_stroke_style_str("#ff0000");
        ctx.set_line_width(1.0);
        ctx.stroke_rect(x + 0.5, y + 0.5, w, h);
        text(ctx, "Start!", x + 45.0, y + 4.0, "#ff0000");
    }

    if hud.online && !hud.connected {
        text(ctx, "Connecting...", 450.0, 360.0, "#000");
    }

    if v.is_some_and(|v| v.phase == Phase::GameOver) {
        draw_scoreboard(ctx, imgs, players, hud);
    }

    if let Some(img) = imgs.get(hud.sound_icon) {
        let _ = ctx.draw_image_with_html_image_element(&img, SOUND_ICON.0, SOUND_ICON.1);
    }

    if let Some(lines) = &hud.debug {
        for (i, l) in lines.iter().enumerate() {
            text(ctx, l, 18.0, 280.0 + 20.0 * i as f64, "#fff");
        }
    }
}

fn draw_lifebar(ctx: &Ctx, health: f64, shield: Option<f64>) {
    let (x, y, w, h) = (412.0, 10.0, 200.0, 14.0);
    let bar = |y: f64, frac: f64, color: &str| {
        ctx.set_fill_style_str("#000");
        round_rect(ctx, x, y, w, h, h / 2.0);
        ctx.fill();
        ctx.set_fill_style_str(color);
        round_rect(ctx, x, y, w * frac.clamp(0.0, 1.0), h, h / 2.0);
        ctx.fill();
        ctx.set_stroke_style_str("#000");
        ctx.set_line_width(2.0);
        round_rect(ctx, x, y, w, h, h / 2.0);
        ctx.stroke();
    };
    bar(y, health, "#ff0000");
    if let Some(s) = shield.filter(|s| *s > 0.0 && *s <= 1.0) {
        bar(y + h + 10.0, s, "#00ff00");
    }
    ctx.set_line_width(1.0);
}

/// ClientState.renderGUI's "waiting for the host" screen.
fn draw_info(ctx: &Ctx, players: &[PlayerView], me: Option<&PlayerView>, hud: &Hud) {
    let s = hud.settings;
    let black = "#000";
    if let Some(code) = hud.code {
        ctx.set_font("bold 22px Verdana, sans-serif");
        text(ctx, &format!("Game code: {code}"), 400.0, 100.0, black);
        ctx.set_font("14px Verdana, sans-serif");
        if let Some(link) = hud.link {
            let (x, y, w, h) = LINK_BOX;
            ctx.set_fill_style_str(if hit(LINK_BOX, hud.mouse) { "rgba(255,255,255,0.8)" } else { "rgba(255,255,255,0.5)" });
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
        text(ctx, &color_name(me.color).to_uppercase(), 528.0, 195.0, slick_color(me.color));
    }
    if s.koth {
        text(ctx, "KING OF THE HILL", 435.0, 235.0, black);
        text(ctx, "Score points by being in the zone in the center!", 295.0, 255.0, black);
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
    for (i, p) in players.iter().enumerate() {
        text(ctx, &p.name, 445.0, 475.0 + 20.0 * i as f64, slick_color(p.color));
    }
    text(ctx, "Press 'Enter' to chat...", 400.0, 575.0, black);
}

/// Console.renderMessages: the latest lines, hidden after a while.
fn draw_console(ctx: &Ctx, hud: &Hud) {
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
        text(ctx, &format!("[{}]", m.stamp), 14.0, y, "#000");
        if m.color == "#ffff00" {
            // Yellow on the light map needs an outline to read.
            ctx.set_stroke_style_str("rgba(0,0,0,0.6)");
            ctx.set_line_width(2.0);
            let _ = ctx.stroke_text(&m.text, 74.0, y);
            ctx.set_line_width(1.0);
        }
        text(ctx, &m.text, 74.0, y, m.color);
    }
    ctx.set_global_alpha(1.0);
}

fn draw_scoreboard(ctx: &Ctx, imgs: &Images, players: &[PlayerView], hud: &Hud) {
    let (x, y, w, h) = SCOREBOARD;
    ctx.set_fill_style_str("rgb(212,212,212)");
    ctx.fill_rect(x, y, w, h);
    ctx.set_stroke_style_str("#000");
    ctx.set_line_width(7.0);
    ctx.stroke_rect(x, y, w, h);
    ctx.set_line_width(1.0);
    ctx.set_font("22px Verdana, sans-serif");
    let mut sorted: Vec<&PlayerView> = players.iter().collect();
    sorted.sort_by(|a, b| b.score.cmp(&a.score));
    for (i, p) in sorted.iter().enumerate() {
        text(ctx, &format!("{}: {}", p.name, p.score), x + 20.0, y + 10.0 + 30.0 * i as f64, slick_color(p.color));
    }
    ctx.set_font("14px Verdana, sans-serif");
    draw_button(ctx, imgs, "button_menu", MENU_BUTTON, hud);
    if !hud.online || hud.host {
        draw_button(ctx, imgs, "button_restart", RESTART_BUTTON, hud);
    }
}

fn draw_button(ctx: &Ctx, imgs: &Images, name: &str, r: (f64, f64, f64, f64), hud: &Hud) {
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
