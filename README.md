# Tansk!

Multiplayer tank-arena deathmatch, originally a 2013 Chalmers TDA367 project
(group B17) in Java with Slick2D and Kryonet. Now rewritten in Rust and
playable in the browser.

## Layout

| path | what |
|-|-|
| `crates/tansk-sim` | the game rules, ported from the Java model: tanks, six weapons, five power-ups, Standard and King of the Hill |
| `crates/tansk3d-sim` | Tansk 3D's rules: hilly terrain, ballistic shells, the same weapons, power-ups and modes |
| `crates/tansk-server` | axum server: game rooms over WebSocket at `/ws` (2D) and `/ws3d` (3D), open games at `/api/rooms` and `/api/rooms3d`, the web client at `/` |
| `crates/tansk-web` | the browser client (WASM): the 2D game at `/` and the first-person WebGL2 game at `/3d`, menus, sound, offline playgrounds |
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
it. Up to 4 players. Up to 4 players. WASD drive, mouse aims, click fires, Enter chats,
Esc back to the menu. Playground (offline): keys 1-6 swap weapons, Q drops a
pickup.

## Tansk 3D

`/3d` is a separate game: first person from your turret over hilly terrain,
shells arc under gravity (the HUD shows where they land). Click to grab the
mouse; mouse aims, WASD drives, click fires, Tab scores, Enter chats, Esc
frees the mouse.

## Deploying

The `Dockerfile` builds both client and server; `railway.json` deploys it on
Railway as a single service (health check `/health`, `PORT` from Railway).

## Changes from the original

Kept faithful where possible (speeds, damage, ammo, timings, 30 ticks/s).
Fixed: Standard mode never scored (eliminations were never counted), the
Rounds setting was unused, bounce shots slid through walls from the right,
and players could get two tanks or spawn-lock.
