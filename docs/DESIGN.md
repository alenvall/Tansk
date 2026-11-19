# TANSK — design bible (2026 reimagining)

The original Tansk (2013, 2D) and its Rust ports are retired. This is a new
game that keeps the name. **Hard requirements (the user's): runs in a web
browser, has multiplayer.** Everything below is a decision unless marked *open*.

---

## The pitch

**Build it on the move. Shoot it apart.** A physical tank arena shooter in a
sun-bleached desert scrapland, in a hand-made TF2-like style. Every tank is a
welded-together junk machine. **Grab three scrap of a kind and an upgrade
bolts onto your tank in real time**: a bigger gun on your turret, a sensor
mast rising from your hull, plates clanking onto your flanks. Everything you
bolt on can be **shot off**. You read an enemy at a glance: that one has
the Big Bore and a mast; take out the mast and it's half-blind.

Matches of 6–10 minutes, up to 8 crews, bots fill empty seats.

## North star (read this before adding anything)

This document is a starting point, not a contract: if more scrap plays
better, add scrap; if more upgrades play better, add upgrades. What it must
keep is **one direction**. Not a tech demo, not a bucket of everything:
everything only looks good when it ties together.

**Three pillars. A feature earns its place by serving one of them, in the
world's own terms.**

1. **Your machine grows.** The match is the story of your tank getting
   bigger, meaner and more patched-up, and of other crews tearing bits off
   it. Everything that matters shows *on the tank*.
2. **The desert tells you things.** Exhaust smoke, track ruts in the sand,
   dust plumes, gunfire, the silhouette of a gun over a dune: you read the
   battlefield from the world, not from UI.
3. **Junk, sweat and sparks.** Every effect is something a hand-welded junk
   machine and its crew would actually do: clanks, soot, sparks, a hatch
   popping open for a wrench. No holograms, no sci-fi glow, no floating
   numbers, no loot-box chrome.

**Fun beats balance, every time.** This is a jump-in-and-play-with-friends
game, not an esports title. If an air strike is wildly overpowered but looks
awesome, it goes in (make it rare, telegraphed and funny to survive). Big,
wacky, spectacular moments are the point; balance only stops things that
make a match not fun.

**Polish over breadth.** One map, three upgrade tracks and one mode that
feel great beat four of each that don't. Each milestone ends with a polish
pass on what exists before anything new starts.

## The world and its lore

- Mad Max-inspired, grounded: a generation after the water ran out, the
  **Sink** (a dried-up inland sea) is littered with wrecks, refineries and
  dead highways. Crews of **Scrappers** fight over the last **fuel** and
  **water**, rebuilding their machines from what they tear off each other.
- Every mechanic is explained by the world: parts are salvage, fuel powers
  boosts and abilities, water and fuel are what the modes fight over.
- Two factions for team modes: the **Rustbacks** (oxide orange, welded
  plate) and the **Dust Saints** (bleached teal, scavenged military kit).
  Free-for-all crews spray their own colours.
- The voice of the game is a crackling **radio caller** (text callouts at
  first, voice later): dry, terse, gallows humour. Not a sports announcer.

## Art direction: stylized hard-surface, TF2 light

**Expressive, colourful, hand-made machines with soul**, from the smoke
clouds to the rivets to the title screen. Mad Max is the *setting* (a
desert fought over for fuel and water), not the art: the machines are
built well, not a pile of DIY junk.

The user's references (kept outside the repo; agents may view them in the
session's images folder, `1.png`–`5.png`; the world's are `6.png`–`9.png`) and what each means:
- **The target (`3.png`)**: stylized hard-surface. Bevelled armour plates
  with panel lines and rows of rivets, metal trim bands, a chunky barrel
  with a reinforced muzzle, crates and boxes bolted on, the paint in a
  saturated team colour with a soft camo pattern, edges worn to metal.
  Clean, defined, hand-painted-looking.
- **Parts and pickups (`1.png`, `2.png`)**: a modular top (turret) and
  bottom (hull), and **each upgrade a distinct silhouette** (a scope
  turret, a twin barrel, a big bore); pickups as iconic objects (a wrench
  for repair, a shield wedge, a turbo). Bright and readable. **But less
  cutesy**: less squat and rounded than these, closer to `3.png`'s
  proportions.
