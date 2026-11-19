//! TANSK 2026 client.
//!
//! * Web (wasm32, built with trunk): renders into `#tansk-canvas` on WebGPU.
//! * Native: a window on Vulkan, or `--shot out.png [--scene gameplay|
//!   showcase|lineup] [--width W --height H] [--seconds S]` to render one
//!   frame headlessly and write a PNG.
//!
//! The renderer's game-facing API is [`world::WorldRenderer`] and
//! [`world::RenderWorld`]; [`game`] is the playable loop (menus, net,
//! input, camera, sound, HUD) and [`demo`] the self-playing title backdrop.

// The lineup and shot compositions are native-only tools; on the web
// their builders are compiled but unused.
#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

mod anim;
mod bloom;
mod camera;
mod demo;
mod effects;
mod game;
mod meshkit;
mod objectives;
// Wired into the game loop next; until then not every export is used.
#[allow(unused_imports)]
mod net;
mod parts;
mod props;
mod props_refinery;
mod props_rustbelt;
mod props_hazards;
mod renderer;
mod shapes;
mod ships;
mod tank;
mod terrain;
mod ui;
mod world;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    native::main();
}

#[cfg(target_arch = "wasm32")]
fn main() {
    web::main();
}
