//! A busy-scene bench and close-up shots for the shield, projectile, pickup
//! and air-strike visuals: `cargo test -p arena-client --release --
//! --ignored vfx --nocapture` (needs a GPU). Shots go to docs/shots.

use arena_core::snapshot::ShotKind;
use arena_core::{Event, Map, MapId, PickupKind, ScrapKind, Sim};
use glam::{Quat, Vec3};

use crate::camera::Camera;
use crate::renderer::{Quality, Renderer};
use crate::world::{paint_for, PickupView, RenderWorld, ShellView, StrikeView, WorldRenderer};

fn device() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::PRIMARY, ..wgpu::InstanceDescriptor::new_without_display_handle() });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, ..Default::default() })).expect("adapter");
    pollster::block_on(crate::renderer::request_device(&adapter)).expect("device")
}

/// A practice match on the Sink played a while, then crowded: every tank
/// shielded, pickups of every kind, shells of every kind in the air, an
/// air strike on its run. The camera chases tank 0's neighbour.
fn busy(wr: &mut WorldRenderer) -> RenderWorld {
    let seed = 7;
    let (mut sim, _) = Sim::practice_on(MapId::Sink, seed, 7, 0.7);
    wr.set_map(&Map::by_id(MapId::Sink, seed));
    let mut world = RenderWorld::default();
    let ticks = 20 * 60;
    for tick in 0..ticks {
        sim.step();
        let ev = sim.drain_events();
        if tick + 300 > ticks {
            world = RenderWorld::from_snapshot(&sim.snapshot(), &paint_for);
            world.time = tick as f32 * arena_core::DT;
            for e in &ev {
                wr.event(e, Vec3::ZERO);
            }
            wr.fx.update(arena_core::DT, &world);
        }
    }
    let (pos, rot) = world.tanks.iter().find(|t| t.alive).map(|t| (t.pos, t.rot)).unwrap_or((Vec3::ZERO, Quat::IDENTITY));
    let fwd = (rot * Vec3::X).with_y(0.0).normalize_or(Vec3::X);
    let side = Vec3::new(-fwd.z, 0.0, fwd.x);
    world.camera = Some(Camera::look(pos - fwd * 10.0 + Vec3::Y * 4.0, pos + fwd * 25.0 + Vec3::Y * 1.5, 58.0));
    world.shields = world.tanks.iter().filter(|t| t.alive).map(|t| (t.player, 0.8)).collect();
    world.buffs = world.tanks.iter().map(|t| (t.player, arena_core::PowerUpKind::ALL[t.player as usize % 5])).collect();
    let kinds = [
        PickupKind::Scrap(ScrapKind::Steel),
        PickupKind::Scrap(ScrapKind::Powder),
        PickupKind::Scrap(ScrapKind::Volts),
        PickupKind::Scrap(ScrapKind::Fuel),
        PickupKind::Repair,
        PickupKind::AirStrike,
    ];
    world.pickups.clear();
    for k in 0..36u16 {
        let p = pos + fwd * (8.0 + (k / 6) as f32 * 6.0) + side * ((k % 6) as f32 - 2.5) * 4.0;
        world.pickups.push(PickupView { id: 500 + k, kind: kinds[k as usize % kinds.len()], pos: p.with_y(pos.y - 0.6) });
    }
    world.shells.clear();
    let shots = [ShotKind::Shell, ShotKind::Rocket, ShotKind::MortarShell, ShotKind::Harpoon];
    for k in 0..40 {
        let p = pos + fwd * (6.0 + k as f32 * 1.5) + side * ((k % 5) as f32 - 2.0) * 3.0 + Vec3::Y * (2.0 + (k % 3) as f32);
        world.shells.push(ShellView { pos: p, vel: fwd * 90.0 + side * 10.0, big: k % 4 == 0, kind: shots[k % shots.len()] });
    }
    let c = pos + fwd * 40.0;
    world.strikes = vec![StrikeView { id: 900, target: c, from: c - side * 30.0, to: c + side * 30.0, delay: 1.0 }];
    world.dt = 1.0 / 60.0;
    world
}

