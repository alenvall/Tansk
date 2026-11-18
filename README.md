# Tansk!

Multiplayer tank-arena deathmatch, originally a 2013 Chalmers TDA367 project
(group B17) in Java with Slick2D and Kryonet. Now rewritten in Rust and
playable in the browser.

## Layout

| path | what |
|-|-|
| `crates/tansk-sim` | the game rules, ported from the Java model: tanks, six weapons, five power-ups, Standard and King of the Hill |
| `crates/tansk-server` | axum server: game rooms over WebSocket at `/ws`, open games at `/api/rooms`, the web client at `/` |
| `crates/tansk-web` | the browser client (WASM): menus, the 2D canvas view and the WebGL2 3D view, sound, and the offline playground |
| `assets/` | the original sprites, sounds and map (music converted to MP3) |
| `java/` | the original Java version, untouched |

## Running locally

```sh
cd crates/tansk-web && trunk build --release --dist ../../dist && cd ../..
cargo run --release -p tansk-server     # http://localhost:8080
```

`cargo test -p tansk-sim` runs the game-rule tests.

## Playing

Host a game to get a four-letter code (or share the link), others join with
it. Up to 4 players. The game is 3D by default; V (or Settings) switches to
the original 2D view. The view is only a camera: the rules are the same flat
game, so 2D and 3D players play together.

Up to 4 players. WASD drive, mouse aims, click fires, Enter chats,
Esc back to the menu. Playground (offline): keys 1-6 swap weapons, Q drops a
pickup.

## Deploying

The `Dockerfile` builds both client and server; `railway.json` deploys it on
Railway as a single service (health check `/health`, `PORT` from Railway).

## Changes from the original

Kept faithful where possible (speeds, damage, ammo, timings, 30 ticks/s).
Fixed: Standard mode never scored (eliminations were never counted), the
Rounds setting was unused, bounce shots slid through walls from the right,
and players could get two tanks or spawn-lock.
