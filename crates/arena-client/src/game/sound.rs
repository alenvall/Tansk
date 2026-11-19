//! Events to sounds, and an engine voice per nearby tank.

use std::collections::HashMap;

use arena_audio::{EngineHandle, EngineInput, EngineKind, SoundId};
use arena_core::events::{HitLocation, PickupKind, Weapon};
use arena_core::parts::{AltKind, GunKind, PartKind};
use arena_core::{Event, PlayerId, ScrapKind};
use glam::Vec3;

use crate::world::TankView;

#[cfg(target_arch = "wasm32")]
use arena_audio::Audio;

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
}

fn p(v: Vec3) -> Option<[f32; 3]> {
    Some(v.to_array())
}

fn gun_sound(g: GunKind) -> SoundId {
    match g {
        GunKind::BigBore => SoundId::BigBore,
        GunKind::TwinBarrel => SoundId::TwinBarrel,
        GunKind::LongGun => SoundId::LongGun,
        _ => SoundId::Cannon,
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
        Sound { audio: Audio::new(), engines: HashMap::new() }
    }

    /// From a user gesture: browsers keep audio off until one.
    pub fn unlock(&mut self) {
        self.audio.unlock();
    }

    pub fn set_volumes(&mut self, master: f32, sfx: f32) {
        self.audio.set_master_volume(master);
        self.audio.set_sfx_volume(sfx);
        self.audio.set_engine_volume(sfx * 0.8);
    }

    /// One sim event. `you` hears your own guns centred.
    pub fn event(&mut self, e: &Event, you: Option<PlayerId>) {
        let a = &mut self.audio;
        let mine = |pl: PlayerId| you == Some(pl);
        match e {
            Event::Fired { player, weapon, pos, .. } => {
                let at = if mine(*player) { None } else { p(*pos) };
                let (id, gain, rate) = match weapon {
                    Weapon::Gun(g) => (gun_sound(*g), 1.0, 1.0),
                    Weapon::Alt(AltKind::Mg) => (SoundId::MachineGun, 0.6, 1.0),
                    Weapon::Alt(AltKind::Coilgun) => (SoundId::CoilgunZap, 1.0, 1.0),
                    Weapon::Alt(AltKind::Tesla) => (SoundId::TeslaCrackle, 0.9, 1.0),
                    Weapon::Alt(AltKind::Rockets) => (SoundId::AutocannonBurst, 0.8, 1.2),
                    Weapon::Alt(AltKind::Flamethrower) => return,
                    Weapon::Special(_) => (SoundId::Cannon, 0.9, 0.7),
                    _ => return,
                };
                a.play_pitched(id, at, gain, rate);
            }
            Event::Hit { target, location, pos, damage, .. } => {
                let id = match location {
                    HitLocation::Hull => {
                        if *damage < 6.0 {
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
                let id = if *radius > 5.0 || matches!(weapon, Weapon::AirStrike | Weapon::Hazard) {
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
            Event::HazardExploded { pos, .. } => a.play_pitched(SoundId::ExplosionLarge, p(*pos), 1.2, 0.8),
            Event::MineTriggered { pos, .. } => a.play_pitched(SoundId::ExplosionSmall, p(*pos), 1.0, 1.0),
            Event::AirStrikeCalled { target, .. } => {
                // No siren in the bank yet: a low, slow beep and the radio.
                a.play_pitched(SoundId::RadioSquelchOn, None, 0.8, 1.0);
                a.play_pitched(SoundId::CountdownBeep, p(*target), 1.2, 0.45);
            }
            Event::WaveStarted { .. } => a.play_pitched(SoundId::CountdownGo, None, 0.8, 0.8),
            Event::WaveCleared { .. } => a.play_pitched(SoundId::UiConfirm, None, 0.7, 1.0),
            Event::BossSpawned { pos, .. } => a.play_pitched(SoundId::ExplosionLarge, p(*pos), 1.0, 0.45),
            Event::BossModuleLost { pos, .. } => a.play_pitched(SoundId::PartBreak, p(*pos), 1.2, 0.7),
            Event::BossDefeated { .. } => a.play_pitched(SoundId::TankDestroyed, None, 1.2, 0.6),
            Event::RaidLost { .. } => a.play_pitched(SoundId::UiError, None, 0.8, 0.8),
            Event::BombingRun { from, .. } => a.play_pitched(SoundId::ShellWhistle, p(*from), 1.0, 0.8),
            Event::CraneHit { pos, .. } => a.play_pitched(SoundId::HitHull, p(*pos), 1.0, 0.7),
            Event::ObstacleDestroyed { pos, .. } => a.play_pitched(SoundId::PartBreak, p(*pos), 0.8, 0.6),
            Event::MatchEnded { .. } => a.play_pitched(SoundId::CountdownGo, None, 0.8, 1.0),
            _ => {}
        }
    }

    /// Ears and engines, once a frame. `own_throttle` is your input.
    pub fn frame(&mut self, listener: Vec3, forward: Vec3, tanks: &[TankView], you: Option<PlayerId>, own_throttle: f32) {
        self.audio.tick();
        self.audio.set_listener(listener.to_array(), forward.to_array());
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