/// Shield hits every few frames round the tanks in view.
fn pepper(wr: &mut WorldRenderer, world: &RenderWorld, frame: u32) {
    if frame % 6 != 0 {
        return;
    }
    for (i, t) in world.tanks.iter().filter(|t| t.alive).enumerate() {
        let a = frame as f32 * 0.37 + i as f32;
        let d = Vec3::new(a.cos(), 0.35, a.sin()).normalize();
        wr.event(&Event::ShieldHit { player: t.player, pos: t.pos + d * 4.0, damage: 10.0 }, Vec3::ZERO);
    }
}

#[test]
#[ignore]
fn vfx_bench() {
    let (w, h) = (1920, 1080);
    let (device, queue) = device();
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    for q in [Quality::High, Quality::Low] {
        let mut wr = WorldRenderer::new(Renderer::with_quality(device.clone(), queue.clone(), format, w, h, q));
        let mut world = busy(&mut wr);
        let (mut cpu, mut wall, mut n) = (0.0f64, 0.0f64, 0u32);
        let mut gpu = 0.0f32;
        let mut gn = 0;
        for frame in 0..480u32 {
            world.time += world.dt;
            pepper(&mut wr, &world, frame);
            let t0 = std::time::Instant::now();
            wr.draw(&view, &world);
            let t1 = t0.elapsed().as_secs_f64() * 1000.0;
            wr.renderer.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
            let t2 = t0.elapsed().as_secs_f64() * 1000.0;
            if frame >= 180 {
                cpu += t1;
                wall += t2;
                n += 1;
                if frame % 30 == 0 && wr.renderer.gpu_ms[0] > 0.0 {
                    gpu += wr.renderer.gpu_ms[0];
                    gn += 1;
                }
            }
        }
        println!(
            "VFX {q:?}: cpu draw {:.2} ms, wall {:.2} ms, gpu {:.2} ms, draws {} instances {}",
            cpu / n as f64,
            wall / n as f64,
            gpu / gn.max(1) as f32,
            wr.renderer.stats.0,
            wr.renderer.stats.1
        );
    }
}

// ── Shots ───────────────────────────────────────────────────────────────

/// Renders `frames` frames of a scene at 60 FPS (the step hook may feed
/// events or move things), then saves the last to docs/shots/`name`.png.
fn shoot(name: &str, setup: impl FnOnce(&mut WorldRenderer) -> RenderWorld, frames: u32, mut step: impl FnMut(&mut WorldRenderer, &mut RenderWorld, u32)) {
    let (w, h) = (1600u32, 900u32);
    let (device, queue) = device();
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut wr = WorldRenderer::new(Renderer::with_quality(device, queue, format, w, h, Quality::High));
    let mut world = setup(&mut wr);
    let texture = wr.renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    world.dt = 1.0 / 60.0;
    for f in 0..frames {
        step(&mut wr, &mut world, f);
        wr.draw(&view, &world);
        wr.renderer.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        world.time += world.dt;
    }
    let r = &wr.renderer;
    let padded = (w * 4).div_ceil(256) * 256;
    let buffer = r.device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (padded * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    let mut enc = r.device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(h) } },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    r.queue.submit([enc.finish()]);
    buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    r.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    let mapped = buffer.slice(..).get_mapped_range().expect("mapped");
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h as usize {
        px.extend_from_slice(&mapped[row * padded as usize..row * padded as usize + (w * 4) as usize]);
    }
    let path = format!("{}/../../docs/shots/{name}.png", env!("CARGO_MANIFEST_DIR"));
    image::save_buffer(&path, &px, w, h, image::ExtendedColorType::Rgba8).expect("save");
}

