//! Native entry: a winit window on Vulkan (Practice against bots, the same
//! game loop as the web), or the headless `--shot`.

use std::sync::Arc;
use std::time::Instant;

use anyhow::{anyhow, bail, Context, Result};
use glam::Vec3;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{CursorGrabMode, Window, WindowId};

use crate::camera::Camera;
use crate::demo::{self, Demo};
use crate::game::{Game, Screen};
use crate::renderer::{request_device, Renderer};
use crate::world::WorldRenderer;

struct Args {
    shot: Option<String>,
    scene: String,
    width: u32,
    height: u32,
    seconds: f32,
    seed: u64,
    /// `ex,ey,ez,tx,ty,tz`: a fixed camera (debugging).
    camera: Option<[f32; 6]>,
}

fn parse_args() -> Result<Args> {
    let mut args = Args { shot: None, scene: "gameplay".into(), width: 1600, height: 900, seconds: 40.0, seed: 7, camera: None };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| anyhow!("{name} needs a value"));
        match a.as_str() {
            "--shot" => args.shot = Some(value("--shot")?),
            "--scene" => args.scene = value("--scene")?,
            "--width" => args.width = value("--width")?.parse().context("--width")?,
            "--height" => args.height = value("--height")?.parse().context("--height")?,
            "--seconds" => args.seconds = value("--seconds")?.parse().context("--seconds")?,
            "--seed" => args.seed = value("--seed")?.parse().context("--seed")?,
            "--camera" => {
                let v: Vec<f32> = value("--camera")?
                    .split(',')
                    .map(|x| x.trim().parse::<f32>())
                    .collect::<Result<_, _>>()
                    .context("--camera wants six numbers")?;
                args.camera = Some(v.try_into().map_err(|_| anyhow!("--camera wants six numbers"))?);
            }
            "--help" | "-h" => {
                println!(
                    "arena-client [--shot out.png [--scene gameplay|showcase|lineup] [--width 1600 --height 900] \
                     [--seconds 40] [--seed 7] [--camera ex,ey,ez,tx,ty,tz]]"
                );
                std::process::exit(0);
            }
            other => bail!("unknown argument {other:?} (try --help)"),
        }
    }
    Ok(args)
}

pub fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn,naga=warn"))
        .init();
    let result = parse_args().and_then(|args| match &args.shot {
        Some(path) => shot(path, &args),
        None => windowed(),
    });
    if let Err(e) = result {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

/// Vulkan first, anything else wgpu supports as a fallback. `WGPU_BACKEND` overrides.
fn instance(backends: wgpu::Backends) -> wgpu::Instance {
    wgpu::Instance::new(wgpu::InstanceDescriptor { backends: backends.with_env(), ..wgpu::InstanceDescriptor::new_without_display_handle() })
}

type Found = (wgpu::Instance, wgpu::Adapter, Option<wgpu::Surface<'static>>);

fn adapter_for(surface_of: impl Fn(&wgpu::Instance) -> Option<wgpu::Surface<'static>>) -> Result<Found> {
    for backends in [wgpu::Backends::VULKAN, wgpu::Backends::PRIMARY] {
        let instance = instance(backends);
        let surface = surface_of(&instance);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: surface.as_ref(),
            ..Default::default()
        }));
        match adapter {
            Ok(adapter) => return Ok((instance, adapter, surface)),
            Err(e) => log::warn!("no adapter on {backends:?}: {e}"),
        }
    }
    bail!("no wgpu adapter available")
}

fn shot(path: &str, args: &Args) -> Result<()> {
    let (width, height) = (args.width, args.height);
    let (_instance, adapter, _) = adapter_for(|_| None)?;
    let (device, queue) = pollster::block_on(request_device(&adapter))?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut wr = WorldRenderer::new(Renderer::new(device, queue, format, width, height));

    let started = Instant::now();
    let mut world = match args.scene.as_str() {
        "lineup" => demo::lineup(&mut wr),
        "gameplay" | "showcase" => {
            let mut d = Demo::new(args.seed, 7);
            wr.set_map(&d.map);
            d.run(args.seconds, &mut wr);
            let mut w = d.world.clone();
            if args.scene == "showcase" {
                // A low golden-hour orbit round the followed tank.
                if let Some(t) = d.follow.and_then(|f| w.tanks.iter().find(|t| t.player == f)) {
                    let c = t.pos;
                    let sun = wr.renderer.look.sun_dir;
                    let side = Vec3::new(-sun.z, 0.0, sun.x).normalize();
                    let eye = c + side * 13.0 - Vec3::new(sun.x, 0.0, sun.z).normalize() * 5.0 + Vec3::Y * 3.2;
                    w.camera = Some(Camera::look(eye, c + Vec3::Y * 1.2, 40.0));
                }
            }
            log::info!(
                "sim: {:.0} s simulated in {:.1} s; {} tanks, {} effects particles",
                args.seconds,
                started.elapsed().as_secs_f32(),
                w.tanks.len(),
                wr.fx.particle_count()
            );
            w
        }
        other => bail!("unknown --scene {other:?}"),
    };
    if let Some([ex, ey, ez, tx, ty, tz]) = args.camera {
        world.camera = Some(Camera::look(Vec3::new(ex, ey, ez), Vec3::new(tx, ty, tz), 45.0));
    }
    world.dt = 0.0;

    let texture = wr.renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shot target"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    wr.draw(&view, &world);
    log::info!("frame: {} draw calls, {} instances", wr.renderer.stats.0, wr.renderer.stats.1);

    // Read back, stripping the 256-byte row padding.
    let r = &wr.renderer;
    let unpadded = width * 4;
    let padded = unpadded.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = r.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("shot readback"),
        size: u64::from(padded) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = r.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("shot copy") });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(height) },
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    r.queue.submit(std::iter::once(encoder.finish()));
    let slice = buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = sender.send(res);
    });
    r.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| anyhow!("waiting for the readback: {e}"))?;
    receiver.recv().context("readback callback never ran")?.map_err(|e| anyhow!("mapping readback: {e}"))?;
    let mut pixels = Vec::with_capacity((unpadded * height) as usize);
    {
        let mapped = slice.get_mapped_range().map_err(|e| anyhow!("reading the mapped buffer: {e}"))?;
        for row in 0..height as usize {
            let start = row * padded as usize;
            pixels.extend_from_slice(&mapped[start..start + unpadded as usize]);
        }
    }
    buffer.unmap();
    image::save_buffer(path, &pixels, width, height, image::ExtendedColorType::Rgba8).with_context(|| format!("writing {path}"))?;
    log::info!("wrote {path} ({width}x{height})");
    Ok(())
}

