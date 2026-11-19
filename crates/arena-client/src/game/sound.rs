//! Events to sounds, and an engine voice per nearby tank.

use std::collections::HashMap;

use arena_audio::{EngineHandle, EngineInput, EngineKind, SoundId};
pub use arena_audio::{MusicTrack, Stinger};
use arena_core::events::{HitLocation, PickupKind, Weapon};
use arena_core::parts::{AltKind, GunKind, PartKind};
use arena_core::{Event, PlayerId, ScrapKind};
use glam::Vec3;

use crate::world::TankView;

#[cfg(target_arch = "wasm32")]
use arena_audio::{Audio, Volumes};

/// The bus levels (natively a stand-in for arena-audio's).
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug)]
struct Volumes {
    master: f32,
    sfx: f32,
    engine: f32,
    ambience: f32,
    music: f32,
}

/// Natively there is no playback yet: the same calls, silent.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct Audio;

#[cfg(not(target_arch = "wasm32"))]
impl Audio {
    fn new() -> Self {
        Audio
    }
    fn unlock(&mut self) {}
    fn tick(&mut self) {}
    fn set_listener(&mut self, _pos: [f32; 3], _forward: [f32; 3]) {}
    fn play_pitched(&mut self, _id: SoundId, _pos: Option<[f32; 3]>, _gain: f32, _rate: f32) {}
    fn set_master_volume(&mut self, _v: f32) {}
    fn set_sfx_volume(&mut self, _v: f32) {}
    fn set_engine_volume(&mut self, _v: f32) {}
    fn set_volumes(&mut self, _v: Volumes) {}
    fn music_set(&mut self, _track: MusicTrack, _intensity: f32) {}
    fn music_stinger(&mut self, _s: Stinger) {}
    fn ambience_set(&mut self, _wind: f32, _storm: f32) {}
    fn horn(&mut self, _index: usize, _pos: Option<[f32; 3]>, _gain: f32) {}
    fn flames(&mut self, _lit: &[(u64, Option<[f32; 3]>)]) {}
    fn engine_add(&mut self, _kind: EngineKind, _seed: u64) -> Option<EngineHandle> {
        None
    }
    fn engine_set(&mut self, _h: EngineHandle, _pos: [f32; 3], _input: EngineInput, _gain: f32) {}
    fn engine_set_own(&mut self, _h: EngineHandle, _input: EngineInput, _gain: f32) {}
    fn engine_remove(&mut self, _h: EngineHandle) {}
}

/// Engines further than this are not voiced.
const ENGINE_RANGE: f32 = 140.0;
const MAX_VOICED: usize = 6;

pub struct Sound {
    audio: Audio,
    engines: HashMap<PlayerId, EngineHandle>,
    /// Where each tank was last drawn (for events without a position: horns).
    seen: HashMap<PlayerId, Vec3>,
}

fn p(v: Vec3) -> Option<[f32; 3]> {
    Some(v.to_array())
}

/// Every gun tier its own voice: (sound, gain, rate).
fn gun_sound(g: GunKind) -> (SoundId, f32, f32) {
    match g {
        GunKind::Stub => (SoundId::StubGun, 0.9, 1.0),
        GunKind::HeavyCannon => (SoundId::Cannon, 1.0, 1.0),
        GunKind::BigBore => (SoundId::BigBore, 1.0, 1.0),
        GunKind::TwinBarrel => (SoundId::TwinBarrel, 1.0, 1.0),
        GunKind::LongGun => (SoundId::LongGun, 1.0, 1.0),
        // The bosses: the War Rig's twin cannon a size up, the Land-Ship's
        // deck guns rattling like autocannons.
        GunKind::RigCannons => (SoundId::TwinBarrel, 1.15, 0.85),
        GunKind::DeckGun => (SoundId::AutocannonBurst, 1.0, 0.9),
    }
}

fn part_hit_sound(part: PartKind) -> SoundId {
    use PartKind::*;
    match part {
        StubGun | HeavyCannon | BigBore | TwinBarrel | LongGun => SoundId::HitBarrel,
        SensorMast | TeslaSpire | Coilgun => SoundId::HitMast,
        _ => SoundId::HitTurret,
    }
}

impl Sound {
    pub fn new() -> Self {
        Sound { audio: Audio::new(), engines: HashMap::new(), seen: HashMap::new() }
    }