/// An open spot on the Sink (where a practice tank spawned), its facing,
/// and the map.
fn open_ground(wr: &mut WorldRenderer) -> (RenderWorld, Vec3, Vec3, Map) {
    let seed = 7;
    let (mut sim, _) = Sim::practice_on(MapId::Sink, seed, 1, 0.7);
    let map = Map::by_id(MapId::Sink, seed);
    wr.set_map(&map);
    for _ in 0..30 {
        sim.step();
    }
    let mut world = RenderWorld::from_snapshot(&sim.snapshot(), &paint_for);
    let t = world.tanks.first().cloned().expect("a tank");
    let fwd = (t.rot * Vec3::X).with_y(0.0).normalize_or(Vec3::X);
    let spot = Vec3::new(t.pos.x, map.height_at(t.pos.x, t.pos.z), t.pos.z);
    world.tanks.clear();
    world.pickups.clear();
    world.shells.clear();
    world.time = 10.0;
    (world, spot, fwd, map)
}

fn yaw_of(fwd: Vec3) -> f32 {
    (-fwd.z).atan2(fwd.x)
}

#[test]
#[ignore]
fn vfx_shots_shield() {
    use crate::world::parked_tank;
    use arena_core::PartKind;
    for (name, own) in [("vfx-shield-hits", false), ("vfx-shield-own", true)] {
        shoot(
            name,
            |wr| {
                let (mut world, spot, fwd, map) = open_ground(wr);
                let side = Vec3::new(-fwd.z, 0.0, fwd.x);
                let a = spot;
                let b = spot + fwd * 14.0 + side * 5.0;
                let b = b.with_y(map.height_at(b.x, b.z));
                world.tanks.push(parked_tank(1, crate::tank::RUSTBACK, a, yaw_of(fwd), PartKind::BigBore, vec![PartKind::ShieldGenerator, PartKind::FlankPlates]));
                world.tanks.push(parked_tank(2, crate::tank::DUST_SAINT, b, yaw_of(-fwd) + 0.5, PartKind::LongGun, vec![PartKind::ShieldGenerator]));
                world.shields = vec![(1, 0.9), (2, 0.7)];
                world.camera = Some(if own {
                    Camera::look(a - fwd * 10.0 + Vec3::Y * 4.5, a + fwd * 25.0 + Vec3::Y * 1.5, 58.0)
                } else {
                    Camera::look(a + side * 13.0 + fwd * 4.0 + Vec3::Y * 4.0, (a + b) * 0.5 + Vec3::Y * 1.0, 50.0)
                });
                world
            },
            70,
            |wr, world, f| {
                // A volley: hits from different sides, the last fresh.
                let hits: [(u32, u8, Vec3); 6] = [
                    (10, 1, Vec3::new(1.0, 0.4, 0.3)),
                    (25, 2, Vec3::new(-1.0, 0.3, 0.2)),
                    (38, 1, Vec3::new(0.2, 0.5, 1.0)),
                    (50, 1, Vec3::new(-0.6, 0.6, -0.8)),
                    (58, 2, Vec3::new(0.3, 0.6, -1.0)),
                    (66, 1, Vec3::new(0.9, 0.2, -0.5)),
                ];
                for (at, who, d) in hits {
                    if f == at {
                        let t = world.tanks.iter().find(|t| t.player == who).expect("tank");
                        // Mostly on the side facing the camera, so the rings show.
                        let to_cam = (world.camera.expect("camera").eye - t.pos).normalize();
                        let pos = t.pos + (to_cam * 1.2 + (t.rot * d).normalize()).normalize() * 4.0;
                        wr.event(&Event::ShieldHit { player: who, pos, damage: 20.0 }, Vec3::ZERO);
                    }
                }
            },
        );
    }
}

