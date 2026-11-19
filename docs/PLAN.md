# TANSK 2026 — the overnight plan

Light process on purpose: this is a fun project. `docs/DESIGN.md` is the
game; this is how it gets built. Borrowed from brick-racers (renderer
techniques, physics, procedural audio) and from Claude Code Game Studios
(smoke checks, honest handover cards, a tweak panel, seeded replays).

## Layout (new; the old crates are deleted once this ships)

| crate | what |
|-|-|
| `crates/arena-core` | the shared simulation: rapier world, tanks, weapons, pickups, destruction, modes, bots' brains, the net protocol, the arena definitions. No rendering, no sockets; compiles native and wasm |
| `crates/arena-server` | axum: quick-match, rooms with codes, 60 Hz authoritative tick, 30 Hz snapshots, bots filling seats, static files |
| `crates/arena-client` | wgpu renderer (WebGPU on the web, Vulkan natively), input, camera, prediction and interpolation, procedural audio, the HTML UI bridge. Builds for wasm (trunk) **and natively** (`--shot` renders a PNG headlessly, `--bench`) |
| `crates/arena-client/web` | index.html, CSS (salvage style: stencil, sheet metal, gauges), the HTML menus and HUD chrome |
| `tuning/*.ron` | every gameplay and feel number, loaded at start, editable live from a tweak panel (F2) |

## Milestones

**M0 — the risky part first (≈1–2 h).** wgpu running in the browser on
WebGPU: a weathered scrap tank in hard desert sun on sand, orbit camera,
native `--shot` producing the same image. Rapier in wasm stepping a body.
Proves the stack before anything is built on it.

**M1 — vertical slice, deployed (≈6 h in).** The Sink, blocked out;
physics tanks that drive and boost; **modular parts** (stub gun, heavy
cannon, autocannon, sensor mast, armour plate) that bolt on when driven
over and break off when shot; per-part hit boxes; exhaust smoke and
track trails; the low, colliding camera; death and
respawn; Scrap Brawl; bots; quick-match online with prediction +
interpolation; the core renderer (PBR with weathering, cascaded shadows,
sky/env light, haze, tonemap, TAA); a HUD; procedural sounds for guns,
engines and explosions. Live on tansk.alenvall.com.

**M2 — the game (≈14 h in).** All parts (weapons, utility, engine),
chassis classes and weight, track damage, destructible props with
server-synced debris, the other modes (Team Brawl, Wellhead, Water Run,
Convoy, Last Crew Rolling), the juice list in DESIGN.md, renderer polish
(GTAO, bloom, particles: dust/smoke/fire/sparks, decals, heat shimmer,
sandstorm), kill feed, radio callouts, the line-up end screen, menus in the
salvage style, a second map.

**M3 — content and polish (≈20 h).** Maps 3 and 4, Garage,
bot difficulty, balance passes from bot-vs-bot statistics, performance
budget on the web, settings (quality presets, sensitivity, volume),
gamepad, final deploy and a handover card.

Every milestone ends with a **polish pass on what exists** (feel, look,
sound, readability) before the next one starts. Breadth comes last.

## How the work is split

I (the lead) own the contracts: `arena-core`'s public types and the protocol,
the crate boundaries, integration, review, and deploys. Sub-agents work in
parallel on disjoint files against those contracts, a few at a time:

- **Render:** the wgpu renderer, materials, shadows, post, particles, decals.
- **Sim:** tanks on rapier, weapons, pickups, destruction, modes.
- **Net/server:** protocol, rooms, quick-match, prediction/reconciliation.
- **World:** the procedural hard-surface model kit, weathering, and the maps.
- **Bots:** navigation, targeting, behaviour.
- **Audio:** procedural synth (porting brick-racers' approach) on Web Audio.
- **UI:** HTML menus, HUD, end screen, tweak panel, salvage style.
- **QA:** headless smoke checks and bot soak matches after each milestone.

## Checks (every milestone, before saying it works)

1. `cargo test` across the workspace; wasm and native builds without warnings.
2. **Native `--shot`** renders of fixed camera positions per arena (look at
   them; compare with the previous milestone).
3. **Headless browser smoke check:** load the site, start Practice, scripted
   keys, screenshot; console errors; text overlap / off-screen check.
4. **Bot soak:** 8 bots, a seeded 10-minute match on the server sim,
   asserting no panics, sane scores, frame/tick time, memory.
5. **Deploy check:** Railway's deployment status says SUCCESS for the
   pushed commit (not "the site answers"), then the live smoke check.
6. **Handover card** in chat: what was checked and how, what is broken,
   what is guessed, and "not yet played by a person" until it has been.

## When the list runs out: keep going (the user has the week)

Never idle. Pick the next most valuable thing from here, build it, check it,
polish it, ship it:

- **Better bots**: personalities (sniper, brawler, scavenger), squad play,
  difficulty that adapts; bots that use the upgrade branches well.
- **PvE**: *Raider waves* (co-op: hold an outpost against escalating
  waves of raider machines), *Convoy defence* against AI raiders, **boss
  machines** (a land-ship, a crane rig, a war rig with destructible
  modules to shoot off one by one), daily seeded challenges.
- More maps from the world references; time-of-day and sandstorm variants.
- Garage and cosmetics; kill-cam and end-of-match replays; spectator mode.
- Controller support; settings and quality presets; performance on low-end
  laptops; accessibility (colour-blind-safe team colours).
- Sound and juice passes; the title screen; radio voice lines.
- Balance from bot-vs-bot statistics (simulate hundreds of seeded matches
  headless), used to catch things that make matches *not fun* (one strategy
  always wins, matches end in a minute, nobody ever gets tier 3), never to
  sand the fun off.
- Spectacle items: a called-in **air strike** (overpowered, rare, a flare
  and a siren first), spring-board traps, a wrecking-ball crane, a fuel
  tanker that blows the whole street.