    /// Renders more sound take variations (a few ms a frame, though one
    /// take can run 8-20 ms). [`Sound::frame`] does this in a match; the
    /// title and loading screens call it so the work is mostly done before
    /// the first match starts (it showed as hitches in its first second).
    pub fn tick(&mut self) {
        self.audio.tick();
    }

    /// From a user gesture: browsers keep audio off until one.
    pub fn unlock(&mut self) {
        self.audio.unlock();
    }

    pub fn set_volumes(&mut self, master: f32, sfx: f32, ambience: f32, music: f32) {
        self.audio.set_volumes(Volumes { master, sfx, engine: sfx * 0.8, ambience, music });
    }

    /// The soundtrack, every frame: which track and how hard (0.1 calm .. 1 brawl).
    pub fn music(&mut self, track: MusicTrack, intensity: f32) {
        self.audio.music_set(track, intensity.clamp(0.0, 1.0));
    }

    /// A cue over the music (round start, victory, boss...).
    pub fn stinger(&mut self, s: Stinger) {
        self.audio.music_stinger(s);
    }

    /// The weather bed, every frame: desert `wind` 0..1 and `storm` 0..1.
    pub fn ambience(&mut self, wind: f32, storm: f32) {
        self.audio.ambience_set(wind.clamp(0.0, 1.0), storm.clamp(0.0, 1.0));
    }

    /// Your shot landed: a bright tick on a part, a dull one on armour.
    pub fn hit_tick(&mut self, part: bool) {
        if part {
            self.audio.play_pitched(SoundId::UiTick, None, 0.75, 1.6);
        } else {
            self.audio.play_pitched(SoundId::UiTick, None, 0.55, 0.7);
        }
    }

    /// A menu button: a real switch click (call from the UI's click handler).
    #[allow(dead_code)]
    pub fn ui_click(&mut self) {
        self.audio.play_pitched(SoundId::UiTick, None, 0.6, 1.0);
    }

    /// A menu choice confirmed (call from the UI).
    #[allow(dead_code)]
    pub fn ui_confirm(&mut self) {
        self.audio.play_pitched(SoundId::UiConfirm, None, 0.7, 1.0);
    }

    /// A menu action refused (call from the UI).
    #[allow(dead_code)]
    pub fn ui_error(&mut self) {
        self.audio.play_pitched(SoundId::UiError, None, 0.7, 1.0);
    }

    pub fn kill_tick(&mut self) {
        self.audio.play_pitched(SoundId::UiConfirm, None, 0.8, 0.9);
    }

    /// Low hull: a thump, quicker and louder the lower it gets.
    pub fn heartbeat(&mut self, hp: f32) {
        let g = 0.25 + (0.3 - hp).max(0.0) * 1.5;
        self.audio.play_pitched(SoundId::ArmourThud, None, g, 0.35);
    }