- **Where the tanks sit: between `1.png` and `3.png`.** Exaggerated
  features (an oversized turret and muzzle, chunky tracks, a readable
  silhouette per part) with `3.png`'s defined hard-surface detail and
  grown-up proportions. Never cutesy.
- **Scrap character (`5.png`)**: a bit too cartoonish as a whole, but its
  ideas are the upgrade parts' vocabulary: a ram of overlapping riveted
  plates and spikes, makeshift parts (a barrel strapped on as a drum, a bent
  exhaust stack), bevelled octagonal wheels. Use these ideas with `3.png`'s
  restraint.
- **The thing to avoid (`4.png`)**: real-life geometry with flat shading
  and no definition. Ours is designed shapes, bevels, panel lines and
  painted light, never untextured facets.

Rules:
- **Shapes**: designed, slightly exaggerated hard-surface. Every part a
  silhouette you can read across the map. Bevels on every edge (they catch
  the rim light), panel lines, rivets and trim bands as standard kit
  details, chamfered rather than rounded, a hint of chunky, never toy-cute.
- **Paint**: a saturated team colour as the main body paint (Rustback
  orange, Dust Saint teal; FFA picks from the paint set) with a soft
  two-tone camo, darker metal trims, bare-metal edge wear, dust on top
  faces, rust and soot in crevices as broad painted shapes.
- **Shading**: TF2's illustrative model (half-Lambert through a warm-cool
  ramp, strong rim light, ambient cube, soft broad specular on metal).
- **World**: the desert scrapland in the same language: chunky stylized
  rock formations with painted strata (not flat-shaded facets), sand with
  soft ripples, wrecks and shacks built from the same plates, rivets and
  trims as the tanks.
- **Palette**: warm sand and rock, bright team colours, metal greys; fire,
  muzzle flash and pickups as the brightest accents.
- **Presentation**: the title screen, menus and HUD share the machines'
  language: painted metal plates, rivets, stencilled type, hazard stripes,
  gauges.

## The world's look: the lighting carries it

The user's environment references (`6.png`–`9.png` in the session's images
folder, kept outside the repo) are **the target for the world**: a desert
outpost and highway with wrecks. **The lighting is the thing that carries
the game**, then the gameplay.

- **Light**: a warm, lowish sun, hot air glowing; strong atmospheric
  scattering and aerial perspective (the distance washes out into warm
  haze, the sky goes from deep blue overhead to a bright warm horizon);
  generous bloom on bright sand and sky; warm bounce light filling the
  shadows; ambient occlusion grounding every prop. The references are a
  touch too soft: **keep shadows a little crisper** (defined edges, still
  warm-tinted, never black).
- **Polygon budget**: the terrain, cliffs and rocks are **lower poly,
  faceted** (big flat-shaded planes on mesas and boulders, sand dunes as
  broad soft facets); props are cleaner and more defined; **tanks are the
  most detailed things on screen** (bevels, rivets, panel lines). Detail
  goes where the player looks.
- **Colour**: warm sand, orange-red mesas with strata, dark asphalt with
  white lane lines; **accent colours that pop**: red, blue and olive
  shipping containers, yellow silos and fuel tanks, yellow and black hazard
  barriers, bright green cacti and shrubs, blue tarps, team colours.
- **Set dressing vocabulary**: shipping containers (stacked, as walls),
  sandbag lines, razor wire, Czech hedgehogs, concrete barriers, silos and
  fuel tanks, quonset tents, watchtowers, windmills, power lines on poles,
  wrecked rusted cars, crates, tyres, cinder blocks, pallets, corrugated
  sheets, a cracked highway, mesas and boulders, cacti.

## Pace: heavy, positional, snappy

Not World of Tanks (too slow), not a car shooter (too floaty and fast).
Closer to Arma than Quake, with the snap of a shooter:

- **Tanks feel heavy**: they take a moment to get going and to stop, hulls
  turn deliberately, turrets traverse at a limited rate (you can be
  out-turned up close), recoil rocks the tank. Top speed is moderate; boost
  is a short burst, not a mode.