struct Live {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    wr: WorldRenderer,
}

/// The windowed game: straight into Practice (the HTML menus are web-only).
/// Click grabs the mouse, Esc lets it go (Esc again quits).
struct App {
    live: Option<Live>,
    game: Game,
    start: Instant,
    last: Instant,
    frames: u32,
    fps_t: f32,
    grabbed: bool,
    error: Option<anyhow::Error>,
}

impl App {
    fn open(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("TANSK 2026").with_inner_size(LogicalSize::new(1600.0, 900.0)))
                .context("creating the window")?,
        );
        let (_instance, adapter, surface) = adapter_for(|instance| instance.create_surface(Arc::clone(&window)).ok())?;
        let surface = surface.ok_or_else(|| anyhow!("could not create a surface for the window"))?;
        let (device, queue) = pollster::block_on(request_device(&adapter))?;
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| anyhow!("the adapter cannot present to this window"))?;
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &config);
        let wr = WorldRenderer::new(Renderer::new(device, queue, config.format, config.width, config.height));
        self.live = Some(Live { window, surface, config, wr });
        Ok(())
    }

    fn grab(&mut self, on: bool) {
        let Some(live) = &self.live else { return };
        if on {
            let ok = live
                .window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| live.window.set_cursor_grab(CursorGrabMode::Confined))
                .is_ok();
            live.window.set_cursor_visible(!ok);
            self.grabbed = ok;
            if ok {
                self.game.pointer_locked();
            }
        } else {
            let _ = live.window.set_cursor_grab(CursorGrabMode::None);
            live.window.set_cursor_visible(true);
            self.grabbed = false;
            self.game.pointer_lost();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.live.is_none() {
            if let Err(e) = self.open(event_loop) {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            if self.grabbed {
                self.game.controls.mouse_move(delta.0 as f32, delta.1 as f32);
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Focused(false) => {
                self.game.controls.release_all();
                if self.grabbed {
                    self.grab(false);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    // winit's KeyCode names match the DOM's KeyboardEvent.code.
                    let name = format!("{code:?}");
                    if event.state == ElementState::Pressed {
                        if code == KeyCode::Escape {
                            if self.grabbed {
                                self.grab(false);
                            } else {
                                event_loop.exit();
                            }
                            return;
                        }
                        self.game.controls.key_down(&name);
                    } else {
                        self.game.controls.key_up(&name);
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let down = state == ElementState::Pressed;
                if down && !self.grabbed {
                    self.grab(true);
                    return;
                }
                let b = match button {
                    MouseButton::Left => 0,
                    MouseButton::Right => 2,
                    MouseButton::Middle => 1,
                    _ => return,
                };
                self.game.controls.button(b, down);
            }
            WindowEvent::Resized(size) => {
                let Some(live) = self.live.as_mut() else { return };
                live.config.width = size.width.max(1);
                live.config.height = size.height.max(1);
                live.surface.configure(&live.wr.renderer.device, &live.config);
                live.wr.renderer.resize(live.config.width, live.config.height);
            }
            WindowEvent::RedrawRequested => {
                let Some(live) = self.live.as_mut() else { return };
                let now = Instant::now();
                let dt = (now - self.last).as_secs_f32();
                self.last = now;
                // No menus natively: there is no pause card, Esc only frees the mouse.
                self.game.resume();
                let world = self.game.frame((now - self.start).as_secs_f64(), dt, &mut live.wr);
                if self.game.screen() == Screen::End {
                    self.game.start_practice();
                }
                let frame = match live.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
                    wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                        live.surface.configure(&live.wr.renderer.device, &live.config);
                        return;
                    }
                    wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
                    wgpu::CurrentSurfaceTexture::Validation => {
                        self.error = Some(anyhow!("surface validation error"));
                        event_loop.exit();
                        return;
                    }
                };
                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                live.wr.draw(&view, &world);
                live.wr.renderer.queue.present(frame);
                self.frames += 1;
                self.fps_t += dt;
                if self.fps_t > 2.0 {
                    let (calls, inst) = live.wr.renderer.stats;
                    let hint = if self.grabbed { "Esc frees the mouse" } else { "click to drive" };
                    live.window.set_title(&format!(
                        "TANSK 2026 - {:.0} fps, {calls} draws, {inst} instances - {hint}",
                        self.frames as f32 / self.fps_t
                    ));
                    self.frames = 0;
                    self.fps_t = 0.0;
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(live) = &self.live {
            live.window.request_redraw();
        }
    }
}

fn windowed() -> Result<()> {
    let event_loop = EventLoop::new().context("creating the event loop")?;
    let mut game = Game::new();
    game.start_practice();
    let now = Instant::now();
    let mut app = App { live: None, game, start: now, last: now, frames: 0, fps_t: 0.0, grabbed: false, error: None };
    event_loop.run_app(&mut app).context("running the event loop")?;
    match app.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}