    /// One sim event. `you` hears your own guns centred.
    pub fn event(&mut self, e: &Event, you: Option<PlayerId>) {
        let a = &mut self.audio;
        let mine = |pl: PlayerId| you == Some(pl);
        match e {
            Event::Fired { player, weapon, pos, .. } => {
                let at = if mine(*player) { None } else { p(*pos) };
                let (id, gain, rate) = match weapon {
                    Weapon::Gun(g) => gun_sound(*g),
                    Weapon::Alt(AltKind::Mg) => (SoundId::MachineGun, 0.6, 1.0),
                    Weapon::Alt(AltKind::Coilgun) => (SoundId::CoilgunZap, 1.0, 1.0),
                    Weapon::Alt(AltKind::Tesla) => (SoundId::TeslaCrackle, 0.9, 1.0),
                    Weapon::Alt(AltKind::Rockets) => (SoundId::RocketLaunch, 0.8, 1.0),
                    // Held: started, looped and stopped from `TankView::flaming` in `frame`.
                    Weapon::Alt(AltKind::Flamethrower) => return,
                    Weapon::Special(PartKind::Mortar | PartKind::ShipMortar) => (SoundId::MortarShot, 1.0, 1.0),
                    Weapon::Special(PartKind::Harpoon) => (SoundId::RocketLaunch, 0.6, 0.7),
                    Weapon::Special(_) => (SoundId::Cannon, 0.9, 0.7),
                    _ => return,
                };
                a.play_pitched(id, at, gain, rate);
            }
            // The MG sends no `Fired`: each round is a tracer.
            Event::Tracer { player, weapon: Weapon::Alt(AltKind::Mg), from, .. } => {
                let at = if mine(*player) { None } else { p(*from) };
                a.play_pitched(SoundId::MachineGun, at, if mine(*player) { 0.5 } else { 0.6 }, 1.0);
            }
            Event::Hit { target, location, pos, damage, .. } => {
                let id = match location {
                    HitLocation::Hull => {
                        if *damage < 2.5 {
                            SoundId::Ricochet
                        } else if *damage < 6.0 {
                            SoundId::ArmourThud
                        } else {
                            SoundId::HitHull
                        }
                    }
                    HitLocation::Turret => SoundId::HitTurret,
                    HitLocation::Track(_) => SoundId::HitTrack,
                    HitLocation::Part(k) => part_hit_sound(*k),
                };
                let at = if mine(*target) { None } else { p(*pos) };
                a.play_pitched(id, at, 0.9, 1.0);
            }
            Event::Impact { pos, .. } => a.play_pitched(SoundId::SandImpact, p(*pos), 0.8, 1.0),
            Event::Explosion { pos, radius, weapon } => {
                let id = if matches!(weapon, Weapon::Hazard) {
                    SoundId::FuelBlast
                } else if *radius > 5.0 || matches!(weapon, Weapon::AirStrike) {
                    SoundId::ExplosionLarge
                } else {
                    SoundId::ExplosionSmall
                };
                a.play_pitched(id, p(*pos), 1.0, 1.0);
            }
            Event::TeslaArc { from, .. } => a.play_pitched(SoundId::TeslaCrackle, p(*from), 0.8, 1.0),
            Event::CoilCharging { player } => {
                if mine(*player) {
                    a.play_pitched(SoundId::CoilgunCharge, None, 0.7, 1.0);
                }
            }
            Event::PartBrokeOff { pos, .. } => a.play_pitched(SoundId::PartBreak, p(*pos), 1.0, 1.0),
            Event::TrackBlown { player, .. } => {
                if mine(*player) {
                    a.play_pitched(SoundId::PartBreak, None, 0.9, 0.8);
                }
            }
            Event::TrackRepaired { player, .. } | Event::RepairFinished { player, .. } => {
                if mine(*player) {
                    a.play_pitched(SoundId::Repair, None, 0.7, 1.0);
                }
            }
            Event::Destroyed { pos, .. } => a.play_pitched(SoundId::TankDestroyed, p(*pos), 1.0, 1.0),
            Event::PickedUp { player, kind, pos } => {
                let id = match kind {
                    PickupKind::Scrap(ScrapKind::Steel) => SoundId::ScrapSteel,
                    PickupKind::Scrap(ScrapKind::Powder) => SoundId::ScrapPowder,
                    PickupKind::Scrap(ScrapKind::Volts) => SoundId::ScrapVolts,
                    PickupKind::Scrap(ScrapKind::Fuel) => SoundId::ScrapFuel,
                    PickupKind::Repair => SoundId::Repair,
                    _ => SoundId::UpgradeBolt,
                };
                let at = if mine(*player) { None } else { p(*pos) };
                a.play_pitched(id, at, if mine(*player) { 0.8 } else { 0.5 }, 1.0);
            }
            Event::UpgradeApplied { player, .. } | Event::PartAttached { player, .. } => {
                if mine(*player) {
                    a.play_pitched(SoundId::UpgradeBolt, None, 1.0, 1.0);
                }
            }
            Event::Ram { pos, speed, .. } => a.play_pitched(SoundId::ArmourThud, p(*pos), (speed / 8.0).clamp(0.4, 1.2), 0.8),
            Event::ShieldHit { pos, .. } => a.play_pitched(SoundId::ArmourThud, p(*pos), 0.6, 1.4),
            // Fuel silos, tanks and drums: the blast and the fireball, chained.
            Event::HazardExploded { pos, .. } => a.play_pitched(SoundId::FuelBlast, p(*pos), 1.2, 1.0),
            Event::Launched { pos, .. } => a.play_pitched(SoundId::ArmourThud, p(*pos), 1.0, 1.6),
            Event::MineTriggered { pos, .. } => a.play_pitched(SoundId::ExplosionSmall, p(*pos), 1.0, 1.0),
            Event::AirStrikeCalled { target, .. } => {
                a.play_pitched(SoundId::FlarePop, p(*target), 1.0, 1.0);
                a.play_pitched(SoundId::AirRaidSiren, None, 0.7, 1.0);
            }
            Event::WaveStarted { .. } => a.play_pitched(SoundId::WaveHorn, None, 0.9, 1.0),
            Event::WaveCleared { .. } => a.play_pitched(SoundId::UiConfirm, None, 0.7, 1.0),
            Event::BossSpawned { pos, .. } => a.play_pitched(SoundId::BossRoar, p(*pos), 1.2, 1.0),
            Event::BossModuleLost { pos, .. } => a.play_pitched(SoundId::ModuleBlowout, p(*pos), 1.2, 1.0),
            Event::BossDefeated { .. } => a.play_pitched(SoundId::TankDestroyed, None, 1.2, 0.6),
            Event::RaidLost { .. } => a.play_pitched(SoundId::UiError, None, 0.8, 0.8),
            Event::BombingRun { from, to, .. } => {
                a.play_pitched(SoundId::BomberFlyover, p((*from + *to) * 0.5), 1.0, 1.0);
                a.play_pitched(SoundId::BombWhistle, p((*from + *to) * 0.5), 0.9, 1.0);
            }
            Event::CraneHit { pos, .. } => a.play_pitched(SoundId::HitHull, p(*pos), 1.0, 0.7),
            Event::ObstacleDestroyed { pos, .. } => a.play_pitched(SoundId::PartBreak, p(*pos), 0.8, 0.6),
            Event::Horn { player, horn } => {
                let at = if mine(*player) { None } else { self.seen.get(player).map(|v| v.to_array()) };
                a.horn(*horn as usize, at, 1.0);
            }
            Event::MatchEnded { .. } => a.play_pitched(SoundId::CountdownGo, None, 0.8, 1.0),
            _ => {}
        }
    }