- **Positioning matters**: side and rear armour are weaker than the front,
  so flanking pays; hull-down behind low cover protects tracks; high ground
  sees over walls.
- **No corner-popping**: main guns reload in 2–4 s; accuracy blooms while
  moving and settles after stopping; peeking out to fire and back costs
  you because you can't fire again soon, and your exhaust smoke and dust
  give you away.
- **Snappy where it counts**: inputs answer immediately (the throttle
  bites, the turret starts turning, the shot fires the frame you click),
  hits land with weight and sound, and time-to-kill is several hits rather
  than one, so fights have a shape.

## Upgrades in real time (the heart of the game)

Your tank grows over the match, right in front of everyone.

### Scrap: the loop
- The map is littered with **scrap** in four kinds, each a chunky,
  readable pickup:
  - **Steel** (grey plates): armour and chassis.
  - **Powder** (red shell crates): guns and damage.
  - **Volts** (blue coils and batteries): sensors and energy weapons.
  - **Fuel** (yellow jerrycans): engine, speed and boost.
- **Three of a kind = an upgrade.** It's instant: the third pickup
  triggers it. A part flies up and **bolts onto your tank** (a weld flash,
  sparks, a clank), and everyone sees it.
- Each kind is an **upgrade track** with tiers (three of a kind per tier).
  At tier 2 the track **branches**: a quick "pick one of two" pops up
  (keys 1/2 or click, 4 seconds, else the default), so two tanks with the
  same scrap end up different.
  - **Powder**: stub gun → heavy cannon → *Big Bore* (huge slow shells,
    splash), *Twin Barrel* (shotgun-style double blast, close range) or
    *Long Gun* (long barrel and a scope: fast flat shots, a deeper zoom,
    for range) → tier 3 of the chosen gun.
  - **Volts**: sensor mast (higher sight point, see over walls) →
    *Coilgun* (charge, pierce) or *Tesla Spire* (chains between nearby
    enemies) → tier 3.
  - **Steel**: flank plates → *Bulwark* (front slab, ram plow, very heavy)
    or *Spaced Armour* (all-round, lighter) → tier 3.
  - **Fuel**: nitro tank (boost) → *Afterburner* (long boosts, ramming
    damage) or *Twin Engines* (top speed, faster turret) → tier 3.
- **Combos**: having tier 2+ in two tracks unlocks a **combo part** on the
  next upgrade, e.g. Powder + Fuel = *Flamethrower nozzle*, Volts + Steel =
  *Shield generator*, Powder + Volts = *Rocket rack with lock-on*,
  Steel + Fuel = *Spiked ram*.
- **Rare finds**: occasional golden crates and the wrecks of big machines
  hold a unique part outright (*Mortar*, *Harpoon*, *Smoke launcher*, *Mine
  layer*, *Repair rig*).

### Shooting it off
- **Every bolted-on part has its own health and hit box.** Shoot it enough
  and it breaks off, clatters to the ground, and you lose that tier (and
  what it gives). Big guns are big targets. Tracks can be blown off too
  (crippled until field-repaired).
- **Losing a part drops its scrap** where it fell; anyone can grab it.
- **Death**: your tank bursts apart, and **most of your scrap spills out**
  for everyone. You respawn with your first tier in each track, so you can
  come back.
- **Bounty**: a tank with many upgrades glows hotter on everyone's radio
  ("big one heading north"), so the snowball is everyone's target.
- **Weight**: every part adds mass; a fully built tank is a monster but
  slow and easier to hit.

### Customisation
- **Garage**: pick a chassis (Scout, Brawler, Hauler: speed vs armour vs
  how many upgrades it carries), paint scheme, crew emblem, hood ornament,
  and a horn.
- **Which way your tank grows** is the in-match customisation: the branch
  picks and combos.

## Combat and feel

- Physical: real mass, suspension, traction, ramming, tipping on dunes,
  getting stuck and rocking free. Explosions push tanks and debris.
- Ballistic shells with drop and travel time; the machine gun and coilgun
  are hitscan.