#[test]
#[ignore]
fn vfx_shots_projectiles() {
    shoot(
        "vfx-projectiles",
        |wr| {
            let (mut world, spot, fwd, _) = open_ground(wr);
            let side = Vec3::new(-fwd.z, 0.0, fwd.x);
            let base = spot + Vec3::Y * 2.2;
            let kinds = [(ShotKind::Shell, false), (ShotKind::Shell, true), (ShotKind::Rocket, false), (ShotKind::MortarShell, false), (ShotKind::Harpoon, false)];
            for (k, (kind, big)) in kinds.iter().enumerate() {
                let p = base + side * (k as f32 - 2.0) * 1.6 + fwd * (k % 2) as f32 * 0.6;
                let vel = if *kind == ShotKind::MortarShell { fwd * 30.0 + Vec3::Y * 12.0 } else { fwd * 90.0 };
                world.shells.push(ShellView { pos: p, vel, big: *big, kind: *kind });
            }
            world.camera = Some(Camera::look(base - fwd * 3.5 + side * 5.5 + Vec3::Y * 1.6, base + fwd * 1.5, 50.0));
            world
        },
        12,
        |wr, world, f| {
            if f == 9 {
                let cam = world.camera.expect("camera");
                let fwd = (cam.target - cam.eye).with_y(0.0).normalize();
                let side = Vec3::new(-fwd.z, 0.0, fwd.x);
                let from = cam.target - fwd * 6.0 - side * 4.0 + Vec3::Y * 1.5;
                wr.event(&Event::Tracer { player: 1, weapon: arena_core::Weapon::Alt(arena_core::AltKind::Coilgun), from, to: from + fwd * 120.0 }, Vec3::ZERO);
            }
        },
    );
}

#[test]
#[ignore]
fn vfx_shots_pickups() {
    use arena_core::PowerUpKind;
    shoot(
        "vfx-pickups",
        |wr| {
            let (mut world, spot, fwd, map) = open_ground(wr);
            let side = Vec3::new(-fwd.z, 0.0, fwd.x);
            let mut kinds = vec![
                PickupKind::Scrap(ScrapKind::Steel),
                PickupKind::Scrap(ScrapKind::Powder),
                PickupKind::Scrap(ScrapKind::Volts),
                PickupKind::Scrap(ScrapKind::Fuel),
                PickupKind::Repair,
                PickupKind::Rare(arena_core::PartKind::Coilgun),
                PickupKind::AirStrike,
            ];
            kinds.extend(PowerUpKind::ALL.iter().map(|p| PickupKind::PowerUp(*p)));
            for (k, kind) in kinds.iter().enumerate() {
                let row = (k / 6) as f32;
                let col = (k % 6) as f32 - 2.5;
                let p = spot + fwd * (8.0 + row * 7.0) + side * col * 4.0;
                world.pickups.push(PickupView { id: k as u16, kind: *kind, pos: p.with_y(map.height_at(p.x, p.z) + 0.1) });
            }
            // A tank under Overcharge among them.
            let tp = spot + fwd * 22.0 + side * 13.0;
            world.tanks.push(crate::world::parked_tank(3, crate::tank::RUSTBACK, tp.with_y(map.height_at(tp.x, tp.z)), yaw_of(-side), arena_core::PartKind::StubGun, vec![]));
            world.buffs = vec![(3, PowerUpKind::Overcharge)];
            world.camera = Some(Camera::look(spot - fwd * 2.0 + Vec3::Y * 5.5, spot + fwd * 14.0, 55.0));
            world
        },
        5,
        |_, _, _| {},
    );
}

#[test]
#[ignore]
fn vfx_shots_plane() {
    for (name, lead) in [("vfx-plane-incoming", 1.6), ("vfx-plane-bombs", -0.3)] {
        shoot(
            name,
            |wr| {
                let (mut world, spot, fwd, map) = open_ground(wr);
                let side = Vec3::new(-fwd.z, 0.0, fwd.x);
                let c = spot + fwd * 30.0;
                let at = |p: Vec3| p.with_y(map.height_at(p.x, p.z));
                world.strikes = vec![StrikeView { id: 1, target: at(c), from: at(c - side * 30.0), to: at(c + side * 30.0), delay: lead + 1.5 }];
                world.camera = Some(Camera::look(spot + Vec3::Y * 3.0 - fwd * 6.0, c + Vec3::Y * 14.0 - side * 12.0, 62.0));
                world
            },
            90,
            |_, world, _| {
                // Count the delay down with the clock, as the snapshots do.
                for s in &mut world.strikes {
                    s.delay = (s.delay - world.dt).max(0.0);
                }
            },
        );
    }
}
