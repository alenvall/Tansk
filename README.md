# TANSK 2026

**Build it on the move. Shoot it apart.** A multiplayer tank arena shooter
for the browser: hand-welded desert machines that grow as you grab scrap,
and fall apart as you get shot. Play at https://tansk.alenvall.com.

The 2013 original (Java, Slick2D) is kept in `java/`; the design of the
reimagining is in `docs/DESIGN.md`, the plan in `docs/PLAN.md`.

## Play

- **Modes:** Scrap Brawl (free-for-all), Team Brawl, Convoy (escort or
  stop the fuel tanker), Wellhead (hold the derricks), Last Crew Rolling
  (no respawns, the storm wall closes in), Raid (co-op waves against
  raiders, the War Rig and the Land-Ship). Practice any of them offline.
- **Maps:** The Sink, The Refinery.
- **Upgrades:** three scrap of a kind bolts a part onto your tank; at tier
  2 you pick a branch; two tracks at tier 2 unlock a combo. Every part can
  be shot off.
- **Controls:** WASD drive, mouse aim, click fire, right-click alt fire,
  Shift boost, F special, Z gunsight, H horn, Tab scores, Enter chat.
  Gamepad supported. How to play is on the title screen.

## Layout

| path | what |
|-|-|
| `crates/arena-core` | the game: rapier physics, tanks and parts, weapons, modes, bots, maps, the net protocol. Deterministic, native and wasm |
| `crates/arena-client` | the browser client: wgpu on WebGPU, procedural models and effects, HTML UI, prediction and interpolation. Also builds natively for screenshots |
| `crates/arena-audio` | every sound and the music, synthesized in code |
| `crates/tansk-server` | axum: rooms at `/ws/arena`, quick match, bots, `/api/arena/rooms`, the static client |
| `crates/tansk-sim`, `tansk3d-sim`, `tansk-web` | the earlier ports, no longer served |

## Develop

```sh
cargo test -p arena-core -p arena-client -p arena-audio -p tansk-server
cd crates/arena-client && trunk build --release --dist ../../dist-arena && cd ../..
PORT=8080 TANSK_STATIC_DIR=dist-arena cargo run --release -p tansk-server
cargo run --release -p arena-core --example harness -- [raid|convoy|wellhead|lastcrew] 200
cargo test -p arena-client --release -- --ignored shots   # native screenshots into docs/shots
```

Debug URL flags: `?autoplay=practice|raid|team|convoy|wellhead|lastcrew`,
`&map=refinery`, `&mouse=free`, `&quality=low`, `&matchtime=45`, `&hints=reset`.

## Deploy

The `Dockerfile` builds the client and server; Railway (EU) builds the
`rust-web` branch on every push.