- **Juice, in the world's terms** (pillar 3):
  - hit markers and a distinct metallic **plonk / clang** per part hit,
    a dull thud on armour, a crunch when a part breaks off;
  - small explosion puffs, sparks and flying bolts on hits, ricochet
    pings off steep plates;
  - camera shake by proximity, recoil that rocks the hull and kicks the
    camera, muzzle blast kicking up sand, shell casings;
  - parts and wrecks flying off with physics, burning wrecks with black
    smoke, scorch marks;
  - **repairs are the crew's**: the Repair kit (a wrench pickup) and the
    field-repair of a blown track pop a hatch, a crew member leans out with
    a wrench, sparks, and the part is patched (visibly welded over);
  - a slow beat on the final kill; the winning machines lined up at the
    end, with every scar and bolted-on part on show.

## Reading movement, and peeking

Third-person over-the-shoulder camera, hold to zoom into the gunsight. The
camera collides with the world and sits low behind the turret.

- **Peeking is a level-design problem**: cover is either low enough that
  a tank behind it still shows (you can't hide behind it, only take less
  damage), or tall enough that the camera can't see over it either
  (2.5 m and up: a single shipping container, ship hulls, cliff walls). No in-between heights. Some cheese
  around corners is fine; don't over-engineer it.
- **The desert gives movement away** (pillar 2):
  - **exhaust smoke** from every tank, darker and thicker when boosting or
    damaged, rising above cover;
  - **track ruts and dust trails** in the sand that linger and fade;
  - **dust plumes** behind fast tanks, visible from far away;
  - **sandstorms** that blind everyone and blow trails away;
  - gunfire with muzzle smoke and positional sound; the radio calls out
    big machines.
- Nameplates and health bars only with line of sight.

## Modes (fighting over resources)

1. **Scrap Brawl**: free-for-all, most kills.
2. **Team Brawl**: Rustbacks vs Dust Saints.
3. **Wellhead**: hold derricks to pump fuel; first team to fill its tank wins.
4. **Water Run**: carry water canisters from the centre to your base;
   carriers are slow and can't boost.
5. **Convoy**: one team escorts a tanker truck along a highway, the other
   stops it.
6. **Last Crew Rolling**: no respawns, a sandstorm closes in.
7. **Practice**: you and bots, offline in the browser.

## Maps (bigger environments)

Large, with verticality and destructible cover:
1. **The Sink**: a dry lake bed with beached rusted ships, dunes, a
   collapsed pier. *First, and the showcase.*
2. **Rust Belt**: a maze of scrapyard stacks, crushed cars and cranes.
3. **Refinery**: derricks, pipe runs, tanks that explode in chains.
4. **Highway 9**: a collapsed overpass, a gas station, a ghost town.

Destructible: corrugated walls, shacks, stacked wrecks and containers,
fences, towers, fuel tanks (explode), oil drums (roll and burn). Server-
authoritative for anything that blocks movement or sight.

## Bots

Server-side crews for every empty seat and Practice: navigation over a
coarse grid, sight-based target selection (they obey the no-peeking rule),
leading shots, choosing and stripping parts, difficulty levels.

## Presentation flow

Site → **Play** (quick match) / **Host** (code, mode, map) / **Practice** /
**Garage** (pick a chassis and paint). Lobby: crews idle at the start line.
Match → end screen with the winning machines in a line-up (their actual
bolted-on loadouts on show) → next vote.

## Technology

- **Client:** Rust → WebAssembly, **wgpu on WebGPU**, also built natively for
  development and headless screenshots (`--shot`).
- **Renderer:** forward PBR in WGSL from brick-racers' techniques: GGX,
  cascaded shadows, sky model, pre-filtered environment, GTAO, bloom,
  filmic tonemap, temporal AA, weathering, GPU particles (dust, smoke, fire,
  sparks), decals, height fog/haze, sandstorm.
- **Physics:** rapier3d, the same crate on server and client.
- **Netcode:** authoritative server at 60 Hz, snapshots at 30 Hz,
  interpolation, client
  prediction for your own tank, lag-compensated hits.
- **Audio:** procedural synthesis (brick-racers' approach) via Web Audio,
  positional.
- **UI:** HTML/CSS for menus and HUD chrome, canvas for the world.
- **Server:** axum, rooms, quick-match, bots; Railway in Europe.