    /// Ears and engines, once a frame. `own_throttle` is your input.
    pub fn frame(&mut self, listener: Vec3, forward: Vec3, tanks: &[TankView], you: Option<PlayerId>, own_throttle: f32) {
        self.audio.tick();
        self.audio.set_listener(listener.to_array(), forward.to_array());
        self.seen.clear();
        self.seen.extend(tanks.iter().map(|t| (t.player, t.pos)));
        // Lit flamethrowers within earshot: ignition, roar loop, cut-off tail.
        let lit: Vec<(u64, Option<[f32; 3]>)> = tanks
            .iter()
            .filter(|t| t.alive && t.flaming)
            .filter(|t| Some(t.player) == you || t.pos.distance(listener) < ENGINE_RANGE * 1.5)
            .map(|t| (u64::from(t.player), if Some(t.player) == you { None } else { Some(t.pos.to_array()) }))
            .collect();
        self.audio.flames(&lit);
        // The nearest live tanks get a voice.
        let mut near: Vec<(&TankView, f32)> = tanks
            .iter()
            .filter(|t| t.alive)
            .map(|t| (t, if Some(t.player) == you { -1.0 } else { t.pos.distance(listener) }))
            .filter(|(_, d)| *d < ENGINE_RANGE)
            .collect();
        near.sort_by(|a, b| a.1.total_cmp(&b.1));
        near.truncate(MAX_VOICED);
        let keep: Vec<PlayerId> = near.iter().map(|(t, _)| t.player).collect();
        let gone: Vec<PlayerId> = self.engines.keys().copied().filter(|p| !keep.contains(p)).collect();
        for g in gone {
            if let Some(h) = self.engines.remove(&g) {
                self.audio.engine_remove(h);
            }
        }
        for (t, _) in near {
            let h = match self.engines.get(&t.player) {
                Some(h) => *h,
                None => match self.audio.engine_add(EngineKind::Brawler, u64::from(t.player) * 7717 + 3) {
                    Some(h) => {
                        self.engines.insert(t.player, h);
                        h
                    }
                    None => continue,
                },
            };
            let ts = t.track_speed[0].abs().max(t.track_speed[1].abs());
            let own = Some(t.player) == you;
            let load = if own { own_throttle.abs() } else { (t.throttle * 1.4).min(1.0) };
            let input = EngineInput {
                rpm: (0.15 + ts / 10.0 * 0.7 + load * 0.25).min(1.0),
                load,
                boost: if t.boosting { 1.0 } else { 0.0 },
                track_speed: ts,
            };
            if own {
                self.audio.engine_set_own(h, input, 0.8);
            } else {
                self.audio.engine_set(h, t.pos.to_array(), input, 1.0);
            }
        }
    }

    /// Silence the engines (back to the title).
    pub fn clear_engines(&mut self) {
        for (_, h) in self.engines.drain() {
            self.audio.engine_remove(h);
        }
    }
}
