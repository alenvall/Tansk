//! The game-facing renderer API.
//!
//! The game layer (net, practice, UI) fills a [`RenderWorld`] every frame,
//! usually with [`RenderWorld::from_snapshot`], feeds simulation events to
//! [`WorldRenderer::event`], and calls [`WorldRenderer::draw`]. Everything
//! below that (meshes, instancing, effects, lights, animation) is here and
//! in [`crate::renderer`].

use std::collections::HashMap;

use arena_core::map::{Cover, GroundMarkKind, Map, ObstacleKind};
use arena_core::snapshot::{ShotKind, Snapshot};
use arena_core::parts::VehicleKind;
use arena_core::{Event, GunKind, PartKind, PickupKind, PlayerId, PropKind, ScrapKind};
use glam::{Mat4, Quat, Vec2, Vec3, Vec4};

use crate::camera::Camera;
use crate::effects::Fx;
use crate::meshkit::{MeshBuilder, Paint};
use crate::parts;
use crate::props;
use crate::props_refinery as refinery;
use crate::renderer::{Instance, Light, MeshId, Renderer};
use crate::shapes::hash;
use crate::tank;

/// The paint set: team or free-for-all colours by player.
pub const PAINT_SET: [u32; 8] = [tank::RUSTBACK, tank::DUST_SAINT, 0xc8322a, 0xe0b020, 0x2f6ad0, 0x5aa03a, 0x7a4ab0, 0xd8d4c8];

/// One tank, as the renderer wants it.
#[derive(Clone, Debug)]
pub struct TankView {
    pub player: PlayerId,
    pub colour: u32,
    pub pos: Vec3,
    pub rot: Quat,
    pub turret_yaw: f32,
    pub gun_pitch: f32,
    /// Wheel compression in metres, left side front to back, then right.
    pub wheels: [f32; 10],
    /// Track surface speeds (left, right), m/s.
    pub track_speed: [f32; 2],
    pub gun: PartKind,
    pub parts: Vec<PartKind>,
    pub alive: bool,
    pub boosting: bool,
    pub burning: bool,
    /// Engine load 0..1 (exhaust and dust).
    pub throttle: f32,
    pub speed: f32,
    /// The chassis class (picks the hull and turret model).
    pub class: arena_core::ChassisClass,
}

impl TankView {
    pub fn hull_transform(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.rot, self.pos)
    }
    pub fn turret_transform(&self) -> Mat4 {
        self.hull_transform() * tank::turret_local(self.turret_yaw)
    }
    pub fn gun_transform(&self) -> Mat4 {
        self.turret_transform() * tank::gun_local(self.gun_pitch)
    }
    /// Muzzle tip in world space (for the game layer's aim and flashes).
    #[allow(dead_code)]
    pub fn muzzle(&self) -> Vec3 {
        self.gun_transform().transform_point3(Vec3::X * parts::muzzle_x(self.gun))
    }
}

#[derive(Clone, Debug)]
pub struct PropView {
    pub id: u16,
    pub kind: PropKind,
    pub pos: Vec3,
    pub rot: Quat,
}

#[derive(Clone, Debug)]
pub struct PickupView {
    pub id: u16,
    pub kind: PickupKind,
    pub pos: Vec3,
}

#[derive(Clone, Debug)]
pub struct ShellView {
    pub pos: Vec3,
    pub vel: Vec3,
    pub big: bool,
    pub kind: ShotKind,
}

/// An air strike: the flare at `target`, then the run from `from` to `to`.
#[derive(Clone, Debug)]
pub struct StrikeView {
    pub id: u16,
    pub target: Vec3,
    pub from: Vec3,
    pub to: Vec3,
    /// Seconds until the first bomb (0 once the run is under way).
    pub delay: f32,
}

/// Everything the renderer draws in one frame, besides the static map.
#[derive(Clone, Debug, Default)]
pub struct RenderWorld {
    pub camera: Option<Camera>,
    /// Seconds since start (animation, shake noise).
    pub time: f32,
    /// Seconds since the last frame.
    pub dt: f32,
    pub tanks: Vec<TankView>,
    pub props: Vec<PropView>,
    pub pickups: Vec<PickupView>,
    pub shells: Vec<ShellView>,
    /// Smoke screens: (id, centre, radius, life 0..1).
    pub smokes: Vec<(u16, Vec3, f32, f32)>,
    /// Static map pieces to hide: (obstacle index, box index or 255 = all).
    pub removed: Vec<(u16, u8)>,
    /// Mines: (id, position, armed).
    pub mines: Vec<(u16, Vec3, bool)>,
    pub strikes: Vec<StrikeView>,
    /// Harpoon cables: (from, to).
    pub tethers: Vec<(Vec3, Vec3)>,
    /// Shield bubbles: (player, strength 0..1).
    pub shields: Vec<(PlayerId, f32)>,
    /// Tanks whose flamethrower is jetting.
    pub flaming: Vec<PlayerId>,
    /// Non-tank chassis: (player, vehicle, wheel compressions in metres).
    /// Tanks not listed here are ordinary tanks.
    pub vehicles: Vec<(PlayerId, VehicleKind, Vec<f32>)>,
    /// Raider Waves: the outpost (centre, zone radius, health 0..1).
    pub raid: Option<(Vec3, f32, f32)>,
    /// Bosses whose core is exposed (glowing and smoking).
    pub exposed_cores: Vec<PlayerId>,
    /// Hull health 0..1 per tank (battle damage, smoke, fire).
    pub health: Vec<(PlayerId, f32)>,
    /// Part health 0..1 (scorch, sparks before they break off).
    pub part_health: Vec<(PlayerId, PartKind, f32)>,
    /// Convoy: chevrons on the road ahead of the truck (ground point, yaw, fade 0..1).
    pub route: Vec<(Vec3, f32, f32)>,
    /// Boss turrets aimed on their own: (boss, part, yaw, pitch) relative to its hull.
    pub turrets: Vec<(PlayerId, PartKind, f32, f32)>,
    /// Wellhead: the derricks (ground point, holder 0/1 or 255, ring -1 Rustbacks
    /// ..= 1 Dust Saints, contested) and the ring radius.
    pub wells: Vec<(Vec3, u8, f32, bool)>,
    pub well_radius: f32,
    /// Last Crew Rolling: the sandstorm wall (centre x/z, radius).
    pub storm_wall: Option<(Vec2, f32)>,
}

fn gun_part(g: GunKind) -> PartKind {
    match g {
        GunKind::Stub => PartKind::StubGun,
        GunKind::HeavyCannon => PartKind::HeavyCannon,
        GunKind::BigBore => PartKind::BigBore,
        GunKind::TwinBarrel => PartKind::TwinBarrel,
        GunKind::LongGun => PartKind::LongGun,
        // The war rig's guns are a turret part; its gun frame carries a stub.
        GunKind::RigCannons | GunKind::DeckGun => PartKind::StubGun,
    }
}

impl RenderWorld {
    /// Fills the dynamic world from a snapshot. Colours come from the
    /// paint set by player id unless `colour_of` overrides it.
    pub fn from_snapshot(snap: &Snapshot, colour_of: &dyn Fn(PlayerId) -> u32) -> RenderWorld {
        let ray = 0.85;
        let tuning = arena_core::Tuning::default();
        // Team play: team 0 Rustback orange; team 1 the Dust Saints' teal in
        // Team Brawl, or the raider scheme in Raider Waves (a raid snapshot
        // says which); free-for-all (255) by player.
        let raid = snap.raid.is_some();
        let team_colour = |t: &arena_core::snapshot::TankSnap| match t.team {
            1 if raid => tank::RAIDER,
            1 => tank::DUST_SAINT,
            0 => tank::RUSTBACK,
            _ => colour_of(t.player),
        };
        let tanks = snap
            .tanks
            .iter()
            .map(|t| {
                let mut wheels = [0.15f32; 10];
                for (w, c) in wheels.iter_mut().zip(&t.wheels) {
                    *w = *c as f32 / 255.0 * ray;
                }
                let v = Vec3::from(t.linvel);
                TankView {
                    class: t.chassis,
                    player: t.player,
                    colour: team_colour(t),
                    pos: t.position(),
                    rot: t.rotation(),
                    turret_yaw: t.turret_yaw,
                    gun_pitch: t.gun_pitch,
                    wheels,
                    track_speed: [t.track_speed[0] as f32 / 100.0, t.track_speed[1] as f32 / 100.0],
                    gun: gun_part(t.gun),
                    parts: t.parts.iter().map(|p| p.kind).filter(|k| !parts::is_gun(*k)).collect(),
                    alive: t.alive,
                    boosting: t.boosting,
                    burning: t.burning,
                    throttle: ((t.track_speed[0].abs() + t.track_speed[1].abs()) as f32 / 1400.0).min(1.0),
                    speed: Vec2::new(v.x, v.z).length(),
                }
            })
            .collect();
        RenderWorld {
            camera: None,
            time: 0.0,
            dt: 0.0,
            tanks,
            props: snap.props.iter().map(|p| PropView { id: p.id, kind: p.kind, pos: Vec3::from(p.pos), rot: arena_core::snapshot::dq_rot(p.rot) }).collect(),
            pickups: snap.pickups.iter().map(|p| PickupView { id: p.id, kind: p.kind, pos: Vec3::from(p.pos) }).collect(),
            shells: snap
                .projectiles
                .iter()
                .map(|p| ShellView { pos: Vec3::from(p.pos), vel: Vec3::from(p.vel), big: matches!(p.gun, GunKind::BigBore), kind: p.shot })
                .collect(),
            smokes: snap.smokes.iter().map(|s| (s.id, Vec3::from(s.pos), s.radius, s.life as f32 / 255.0)).collect(),
            removed: snap.removed.iter().map(|r| (r.obstacle, r.box_index)).collect(),
            mines: snap.mines.iter().map(|m| (m.id, Vec3::from(m.pos), m.armed)).collect(),
            strikes: snap
                .strikes
                .iter()
                .map(|s| StrikeView {
                    id: s.id,
                    target: Vec3::from(s.target),
                    from: Vec3::from(s.from),
                    to: Vec3::from(s.to),
                    delay: s.ticks_left as f32 / 60.0,
                })
                .collect(),
            tethers: snap.tethers.iter().map(|t| (Vec3::from(t.from), Vec3::from(t.to))).collect(),
            vehicles: snap
                .tanks
                .iter()
                .filter(|t| t.vehicle != VehicleKind::Tank)
                .map(|t| {
                    let c = arena_core::tank::chassis_of(&tuning, t.vehicle);
                    let ray_len = c.suspension_travel + c.wheel_radius;
                    (t.player, t.vehicle, t.wheels.iter().map(|w| *w as f32 / 255.0 * ray_len).collect())
                })
                .collect(),
            raid: snap.raid.as_ref().map(|r| (Vec3::from(r.outpost), r.radius, r.health as f32 / r.max_health.max(1) as f32)),
            exposed_cores: snap.bosses.iter().filter(|b| b.core_exposed).map(|b| b.player).collect(),
            health: snap.tanks.iter().map(|t| (t.player, t.health as f32 / t.max_health.max(1) as f32)).collect(),
            part_health: snap.tanks.iter().flat_map(|t| t.parts.iter().map(move |p| (t.player, p.kind, p.health as f32 / 255.0))).collect(),
            shields: snap.tanks.iter().filter(|t| t.alive && t.shield > 0).map(|t| (t.player, t.shield as f32 / 255.0)).collect(),
            flaming: snap.tanks.iter().filter(|t| t.alive && t.flaming).map(|t| t.player).collect(),
            route: Vec::new(),
            turrets: snap.bosses.iter().flat_map(|b| b.turrets.iter().map(move |t| (b.player, t.part, t.yaw, t.pitch))).collect(),
            wells: snap
                .wellhead
                .as_ref()
                .map(|w| w.wells.iter().map(|x| (Vec3::from(x.pos), x.owner.unwrap_or(255), x.progress, x.contested)).collect())
                .unwrap_or_default(),
            well_radius: snap.wellhead.as_ref().map_or(0.0, |w| w.radius),
            storm_wall: snap.storm_circle.as_ref().map(|c| (Vec2::from(c.center), c.radius)),
        }
    }
}

/// The default colour for a player.
pub fn paint_for(player: PlayerId) -> u32 {
    PAINT_SET[player as usize % PAINT_SET.len()]
}

/// Meshes built on first use and uploaded once. Pinned meshes (tanks,
/// parts, effects props) live for the whole session; scene meshes (map
/// pieces, keyed by sizes that change with every map and seed) belong to
/// the open scene and are freed with it.
pub struct Library {
    ids: HashMap<String, MeshId>,
    scene_ids: HashMap<String, MeshId>,
}

impl Library {
    pub fn new() -> Self {
        Self { ids: HashMap::new(), scene_ids: HashMap::new() }
    }

    /// A mesh shared within the current scene only (recorded in its scope).
    pub fn scene(&mut self, r: &mut Renderer, key: &str, build: impl FnOnce() -> MeshBuilder) -> MeshId {
        if let Some(id) = self.scene_ids.get(key) {
            return *id;
        }
        let id = r.add_mesh(&build());
        self.scene_ids.insert(key.to_string(), id);
        id
    }

    /// Forgets one scene mesh (freed early).
    fn forget_scene_mesh(&mut self, id: MeshId) {
        self.scene_ids.retain(|_, v| *v != id);
    }

    /// Forgets the scene meshes (the scene that owned them is being freed).
    fn clear_scene(&mut self) {
        self.scene_ids.clear();
    }

    /// Pinned meshes held (for leak checks).
    #[allow(dead_code)]
    pub fn pinned(&self) -> usize {
        self.ids.len()
    }

    pub fn get(&mut self, r: &mut Renderer, key: &str, build: impl FnOnce() -> MeshBuilder) -> MeshId {
        if let Some(id) = self.ids.get(key) {
            return *id;
        }
        let id = r.add_mesh_pinned(&build());
        self.ids.insert(key.to_string(), id);
        id
    }
}

/// The burnt paint of a wreck.
const WRECK_CHARRED: u32 = 0x2c2622;

/// The team colour tank meshes are built with when an instance tint
/// repaints them (any colour works; this one shows up if a tint is missed).
const TINTED: u32 = 0xff00ff;

/// Side of a dressing chunk cell, m.
const CHUNK_CELL: f32 = 64.0;
/// Static meshes placed at most this many times are merged into chunks
/// (one-off obstacle pieces: hulls, walls cut to size, towers). Meshes
/// placed more often stay instanced: they already cost one draw.
const CHUNK_MAX_COPIES: usize = 4;

/// Static pieces merged into one mesh per map cell, to save draw calls.
struct Chunk {
    mesh: MeshId,
    /// Pieces that can be shot away: owner, first index, their indices.
    owned: Vec<((u16, u8), u32, Vec<u32>)>,
    /// Which of `owned` are blanked out (removed) on the GPU right now.
    blanked: Vec<bool>,
}

/// A static piece of the map, and which obstacle box it belongs to.
struct StaticPiece {
    instance: Instance,
    owner: Option<(u16, u8)>,
}

/// Per-tank animation state the renderer keeps between frames.
#[derive(Default, Clone)]
struct TankAnim {
    tread: [f32; 2],
    seen: bool,
}

/// The renderer with its mesh library, static map and effects.
pub struct WorldRenderer {
    pub renderer: Renderer,
    pub lib: Library,
    pub fx: Fx,
    statics: Vec<StaticPiece>,
    anims: HashMap<PlayerId, TankAnim>,
    map_occluders: Vec<Vec4>,
    /// Static contact-shadow blobs: (ground point, radius).
    blobs: Vec<Vec4>,
    /// Crane jib tips the wrecking-ball ropes hang from.
    crane_anchors: Vec<Vec3>,
    /// Land-Ship stern ramps: when each ship last dropped a buggy (renderer clock).
    ramps: HashMap<PlayerId, f32>,
    /// The renderer clock (`RenderWorld::time` of the last frame).
    clock: f32,
    /// Pumpjacks to animate: (obstacle, frame, phase).
    pumpjacks: Vec<(u16, Mat4, f32)>,
    /// When each air strike's run began (renderer clock).
    strike_runs: HashMap<u16, f32>,
    /// Chassis sizes, suspension and turret pivots for every vehicle.
    tuning: arena_core::Tuning,
    /// Meshes owned by the current static scene (map, line-up, podium),
    /// freed when the scene is replaced.
    scene_meshes: Vec<MeshId>,
    /// The current map (ground heights for markers).
    map: Option<Map>,
    /// Wreck props, kept past the sim's lifetime and then sunk:
    /// prop id -> (pose, seconds since first seen, seconds since gone).
    wrecks: HashMap<u16, (Mat4, f32, Option<f32>)>,
    /// The static scene must be handed to the renderer again (it changed).
    statics_dirty: bool,
    /// The removed-pieces list the renderer's static scene was built with.
    statics_removed: Vec<(u16, u8)>,
    /// Merged one-off map pieces (see `chunk_statics`).
    chunks: Vec<Chunk>,
    /// CPU ms spent in the last `draw`: effects sim, instance build,
    /// sprite build, renderer (cull + pack + encode).
    pub prof: [f64; 4],
    /// How many dressing props the last `set_map` placed.
    pub dressed: usize,
}

/// A millisecond clock for the frame profile.
pub fn clock_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window().and_then(|w| w.performance()).map(|p| p.now()).unwrap_or(0.0)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64() * 1000.0
    }
}

impl WorldRenderer {
    pub fn new(renderer: Renderer) -> Self {
        Self {
            renderer,
            lib: Library::new(),
            fx: Fx::new(),
            statics: Vec::new(),
            anims: HashMap::new(),
            map_occluders: Vec::new(),
            blobs: Vec::new(),
            crane_anchors: Vec::new(),
            pumpjacks: Vec::new(),
            ramps: HashMap::new(),
            clock: 0.0,
            strike_runs: HashMap::new(),
            tuning: arena_core::Tuning::default(),
            scene_meshes: Vec::new(),
            map: None,
            wrecks: HashMap::new(),
            prof: [0.0; 4],
            statics_dirty: true,
            statics_removed: Vec::new(),
            chunks: Vec::new(),
            dressed: 0,
        }
    }

    /// Builds the static map: terrain, obstacles, set dressing.
    /// Frees the previous static scene's meshes and starts recording the
    /// next one's. `set_map`, the line-ups and the podium call this.
    pub fn begin_scene(&mut self) {
        // Anything a scene left open (never ended) goes too.
        let open = self.renderer.end_scope();
        for id in std::mem::take(&mut self.scene_meshes).into_iter().chain(open) {
            self.renderer.free_mesh(id);
        }
        self.lib.clear_scene();
        self.renderer.begin_scope();
        self.statics_dirty = true;
    }

    fn end_scene(&mut self) {
        self.scene_meshes = self.renderer.end_scope();
    }

    pub fn set_map(&mut self, map: &Map) {
        self.begin_scene();
        self.renderer.recorded = Some(HashMap::new());
        self.map = Some(map.clone());
        self.statics.clear();
        self.blobs.clear();
        self.crane_anchors = map.cranes.iter().map(|c| c.anchor).collect();
        self.pumpjacks.clear();
        self.fx.flares.clear();
        let r = &mut self.renderer;
        let lib = &mut self.lib;
        // Built now, not on the first kill: building them mid-fight cost a
        // ~2 ms hitch on top of the kill's other work.
        lib.get(r, "wreck:hull", || tank::hull_mesh(WRECK_CHARRED));
        lib.get(r, "wreck:turret", || tank::turret_mesh(WRECK_CHARRED));
        for (mesh, transform) in crate::terrain::build(map) {
            let id = r.add_mesh(&mesh);
            self.statics.push(StaticPiece { instance: Instance::new(id, transform), owner: None });
        }
        for (oi, o) in map.obstacles.iter().enumerate() {
            let seed = oi as u32 * 7919 + 3;
            let frame = Mat4::from_rotation_translation(o.rot, o.pos);
            let boxes: Vec<(Vec3, Vec3)> = o.boxes.iter().map(|b| (b.center, b.half)).collect();
            match o.kind {
                ObstacleKind::ShipHull => {
                    let id = r.add_mesh(&props::ship_hull(&boxes, seed));
                    self.statics.push(StaticPiece { instance: Instance::new(id, frame), owner: Some((oi as u16, 255)) });
                }
                ObstacleKind::Tower if !boxes.is_empty() => {
                    let (c, h) = boxes[0];
                    let id = r.add_mesh(&lift(props::watchtower(h), h.y));
                    self.statics.push(StaticPiece { instance: Instance::new(id, frame * Mat4::from_translation(c - Vec3::Y * h.y)), owner: Some((oi as u16, 255)) });
                }
                ObstacleKind::PowerPole => {
                    let id = lib.scene(r, "pole", props::power_pole);
                    self.statics.push(StaticPiece { instance: Instance::new(id, frame), owner: Some((oi as u16, 255)) });
                }
                ObstacleKind::Crane if boxes.len() >= 2 => {
                    let id = r.add_mesh(&props::crane(boxes[0], boxes[1]));
                    self.statics.push(StaticPiece { instance: Instance::new(id, frame), owner: Some((oi as u16, 255)) });
                }
                ObstacleKind::Derrick if boxes.len() >= 5 => {
                    let base = boxes[0].0.x.abs().max(boxes[0].0.z.abs());
                    let top = boxes[4].0.y + boxes[4].1.y;
                    let id = lib.scene(r, &format!("derrick:{base:.2}:{top:.2}"), || refinery::derrick(base, top));
                    self.statics.push(StaticPiece { instance: Instance::new(id, frame), owner: Some((oi as u16, 255)) });
                }
                ObstacleKind::FlareStack if boxes.len() >= 2 => {
                    let id = r.add_mesh(&refinery::flare_stack(boxes[0], boxes[1]));
                    self.statics.push(StaticPiece { instance: Instance::new(id, frame), owner: Some((oi as u16, 255)) });
                    let tip = boxes[1].0.y + boxes[1].1.y + 1.4;
                    self.fx.flares.push(frame.transform_point3(Vec3::Y * tip));
                }
                ObstacleKind::WaterTower if boxes.len() >= 5 => {
                    let id = r.add_mesh(&props::water_tower(&boxes[..4], boxes[4]));
                    self.statics.push(StaticPiece { instance: Instance::new(id, frame), owner: Some((oi as u16, 255)) });
                }
                ObstacleKind::RadioMast if !boxes.is_empty() => {
                    let (c, h) = boxes[0];
                    let id = r.add_mesh(&props::radio_mast(h, c));
                    self.statics.push(StaticPiece { instance: Instance::new(id, frame), owner: Some((oi as u16, 255)) });
                }
                ObstacleKind::Windmill if !boxes.is_empty() => {
                    let (c, h) = boxes[0];
                    let id = r.add_mesh(&props::windmill(h, c));
                    self.statics.push(StaticPiece { instance: Instance::new(id, frame), owner: Some((oi as u16, 255)) });
                }
                ObstacleKind::Pumpjack => {
                    let id = lib.scene(r, "pumpjack:base", refinery::pumpjack_base);
                    self.statics.push(StaticPiece { instance: Instance::new(id, frame), owner: Some((oi as u16, 255)) });
                    self.pumpjacks.push((oi as u16, frame, hash(oi as u32, 41) * std::f32::consts::TAU));
                }
                _ => {
                    for (bi, b) in o.boxes.iter().enumerate() {
                        // Meshes are built centred and lifted to stand on y = 0, so
                        // their local y is height above the ground in the shader.
                        let xf = frame * Mat4::from_rotation_translation(b.rot, b.center) * Mat4::from_translation(Vec3::Y * -b.half.y);
                        let h = b.half;
                        let k = |name: &str| format!("{name}:{:.2}:{:.2}:{:.2}", h.x, h.y, h.z);
                        let id = match o.kind {
                            ObstacleKind::ContainerStack => {
                                let c = props::CONTAINER_COLOURS[(hash(bi as u32, seed) * 4.0) as usize % 4];
                                lib.scene(r, &format!("{}:{c:x}", k("container")), || lift(props::container(h, c), h.y))
                            }
                            ObstacleKind::Barrier => {
                                let hz = (oi + bi) % 3 == 0;
                                lib.scene(r, &format!("{}:{hz}", k("barrier")), || lift(props::barrier(h, hz), h.y))
                            }
                            ObstacleKind::Sandbags => lib.scene(r, &k("sandbags"), || lift(props::sandbags(h, 5), h.y)),
                            ObstacleKind::Silo => lib.scene(r, &k("silo"), || lift(props::silo(h.x, h.y), h.y)),
                            ObstacleKind::RockFormation => r.add_mesh(&lift(props::rock_block(h, seed + bi as u32), h.y)),
                            ObstacleKind::Mesa => r.add_mesh(&lift(props::mesa_block(h, seed + bi as u32), h.y)),
                            ObstacleKind::Highway => lib.scene(r, &k("highway"), || lift(props::highway(h), h.y)),
                            ObstacleKind::Pipe => lib.scene(r, &k("pipe"), || lift(props::pipe(h), h.y)),
                            ObstacleKind::CarWreck => {
                                let v = (hash(oi as u32, 3) * 4.0) as u32;
                                lib.scene(r, &format!("{}:{v}", k("car")), || lift(props::car_wreck(h, v), h.y))
                            }
                            ObstacleKind::PierRuin => lib.scene(r, &k("pier"), || lift(props::pier_ruin(h, 3), h.y)),
                            ObstacleKind::Wall => lib.scene(r, &k("wall"), || lift(props::wall_segment(h, 3), h.y)),
                            ObstacleKind::Tent => lib.scene(r, &k("tent"), || lift(props::quonset(h), h.y)),
                            ObstacleKind::Hedgehog => lib.scene(r, "hedgehog", props::hedgehog),
                            ObstacleKind::GasStation => match bi {
                                0 => lib.scene(r, &k("gasshop"), || lift(props::gas_shop(h), h.y)),
                                1 => lib.scene(r, &k("canopy"), || lift(props::canopy(h), h.y)),
                                _ => {
                                    let pump = bi % 2 == 0;
                                    lib.scene(r, &format!("{}:{pump}", k("pillar")), || lift(props::pillar(h, pump), h.y))
                                }
                            },
                            ObstacleKind::StorageTank => {
                                let v = oi as u32 % 3;
                                lib.scene(r, &format!("{}:{v}", k("storage")), || lift(refinery::storage_tank(h.x, h.y, v), h.y))
                            }
                            ObstacleKind::Column => lib.scene(r, &k("column"), || lift(refinery::column(h.x, h.y), h.y)),
                            ObstacleKind::PipeRack if bi == 0 => lib.scene(r, &k("rack"), || lift(refinery::rack_bundle(h), h.y)),
                            ObstacleKind::PipeRack => lib.scene(r, &k("rackleg"), || lift(refinery::rack_leg(h), h.y)),
                            ObstacleKind::TankCar => {
                                let v = (hash(oi as u32, 9) * 3.0) as u32;
                                lib.scene(r, &format!("{}:{v}", k("tankcar")), || lift(refinery::tank_car(h, v), h.y))
                            }
                            ObstacleKind::Rails => lib.scene(r, &k("rails"), || lift(refinery::rails(h), h.y)),
                            ObstacleKind::ControlHouse => lib.scene(r, &k("controlhouse"), || lift(refinery::control_house(h), h.y)),
                            ObstacleKind::LaunchPlate => lib.scene(r, &k("launch"), || lift(props::launch_plate(h), 2.0 * h.y - 0.12)),
                            ObstacleKind::Crates => lib.scene(r, &k("crates"), || {
                                let mut m = MeshBuilder::new();
                                crate::tank::crate_box(&mut m, Vec3::ZERO, h);
                                lift(m, h.y)
                            }),
                            ObstacleKind::FuelTank => lib.scene(r, &k("fueltank"), || {
                                let mut m = MeshBuilder::new();
                                let rr = h.y.min(h.z);
                                m.cylinder(rr, h.x, rr * 0.3, 2, 18, Paint::painted(0xdcaa22, 0.6), Mat4::from_rotation_z(-std::f32::consts::FRAC_PI_2));
                                m.bevel_box(Vec3::new(0.5, 0.3, h.z * 0.6), 0.04, 1, Paint::with(crate::meshkit::Material::Hazard, 0xffffff, 0.6), Mat4::from_translation(Vec3::new(0.0, 0.0, rr)));
                                lift(m, h.y)
                            }),
                            _ => lib.scene(r, &k("generic"), || lift(generic_box(h), h.y)),
                        };
                        self.statics.push(StaticPiece { instance: Instance::new(id, xf), owner: Some((oi as u16, bi as u8)) });
                    }
                }
            }
            // Rubble round tall rock; occluders for the ground AO.
            if matches!(o.kind, ObstacleKind::Mesa | ObstacleKind::RockFormation) {
                let id = r.add_mesh(&props::rubble(o.radius * 0.85, 9, 1.1, seed));
                self.statics.push(StaticPiece { instance: Instance::new(id, Mat4::from_translation(o.pos)), owner: None });
            }
            if o.cover != Cover::Flat {
                for (c, _, h) in o.world_boxes() {
                    let base = Vec3::new(c.x, map.height_at(c.x, c.z), c.z);
                    self.blobs.push(base.extend((h.x.max(h.z) * 1.25 + 0.6).min(14.0)));
                }
            }
        }
        self.dress(map);
        let before = self.statics.len();
        self.chunk_statics();
        self.renderer.recorded = None;
        self.end_scene();
        self.statics_dirty = true;
        let total = self.statics.len();
        log::info!("map: {before} static pieces, {total} after chunking ({} chunks), {} meshes", self.chunks.len(), self.renderer.mesh_count());
    }

    /// Merges one-off static pieces (meshes placed at most
    /// `CHUNK_MAX_COPIES` times) into one mesh per `CHUNK_CELL` map cell:
    /// each unique mesh would otherwise cost a draw call in every view. The
    /// merged pieces' own meshes are freed, so memory stays about level.
    /// A piece that can be shot away is blanked out of its chunk's index
    /// buffer when it goes (see `sync_chunks`); nothing is rebuilt.
    fn chunk_statics(&mut self) {
        self.chunks.clear();
        let Some(rec) = self.renderer.recorded.take() else { return };
        let mut copies: HashMap<MeshId, usize> = HashMap::new();
        for s in &self.statics {
            *copies.entry(s.instance.mesh).or_default() += 1;
        }
        let asphalt = crate::meshkit::Material::Asphalt as u32 as f32;
        let mergeable = |s: &StaticPiece| -> bool {
            let id = s.instance.mesh;
            copies.get(&id).is_some_and(|n| *n <= CHUNK_MAX_COPIES)
                && s.instance.params == Vec4::ZERO
                && self.renderer.in_open_scope(id)
                && self.renderer.mesh_radius(id) < 40.0
                // Asphalt paints its lanes in the deck's own frame.
                && rec.get(&id).is_some_and(|m| !m.indices.is_empty() && !m.vertices.iter().any(|v| v.params[1] == asphalt))
        };
        let mut cells: std::collections::BTreeMap<(i32, i32), Vec<usize>> = std::collections::BTreeMap::new();
        let mut keep: Vec<StaticPiece> = Vec::new();
        let mut merged: Vec<MeshId> = Vec::new();
        for (i, s) in self.statics.iter().enumerate() {
            if mergeable(s) {
                let at = s.instance.model.w_axis.truncate();
                cells.entry(((at.x / CHUNK_CELL).floor() as i32, (at.z / CHUNK_CELL).floor() as i32)).or_default().push(i);
                merged.push(s.instance.mesh);
            }
        }
        for ((cx, cz), pieces) in &cells {
            let origin = Vec3::new((*cx as f32 + 0.5) * CHUNK_CELL, 0.0, (*cz as f32 + 0.5) * CHUNK_CELL);
            let mut m = MeshBuilder::new();
            let mut owned = Vec::new();
            for &i in pieces {
                let piece = &self.statics[i];
                let Some(src) = rec.get(&piece.instance.mesh) else { continue };
                let model = piece.instance.model;
                let normal_m = glam::Mat3::from_mat4(model).inverse().transpose();
                let ground = model.w_axis.y;
                let base = m.vertices.len() as u32;
                for v in &src.vertices {
                    let mut v = *v;
                    v.position = (model.transform_point3(Vec3::from(v.position)) - origin).to_array();
                    v.normal = (normal_m * Vec3::from(v.normal)).normalize_or_zero().to_array();
                    // local.y + w = height above this piece's ground.
                    v.params[3] += origin.y - ground;
                    m.vertices.push(v);
                }
                let first = m.indices.len() as u32;
                m.indices.extend(src.indices.iter().map(|k| k + base));
                if let Some(owner) = piece.owner {
                    owned.push((owner, first, m.indices[first as usize..].to_vec()));
                }
            }
            let mesh = self.renderer.add_mesh(&m);
            let blanked = vec![false; owned.len()];
            self.chunks.push(Chunk { mesh, owned, blanked });
            keep.push(StaticPiece { instance: Instance::new(mesh, Mat4::from_translation(origin)), owner: None });
        }
        drop(rec);
        let statics = std::mem::take(&mut self.statics);
        let merged_set: std::collections::HashSet<usize> = cells.values().flatten().copied().collect();
        self.statics = statics.into_iter().enumerate().filter(|(i, _)| !merged_set.contains(i)).map(|(_, s)| s).collect();
        self.statics.extend(keep);
        // The merged pieces' own meshes are unused now.
        merged.sort();
        merged.dedup();
        for id in merged {
            if !self.statics.iter().any(|s| s.instance.mesh == id) {
                self.renderer.free_scoped(id);
                self.lib.forget_scene_mesh(id);
            }
        }
    }

    /// Blanks chunk pieces that were shot away (and restores any that came
    /// back) by rewriting their slice of the chunk's index buffer.
    fn sync_chunks(&mut self, removed: &[(u16, u8)]) {
        for c in &mut self.chunks {
            for (k, ((o, b), first, indices)) in c.owned.iter().enumerate() {
                let gone = removed.iter().any(|&(ro, rb)| ro == *o && (rb == 255 || rb == *b));
                if gone != c.blanked[k] {
                    let data = if gone { vec![0u32; indices.len()] } else { indices.clone() };
                    self.renderer.write_indices(c.mesh, *first, &data);
                    c.blanked[k] = gone;
                }
            }
        }
    }

    /// Replaces the static scene with hand-placed instances (lineups).
    /// Meshes uploaded since the last `begin_scene` belong to this scene.
    pub fn set_statics(&mut self, statics: Vec<Instance>) {
        self.statics = statics.into_iter().map(|instance| StaticPiece { instance, owner: None }).collect();
        self.chunks.clear();
        self.statics_dirty = true;
        self.blobs.clear();
        self.crane_anchors.clear();
        self.pumpjacks.clear();
        self.fx.flares.clear();
        if self.renderer.in_scope() {
            self.end_scene();
        }
    }

    /// Visual-only set dressing, clustered where people would have left it:
    /// clutter hugging the places (crates and drums by containers, tyres
    /// at the gas station, drifts against walls, debris along the road),
    /// rock outcrops and cactus groves in the open, and clean empty sand
    /// between them. Plus the ground marks and the power lines.
    fn dress(&mut self, map: &Map) {
        let r = &mut self.renderer;
        let lib = &mut self.lib;
        let salt = if map.id == arena_core::MapId::Refinery { 0.85 } else { 0.0 };
        let plan = dressing_plan(map);
        let mut blobs: Vec<Vec4> = Vec::new();
        for d in &plan {
            let (id, blob) = clutter_mesh(lib, r, d.kind, d.v, salt);
            let p = Vec3::new(d.pos.x, map.height_at(d.pos.x, d.pos.y), d.pos.y);
            self.statics.push(StaticPiece { instance: Instance::new(id, Mat4::from_rotation_translation(Quat::from_rotation_y(d.yaw), p)), owner: None });
            if blob > 0.0 {
                blobs.push(p.extend(blob));
            }
        }
        let placed = plan.len();
        self.blobs.extend(blobs);
        // Tracks, packed ground and oil, draped over the terrain.
        for m in crate::terrain::marks(map) {
            let id = r.add_mesh(&m);
            self.statics.push(StaticPiece { instance: Instance::new(id, Mat4::IDENTITY), owner: None });
        }
        // Power lines between the map's poles.
        let map_poles: Vec<Vec3> = map.obstacles.iter().filter(|o| o.kind == ObstacleKind::PowerPole).map(|o| o.pos).collect();
        for (i, a) in map_poles.iter().enumerate() {
            let next = map_poles.iter().enumerate().filter(|(j, b)| *j > i && a.distance(**b) < 70.0).min_by(|x, y| a.distance(*x.1).partial_cmp(&a.distance(*y.1)).unwrap_or(std::cmp::Ordering::Equal));
            if let Some((_, b)) = next {
                let top = |p: Vec3| p + Vec3::Y * props::POLE_WIRE_HEIGHT;
                let id = r.add_mesh(&props::wires(top(*a), top(*b), 1.6));
                self.statics.push(StaticPiece { instance: Instance::new(id, Mat4::IDENTITY), owner: None });
            }
        }
        self.dressed = placed;
        log::info!("map dressing: {placed} props, {} map poles", map_poles.len());
    }

    /// Feeds one simulation event to the effects (and the camera shake the
    /// caller may apply: returns trauma to add for an event near `listener`).
    pub fn event(&mut self, e: &Event, listener: Vec3) -> f32 {
        if let Event::ShipLaunchedBuggy { ship, .. } = e {
            self.ramps.insert(*ship, self.clock);
        }
        self.fx.event(e, listener)
    }

    /// Draws a frame of `world` (plus the static map) into `output`.
    pub fn draw(&mut self, output: &wgpu::TextureView, world: &RenderWorld) {
        let camera = world.camera.unwrap_or_else(|| Camera::look(Vec3::new(30.0, 20.0, 30.0), Vec3::ZERO, 45.0));
        self.clock = world.time;
        self.ramps.retain(|_, s| world.time - *s < 6.0 && world.time >= *s);
        let dt = world.dt.clamp(0.0, 0.1);
        let t0 = clock_ms();
        self.fx.update(dt, world);
        let t1 = clock_ms();

        // The static scene goes to the renderer once, and again only when
        // it changes (a new scene, or map pieces shot away).
        if self.statics_dirty || self.statics_removed != world.removed {
            let removed = &world.removed;
            let kept: Vec<Instance> = self
                .statics
                .iter()
                .filter(|s| !s.owner.is_some_and(|(o, b)| removed.iter().any(|&(ro, rb)| ro == o && (rb == 255 || rb == b))))
                .map(|s| s.instance)
                .collect();
            self.renderer.set_statics(&kept);
            self.sync_chunks(&world.removed);
            self.statics_removed = world.removed.clone();
            self.statics_dirty = false;
        }
        let mut instances: Vec<Instance> = Vec::with_capacity(400);
        let mut occluders = self.map_occluders.clone();
        let mut blobs = self.blobs.clone();
        for t in world.tanks.iter() {
            let g = t.hull_transform().transform_point3(Vec3::new(0.0, -tank::HULL_Y, 0.0));
            blobs.push(g.extend(3.6));
        }
        for p in &world.props {
            blobs.push((p.pos - Vec3::Y * 0.4).extend(0.7));
        }
        let r = &mut self.renderer;
        let lib = &mut self.lib;
        // Wrecks: track each one, hold it a while after the sim drops it,
        // then sink it into the sand.
        for w in self.wrecks.values_mut() {
            w.1 += dt;
            if let Some(g) = &mut w.2 {
                *g += dt;
            }
        }
        for p in world.props.iter().filter(|p| p.kind == PropKind::Wreck) {
            let e = self.wrecks.entry(p.id).or_insert((Mat4::IDENTITY, 0.0, None));
            e.0 = Mat4::from_rotation_translation(p.rot, p.pos);
            e.2 = None;
        }
        for (id, w) in self.wrecks.iter_mut() {
            if w.2.is_none() && !world.props.iter().any(|p| p.id == *id) {
                w.2 = Some(0.0);
            }
        }
        self.wrecks.retain(|_, w| w.2.is_none_or(|g| g < 12.0));
        let charred = WRECK_CHARRED;
        for (id, (pose, _age, gone)) in &self.wrecks {
            let sink = gone.map(|g| ((g - 8.0) / 4.0).clamp(0.0, 1.0)).unwrap_or(0.0);
            let pose = Mat4::from_translation(Vec3::Y * -2.6 * sink) * *pose;
            let hull = lib.get(r, "wreck:hull", || tank::hull_mesh(charred));
            instances.push(Instance { params: Vec4::new(0.0, 0.0, 0.0, 1.0), ..Instance::new(hull, pose) });
            // The turret, blown off and lying beside the hull.
            let side = if id % 2 == 0 { 1.0 } else { -1.0 };
            let turret_pose = pose
                * Mat4::from_rotation_translation(
                    Quat::from_rotation_y(0.9 * side) * Quat::from_rotation_x(0.5 * side) * Quat::from_rotation_z(0.25),
                    Vec3::new(-1.2, -0.35, side * 3.4),
                );
            let turret = lib.get(r, "wreck:turret", || tank::turret_mesh(charred));
            instances.push(Instance { params: Vec4::new(0.0, 0.0, 0.0, 1.0), ..Instance::new(turret, turret_pose) });
        }

        for t in &world.tanks {
            if !t.alive {
                continue;
            }
            let health = world.health.iter().find(|h| h.0 == t.player).map(|h| h.1).unwrap_or(1.0);
            let damage = (1.0 - health).clamp(0.0, 1.0);
            let anim = self.anims.entry(t.player).or_default();
            for s in 0..2 {
                anim.tread[s] = (anim.tread[s] + t.track_speed[s] * dt).rem_euclid(1000.0);
            }
            anim.seen = true;
            let col = if t.alive { t.colour } else { 0x2c2622 };
            let hull = t.hull_transform();
            let params = Vec4::new(anim.tread[0], 0.0, anim.tread[1], damage);
            let vehicle = world.vehicles.iter().find(|v| v.0 == t.player);
            let kind = vehicle.map(|v| v.1).unwrap_or(VehicleKind::Tank);
            if kind != VehicleKind::Tank {
                let chassis = arena_core::tank::chassis_of(&self.tuning, kind);
                let scale = Vec3::from(chassis.hull_half) / Vec3::from(self.tuning.tank.hull_half);
                let pivot = Vec3::from(self.tuning.turret.pivot) * scale;
                let (hull_mesh, key): (fn(u32) -> MeshBuilder, &str) = match kind {
                    VehicleKind::Buggy => (tank::buggy_mesh, "buggy"),
                    VehicleKind::Technical => (tank::technical_mesh, "technical"),
                    VehicleKind::LandShip => (crate::ships::land_ship, "landship"),
                    VehicleKind::FuelTanker => (crate::ships::fuel_tanker, "tanker"),
                    _ => (tank::war_rig_mesh, "warrig"),
                };
                let id = lib.get(r, &format!("{key}:{col:x}"), || hull_mesh(col));
                instances.push(Instance::new(id, hull));
                let turret = hull * Mat4::from_rotation_translation(Quat::from_rotation_y(t.turret_yaw), pivot);
                match kind {
                    VehicleKind::Buggy => {
                        let id = lib.get(r, "buggy:mg", tank::buggy_turret);
                        instances.push(Instance::new(id, turret));
                    }
                    VehicleKind::Technical => {
                        let id = lib.get(r, &format!("technical:rack:{col:x}"), || tank::technical_turret(col));
                        instances.push(Instance::new(id, turret));
                    }
                    _ => {}
                }
                let radius = chassis.wheel_radius;
                let wheel = lib.get(r, &format!("truckwheel:{radius:.2}"), || tank::truck_wheel(radius));
                let n = chassis.wheels_per_side.max(1);
                let ray_len = chassis.suspension_travel + chassis.wheel_radius;
                let comps = vehicle.map(|v| v.2.clone()).unwrap_or_default();
                // The Land-Ship rolls on its track modules, not wheels.
                let wheels = if kind == VehicleKind::LandShip { 0 } else { 2 * n };
                for i in 0..wheels {
                    let side = if i < n { -1.0 } else { 1.0 };
                    let k = i % n;
                    let x = (n as f32 - 1.0) * 0.5 * chassis.wheel_spacing - k as f32 * chassis.wheel_spacing;
                    let comp = comps.get(i).copied().unwrap_or(ray_len * 0.3);
                    let y = chassis.ray_origin_y - (ray_len - comp) + radius;
                    let spin = -anim.tread[(side > 0.0) as usize] / radius;
                    let local = Mat4::from_rotation_translation(
                        Quat::from_rotation_y(if side < 0.0 { std::f32::consts::PI } else { 0.0 }) * Quat::from_rotation_z(spin),
                        Vec3::new(x, y, side * chassis.track_half_width),
                    );
                    instances.push(Instance::new(wheel, hull * local));
                }
                for &p in &t.parts {
                    // The Land-Ship's deck guns turn and lift on their own; its
                    // stern ramp drops when a buggy rolls out.
                    if let Some(ring) = arena_core::parts::SHIP_GUNS.iter().find(|g| g.0 == p).map(|g| g.1) {
                        let rest = if p == PartKind::ShipGunAft { std::f32::consts::PI } else { 0.0 };
                        let (yaw, pitch) = world.turrets.iter().find(|x| x.0 == t.player && x.1 == p).map_or((rest, 0.0), |x| (x.2, x.3));
                        let house = lib.get(r, &format!("shipgun:{col:x}"), || crate::ships::ship_gun_house(col));
                        let tf = hull * Mat4::from_rotation_translation(Quat::from_rotation_y(yaw), ring);
                        instances.push(Instance::new(house, tf));
                        let barrel = lib.get(r, "shipgun:barrel", crate::ships::ship_gun_barrel);
                        instances.push(Instance::new(barrel, tf * Mat4::from_rotation_translation(Quat::from_rotation_z(pitch), crate::ships::SHIP_TRUNNION)));
                        continue;
                    }
                    if p == PartKind::ShipHangar {
                        let frame_id = lib.get(r, "shiphangar", crate::ships::ship_hangar_frame);
                        instances.push(Instance::new(frame_id, hull));
                        let open = self.ramps.get(&t.player).map_or(0.0, |s| ramp_open(world.time - s));
                        let ramp = lib.get(r, "shipramp", crate::ships::ship_ramp);
                        instances.push(Instance::new(ramp, hull * Mat4::from_rotation_translation(Quat::from_rotation_z(open * 1.25), crate::ships::SHIP_RAMP_HINGE)));
                        continue;
                    }
                    let id = lib.get(r, &format!("part:{p:?}:{col:x}"), || parts::part_mesh(p, col));
                    let frame = match arena_core::part_def(p).frame {
                        arena_core::Frame::Hull => hull,
                        _ => turret,
                    };
                    instances.push(Instance::new(id, frame));
                }
                let size = Vec3::from(chassis.hull_half);
                occluders.push(hull.transform_point3(Vec3::new(0.0, -size.y, 0.0)).extend(size.x.max(size.z)));
                continue;
            }
            // One mesh per part for every crew: the team paint comes from
            // the instance tint.
            let tint = crate::meshkit::srgb_hex(col).extend(1.0);
            let class = t.class;
            let hull_id = lib.get(r, &format!("hull:{class:?}"), || tank::hull_mesh_class(class, TINTED));
            instances.push(Instance::tinted(hull_id, hull, params, tint));
            let dmg = Vec4::new(0.0, 0.0, 0.0, damage);
            let turret_id = lib.get(r, &format!("turret:{class:?}"), || tank::turret_mesh_class(class, TINTED));
            instances.push(Instance::tinted(turret_id, t.turret_transform(), dmg, tint));
            let gun = t.gun;
            let gun_id = lib.get(r, &format!("gun:{gun:?}"), || parts::gun_mesh(gun, TINTED));
            instances.push(Instance::tinted(gun_id, t.gun_transform(), dmg, tint));
            let wheel = lib.get(r, "wheel", tank::road_wheel_mesh);
            for (i, c) in t.wheels.iter().enumerate() {
                let side = if i < 5 { -1.0 } else { 1.0 };
                let x = tank::WHEEL_X[i % 5];
                let local = Mat4::from_rotation_translation(
                    Quat::from_rotation_y(if side < 0.0 { std::f32::consts::PI } else { 0.0 }) * Quat::from_rotation_z(-anim.tread[(side > 0.0) as usize] / tank::WHEEL_R),
                    Vec3::new(x, tank::wheel_y(*c), side * tank::WHEEL_Z),
                );
                instances.push(Instance::new(wheel, hull * local));
            }
            for &p in &t.parts {
                let id = lib.get(r, &format!("part:{p:?}"), || parts::part_mesh(p, TINTED));
                let frame = match arena_core::part_def(p).frame {
                    arena_core::Frame::Hull => hull,
                    arena_core::Frame::Turret => t.turret_transform(),
                    arena_core::Frame::Gun => t.gun_transform(),
                };
                let part_health = world.part_health.iter().find(|h| h.0 == t.player && h.1 == p).map(|h| h.2).unwrap_or(1.0);
                let part_damage = (1.0 - part_health).max(damage * 0.6).clamp(0.0, 1.0);
                instances.push(Instance::tinted(id, frame, Vec4::new(0.0, 0.0, 0.0, part_damage), tint));
            }
            occluders.push(hull.transform_point3(Vec3::new(1.4, -0.1, 0.0)).extend(1.7));
            occluders.push(hull.transform_point3(Vec3::new(-1.4, -0.1, 0.0)).extend(1.7));
        }
        for p in &world.props {
            let xf = Mat4::from_rotation_translation(p.rot, p.pos);
            let id = match p.kind {
                PropKind::OilDrum => {
                    let c = [0x34587e, 0x8c2a1e, 0x2f6d5e, 0xc28a22][p.id as usize % 4];
                    lib.get(r, &format!("drum:{c:x}"), || props::oil_drum(c, 0.6))
                }
                PropKind::Debris(kind) => lib.get(r, &format!("debris:{kind:?}"), || centred(if parts::is_gun(kind) { parts::gun_mesh(kind, 0x3a3430) } else { parts::part_mesh(kind, 0x3a3430) })),
                // Drawn above from the wreck tracker (held, then sunk).
                PropKind::Wreck => continue,
                PropKind::Container => lib.get(r, "container:loose", || props::container(Vec3::new(6.1, 1.3, 1.22), 0xb3302a)),
                PropKind::Rubble(_) => lib.get(r, "rubble:chunk", || props::boulder(5, 0.8)),
                _ => lib.get(r, "ball", || {
                    let mut m = MeshBuilder::new();
                    m.rock_blob(3, Vec3::splat(1.4), Paint::steel(0x4a4440, 0.5), Mat4::IDENTITY);
                    m
                }),
            };
            instances.push(Instance::new(id, xf));
        }
        for p in &world.pickups {
            let k = match p.kind {
                PickupKind::Scrap(ScrapKind::Steel) => 0,
                PickupKind::Scrap(ScrapKind::Powder) => 1,
                PickupKind::Scrap(ScrapKind::Volts) => 2,
                PickupKind::Scrap(ScrapKind::Fuel) => 3,
                _ => 4,
            };
            let id = lib.get(r, &format!("pickup:{k}"), || props::pickup(k));
            let bob = (world.time * 2.0 + p.id as f32).sin() * 0.12 + 0.3;
            let xf = Mat4::from_rotation_translation(Quat::from_rotation_y(world.time * 1.2 + p.id as f32), p.pos + Vec3::Y * bob);
            instances.push(Instance::new(id, xf));
        }
        // Special projectiles as meshes along their flight.
        for s in &world.shells {
            let key = match s.kind {
                ShotKind::Rocket => "shot:rocket",
                ShotKind::MortarShell => "shot:mortar",
                ShotKind::Harpoon => "shot:harpoon",
                _ => continue,
            };
            let id = lib.get(r, key, || match s.kind {
                ShotKind::Rocket => props::rocket(),
                ShotKind::MortarShell => props::mortar_shell(),
                _ => props::harpoon(),
            });
            let rot = Quat::from_rotation_arc(Vec3::X, s.vel.normalize_or(Vec3::X));
            instances.push(Instance::new(id, Mat4::from_rotation_translation(rot, s.pos)));
        }
        for (oi, frame, phase) in &self.pumpjacks {
            if world.removed.iter().any(|&(ro, _)| ro == *oi) {
                continue;
            }
            // About seven strokes a minute; the head bobs, the crank turns.
            let a = world.time * 0.75 + phase;
            let beam = lib.get(r, "pumpjack:beam", refinery::pumpjack_beam);
            let xf = *frame * Mat4::from_rotation_translation(Quat::from_rotation_z(a.sin() * 0.26), refinery::PUMPJACK_PIVOT);
            instances.push(Instance::new(beam, xf));
            let crank = lib.get(r, "pumpjack:crank", refinery::pumpjack_crank);
            let xf = *frame * Mat4::from_rotation_translation(Quat::from_rotation_z(-a), refinery::PUMPJACK_CRANK);
            instances.push(Instance::new(crank, xf));
        }
        for (p, yaw, fade) in &world.route {
            let id = lib.get(r, "chevron", crate::ships::chevron);
            let s = 0.7 + 0.3 * fade;
            instances.push(Instance::new(id, Mat4::from_scale_rotation_translation(Vec3::splat(s), Quat::from_rotation_y(*yaw), *p)));
        }
        for (_, p, _) in &world.mines {
            let id = lib.get(r, "mine", props::mine);
            instances.push(Instance::new(id, Mat4::from_translation(*p)));
        }
        if let Some((centre, radius, _)) = world.raid {
            let map = self.map.as_ref();
            let ground = |x: f32, z: f32| map.map(|m| m.height_at(centre.x + x, centre.z + z)).unwrap_or(centre.y);
            let key = format!("raidzone:{radius:.1}:{:.0}:{:.0}", centre.x, centre.z);
            let id = lib.get(r, &key, || props::raid_zone(radius, tank::RUSTBACK, &ground));
            instances.push(Instance::new(id, Mat4::from_translation(Vec3::new(centre.x, 0.0, centre.z))));
        }
        // Wellhead: each derrick's pad, pumpjack (it pumps while held), the
        // holder's flag on the mast and the capture ring of painted plates.
        for (k, &(pos, owner, fill, contested)) in world.wells.iter().enumerate() {
            let map = self.map.as_ref();
            let frame = Mat4::from_rotation_translation(Quat::from_rotation_y(0.6 + k as f32 * 2.1), pos);
            let pad = lib.get(r, "well:pad", crate::objectives::well_pad);
            instances.push(Instance::new(pad, frame));
            let jack = frame * Mat4::from_translation(Vec3::new(-0.8, 0.12, 0.0));
            let base = lib.get(r, "well:pumpjack", refinery::pumpjack_base);
            instances.push(Instance::new(base, jack));
            let a = if owner < 2 { world.time * 1.1 + k as f32 } else { 0.4 };
            let beam = lib.get(r, "pumpjack:beam", refinery::pumpjack_beam);
            instances.push(Instance::new(beam, jack * Mat4::from_rotation_translation(Quat::from_rotation_z(a.sin() * 0.26), refinery::PUMPJACK_PIVOT)));
            let crank = lib.get(r, "pumpjack:crank", refinery::pumpjack_crank);
            instances.push(Instance::new(crank, jack * Mat4::from_rotation_translation(Quat::from_rotation_z(-a), refinery::PUMPJACK_CRANK)));
            let team = |t: u8| match t {
                0 => tank::RUSTBACK,
                1 => tank::DUST_SAINT,
                _ => 0xd8ccb0,
            };
            let tint = |c: u32| crate::meshkit::srgb_hex(c).extend(1.0);
            // The flag: the holder's colours (a pale rag when nobody holds it), stirring.
            let flag = lib.get(r, "well:flag", crate::objectives::well_flag);
            let stir = Quat::from_rotation_y((world.time * 1.7 + k as f32).sin() * 0.18 - 0.3);
            let lift = if owner < 2 { 0.0 } else { -1.8 };
            let fx = frame * Mat4::from_rotation_translation(stir, crate::objectives::FLAG_AT + Vec3::Y * lift);
            instances.push(Instance::tinted(flag, fx, Vec4::ZERO, tint(team(owner))));
            // The ring: plates fill round from north in the capturing crew's paint.
            let radius = world.well_radius.max(4.0);
            let n = crate::objectives::RING_PLATES;
            let len = radius * std::f32::consts::TAU / n as f32 * 0.7;
            let plate = lib.get(r, &format!("well:plate:{len:.2}"), || crate::objectives::ring_plate(len));
            let stake = lib.get(r, "well:stake", crate::objectives::ring_stake);
            let side = if fill < 0.0 { 0 } else { 1 };
            let lit = (fill.abs() * n as f32).round() as usize;
            let blink = contested && (world.time * 4.0).fract() < 0.5;
            for i in 0..n {
                let ang = (i as f32 + 0.5) / n as f32 * std::f32::consts::TAU;
                let (x, z) = (pos.x + ang.sin() * radius, pos.z - ang.cos() * radius);
                let y = map.map_or(pos.y, |m| m.height_at(x, z));
                let rot = Quat::from_rotation_y(-ang);
                let colour = if i < lit {
                    team(side)
                } else if blink {
                    0xe8b419
                } else {
                    0x4a423a
                };
                instances.push(Instance::tinted(plate, Mat4::from_rotation_translation(rot, Vec3::new(x, y, z)), Vec4::ZERO, tint(colour)));
                if i % 5 == 0 {
                    let (sx, sz) = (pos.x + ang.sin() * (radius + 0.6), pos.z - ang.cos() * (radius + 0.6));
                    let sy = map.map_or(pos.y, |m| m.height_at(sx, sz));
                    instances.push(Instance::new(stake, Mat4::from_translation(Vec3::new(sx, sy, sz))));
                }
            }
        }
        // Last Crew: the dust curtain along the wall (drawn by the effects pass).
        self.fx.curtain = match (world.storm_wall, self.map.as_ref()) {
            (Some((c, rad)), Some(m)) => crate::objectives::curtain_points(c, rad, camera.eye, world.time, &|x, z| m.height_at(x, z)),
            (Some((c, rad)), None) => crate::objectives::curtain_points(c, rad, camera.eye, world.time, &|_, _| 0.0),
            _ => Vec::new(),
        };
        // Wrecking balls hang from the nearest crane jib tip.
        let mut ropes: Vec<(Vec3, Vec3)> = world.tethers.clone();
        for p in world.props.iter().filter(|p| matches!(p.kind, PropKind::WreckingBall)) {
            if let Some(a) = self.crane_anchors.iter().min_by(|a, b| a.distance_squared(p.pos).partial_cmp(&b.distance_squared(p.pos)).unwrap_or(std::cmp::Ordering::Equal)) {
                ropes.push((*a, p.pos + Vec3::Y * 1.6));
            }
        }
        // Air strikes: the bomber crosses the run line once the delay is up.
        self.strike_runs.retain(|id, _| world.strikes.iter().any(|s| s.id == *id));
        for s in &world.strikes {
            if s.delay > 0.0 {
                continue;
            }
            let start = *self.strike_runs.entry(s.id).or_insert(world.time);
            let dir = (s.to - s.from).normalize_or(Vec3::X);
            let speed = 70.0;
            let pos = s.from - dir * 140.0 + dir * speed * (world.time - start) + Vec3::Y * 75.0;
            let id = lib.get(r, "plane", props::plane);
            let rot = Quat::from_rotation_arc(Vec3::X, dir);
            instances.push(Instance::new(id, Mat4::from_rotation_translation(rot, pos)));
        }
        let mut lights: Vec<Light> = self.fx.lights_with(
            &self.fx.flares.iter().map(|f| Light { pos: *f + Vec3::Y * 2.0, radius: 30.0, color: Vec3::new(1.0, 0.55, 0.18) * (5.0 + (world.time * 9.0).sin()) }).collect::<Vec<_>>(),
        );
        for p in &world.pickups {
            if lights.len() < 6 {
                lights.push(Light { pos: p.pos + Vec3::Y, radius: 4.0, color: Vec3::new(1.0, 0.8, 0.4) * 0.6 });
            }
        }
        let t2 = clock_ms();
        let sprites = self.fx.sprites(world, camera.eye, &ropes);
        let ruts = self.fx.ruts();
        self.fx.set_max_particles(self.renderer.quality_settings().max_particles);
        self.fx.storm = self.renderer.look.storm;
        let t3 = clock_ms();
        self.renderer.render(output, &camera, world.time, &instances, &sprites, &ruts, &blobs, &lights, &occluders);
        let t4 = clock_ms();
        self.prof = [t1 - t0, t2 - t1, t3 - t2, t4 - t3];
    }
}

/// How far a Land-Ship's stern ramp is open (0 shut .. 1 down), `t` seconds
/// after a buggy launch: it drops fast, holds, then winches back up.
fn ramp_open(t: f32) -> f32 {
    if t < 0.0 {
        0.0
    } else if t < 0.5 {
        (t / 0.5).powf(0.7)
    } else if t < 3.0 {
        1.0
    } else {
        (1.0 - (t - 3.0) / 1.5).max(0.0)
    }
}

/// A plain plated box, for obstacle kinds without a dedicated model yet.
fn generic_box(h: Vec3) -> MeshBuilder {
    let mut m = MeshBuilder::new();
    m.bevel_box(h, 0.08, 2, Paint::painted(0x7a6a52, 0.8), Mat4::IDENTITY);
    m
}

/// Re-centres a part mesh on its bounding box (for loose debris).
fn centred(mut m: MeshBuilder) -> MeshBuilder {
    if m.vertices.is_empty() {
        m.bevel_box(Vec3::splat(0.3), 0.05, 1, Paint::painted(0x3a3430, 0.8), Mat4::IDENTITY);
        return m;
    }
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for v in &m.vertices {
        lo = lo.min(Vec3::from(v.position));
        hi = hi.max(Vec3::from(v.position));
    }
    m.transform_from(0, Mat4::from_translation(-(lo + hi) * 0.5));
    m
}

/// Shifts a centred mesh up so it stands on y = 0.
fn lift(mut m: MeshBuilder, dy: f32) -> MeshBuilder {
    m.transform_from(0, Mat4::from_translation(Vec3::Y * dy));
    m
}

// ── Set dressing: where the clutter goes ─────────────────────────────────

/// A visual-only dressing prop (tanks drive through these).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Clutter {
    Crates,
    Jerry,
    Tarp,
    Sandbags,
    TyreStack,
    Tyre,
    Pallet,
    Cinder,
    Sheet,
    Pebbles,
    Tuft,
    Shrub,
    /// A boulder; `v` is its size class 0..6.
    Boulder,
    Cactus,
    Razor,
    GroundPipe,
    Drum,
    /// A sand drift heaped against a wall; +Z points away from the wall.
    Drift,
}

/// One placed dressing prop.
#[derive(Clone, Copy, Debug)]
struct Dressing {
    kind: Clutter,
    v: u32,
    pos: Vec2,
    yaw: f32,
}

/// The mesh for a dressing prop (built once per variant) and its contact
/// shadow radius (0: none).
fn clutter_mesh(lib: &mut Library, r: &mut Renderer, k: Clutter, v: u32, salt: f32) -> (MeshId, f32) {
    use Clutter::*;
    match k {
        Crates => (lib.scene(r, &format!("crates:{}", v % 2), || props::crates(v % 2)), 1.4),
        Jerry => (lib.scene(r, &format!("jerry:{}", v % 3), || props::jerrycans(v % 3)), 0.5),
        Tarp => (lib.scene(r, &format!("tarp:{}", v % 3), || props::tarp(v % 3)), 1.4),
        Sandbags => (
            lib.scene(r, "sandbags:short", || {
                let mut m = props::sandbags(Vec3::new(1.4, 0.4, 0.3), 9);
                m.transform_from(0, Mat4::from_translation(Vec3::Y * 0.4));
                m
            }),
            1.4,
        ),
        TyreStack => (lib.scene(r, &format!("tyres:{}", 2 + v % 3), || props::tyres(2 + v % 3, 2 + v)), 0.6),
        Tyre => (lib.scene(r, "tyres:1", || props::tyres(1, 1)), 0.5),
        Pallet => (lib.scene(r, &format!("pallet:{}", v % 3), || props::pallet(v % 3)), 0.7),
        Cinder => (lib.scene(r, &format!("cinder:{}", v % 3), || props::cinder_blocks(v % 3)), 0.4),
        Sheet => (lib.scene(r, &format!("sheet:{}", v % 3), || props::sheet_on_ground(v % 3)), 0.0),
        Pebbles => (lib.scene(r, &format!("pebbles:{}", v % 3), || props::pebbles(v % 3)), 0.0),
        Tuft => (lib.scene(r, &format!("tuft:{}", v % 4), || props::grass_tuft(v % 4)), 0.0),
        Shrub => (lib.scene(r, &format!("shrub:{}", v % 3), || props::shrub(v % 3)), 0.6),
        Boulder => {
            let c = v % 6;
            (lib.scene(r, &format!("boulder:{c}"), || props::boulder(c + 17, 0.4 + 0.25 * c as f32)), 0.6 + 0.25 * c as f32)
        }
        Cactus => (lib.scene(r, &format!("cactus:{}", v % 4), || props::cactus(v % 4)), 0.0),
        Razor => (lib.scene(r, "razor", || props::razor_wire(4.0)), 0.0),
        GroundPipe => {
            let c = v % 3;
            (lib.scene(r, &format!("gpipe:{c}"), || refinery::ground_pipe(7.0, [0x8a8a82, 0x7a3a26, 0xd6a21e][c as usize])), 0.0)
        }
        Drum => {
            let c = v % 3;
            (lib.scene(r, &format!("drum:{c}"), || lift(props::oil_drum([0x2f5f9a, 0xb3302a, 0x5e6a3a][c as usize], 0.75), props::DRUM_HALF)), 0.5)
        }
        Drift => {
            let c = v % 3;
            let (len, depth, h) = (4.5 + 1.5 * c as f32, 2.2 + 0.5 * c as f32, 0.6 + 0.15 * c as f32);
            // Blown sand, even over the salt flat (lower there: less of it).
            let h = if salt > 0.0 { h * 0.6 } else { h };
            (lib.scene(r, &format!("drift:{c}:{}", salt > 0.0), || props::sand_drift(len, depth, h, 0.0, c)), 0.0)
        }
    }
}

/// What clusters round an obstacle: how many props, how far out from its
/// boxes (min, max m), and the mix (prop, weight).
fn recipe(kind: ObstacleKind, ground_boxes: usize, desert: bool) -> (usize, (f32, f32), &'static [(Clutter, u32)]) {
    use Clutter::*;
    match kind {
        ObstacleKind::ContainerStack => (3 + 4 * ground_boxes.min(4), (0.2, 1.8), &[(Crates, 4), (Pallet, 3), (Drum, 2), (Jerry, 2), (Cinder, 2), (Tarp, 1), (TyreStack, 1), (Sheet, 2)]),
        ObstacleKind::Tent => (10, (0.3, 2.2), &[(Crates, 3), (Jerry, 2), (Pallet, 2), (Tarp, 2), (Cinder, 1), (Drum, 1)]),
        ObstacleKind::Tower => (5, (0.3, 1.5), &[(Sandbags, 2), (Crates, 2), (Jerry, 1)]),
        ObstacleKind::Silo | ObstacleKind::FuelTank => (5, (0.2, 1.4), &[(Drum, 3), (Jerry, 2), (Pallet, 1)]),
        ObstacleKind::GasStation => (16, (0.3, 3.0), &[(TyreStack, 4), (Tyre, 3), (Jerry, 2), (Drum, 2), (Sheet, 2), (Cinder, 2), (Crates, 1)]),
        ObstacleKind::CarWreck => (4, (0.0, 3.0), &[(Tyre, 3), (Sheet, 2), (Pebbles, 2), (Cinder, 1)]),
        ObstacleKind::ShipHull => (12, (0.5, 6.0), &[(Sheet, 4), (Pebbles, 3), (GroundPipe, 1), (Tyre, 1), (Boulder, 1)]),
        ObstacleKind::Wall => (3, (0.2, 1.4), &[(Crates, 3), (Pallet, 2), (Drum, 2), (Cinder, 2), (Sandbags, 2), (Jerry, 1), (Tarp, 1), (Sheet, 1)]),
        ObstacleKind::Sandbags => (2, (0.3, 1.2), &[(Crates, 2), (Jerry, 2), (Tyre, 1)]),
        ObstacleKind::Barrier => (1, (0.3, 2.0), &[(Razor, 2), (Pebbles, 1), (Tuft, 1)]),
        ObstacleKind::Hedgehog => (1, (0.3, 1.5), &[(Razor, 1), (Pebbles, 1)]),
        ObstacleKind::Mesa if desert => (12, (0.0, 8.0), &[(Boulder, 6), (Shrub, 2), (Pebbles, 2), (Tuft, 2), (Cactus, 1)]),
        ObstacleKind::RockFormation if desert => (9, (0.0, 6.0), &[(Boulder, 6), (Shrub, 2), (Pebbles, 2), (Tuft, 2), (Cactus, 1)]),
        ObstacleKind::Mesa => (10, (0.0, 8.0), &[(Boulder, 6), (Shrub, 1), (Pebbles, 2), (Tuft, 1)]),
        ObstacleKind::RockFormation => (8, (0.0, 6.0), &[(Boulder, 6), (Shrub, 1), (Pebbles, 2), (Tuft, 1)]),
        ObstacleKind::Crane => (9, (0.5, 4.0), &[(Pallet, 3), (Crates, 2), (Cinder, 2), (Drum, 2), (Sheet, 1)]),
        ObstacleKind::PierRuin => (4, (0.0, 3.0), &[(Sheet, 2), (Pebbles, 2), (Tyre, 1)]),
        ObstacleKind::Pipe => (1, (0.2, 1.0), &[(Pebbles, 1), (Tuft, 1)]),
        ObstacleKind::PowerPole => (1, (0.3, 1.0), &[(Tuft, 2), (Shrub, 1), (Pebbles, 1)]),
        ObstacleKind::StorageTank => (2, (0.3, 1.6), &[(Drum, 2), (GroundPipe, 1), (Jerry, 1)]),
        ObstacleKind::Column => (2, (0.3, 1.5), &[(Crates, 2), (Pallet, 1), (Drum, 1)]),
        ObstacleKind::PipeRack => (3, (0.3, 1.5), &[(Drum, 2), (Pallet, 1), (Cinder, 1)]),
        ObstacleKind::Pumpjack => (3, (0.3, 2.0), &[(Drum, 2), (GroundPipe, 1), (Jerry, 1)]),
        ObstacleKind::Derrick => (7, (0.5, 4.0), &[(GroundPipe, 3), (Pallet, 2), (Drum, 2), (Crates, 1)]),
        ObstacleKind::TankCar => (2, (0.3, 1.5), &[(Pallet, 1), (Cinder, 1), (Pebbles, 1)]),
        ObstacleKind::FlareStack => (4, (0.5, 2.5), &[(Drum, 2), (Crates, 1), (Sandbags, 1)]),
        ObstacleKind::ControlHouse => (12, (0.3, 2.0), &[(Crates, 3), (Jerry, 2), (Tarp, 1), (Pallet, 2), (Drum, 1)]),
        ObstacleKind::WaterTower => (7, (0.3, 3.0), &[(Drum, 2), (Tuft, 3), (Shrub, 2), (Jerry, 1), (Pallet, 1)]),
        ObstacleKind::Windmill => (5, (0.5, 3.0), &[(Tuft, 3), (Shrub, 2), (Tyre, 1)]),
        ObstacleKind::RadioMast => (2, (0.3, 1.5), &[(Crates, 2), (Cinder, 1)]),
        _ => (0, (0.0, 0.0), &[]),
    }
}

fn pick(table: &[(Clutter, u32)], roll: f32) -> Clutter {
    let total: u32 = table.iter().map(|t| t.1).sum();
    let mut x = roll * total as f32;
    for (k, w) in table {
        if x < *w as f32 {
            return *k;
        }
        x -= *w as f32;
    }
    table[table.len() - 1].0
}

/// A point `d` m out from one of the obstacle's ground boxes (a side
/// picked by length), with the outward direction.
fn hug(o: &arena_core::Obstacle, s: u32, d: f32) -> Option<(Vec2, Vec2)> {
    let boxes: Vec<(Vec3, Quat, Vec3)> = o.world_boxes().filter(|(c, _, h)| c.y - h.y < o.pos.y + 3.0).collect();
    if boxes.is_empty() {
        return None;
    }
    let (c, r, h) = boxes[(hash(s, 1) * boxes.len() as f32) as usize % boxes.len()];
    let c2 = Vec2::new(c.x, c.z);
    if o.kind.is_cylinder() {
        let a = hash(s, 2) * std::f32::consts::TAU;
        let n = Vec2::new(a.cos(), a.sin());
        return Some((c2 + n * (h.x + d), n));
    }
    let ax = r * Vec3::X;
    let az = r * Vec3::Z;
    let ax = Vec2::new(ax.x, ax.z).normalize_or(Vec2::X);
    let az = Vec2::new(az.x, az.z).normalize_or(Vec2::Y);
    let sign = if hash(s, 3) < 0.5 { -1.0 } else { 1.0 };
    let t = hash(s, 4) * 2.0 - 1.0;
    let (n, along, ext, off) = if hash(s, 2) * (h.x + h.z) < h.x { (az * sign, ax, h.x, h.z) } else { (ax * sign, az, h.z, h.x) };
    Some((c2 + n * (off + d) + along * t * ext, n))
}

/// Distance from `p` to the polyline `pts`.
fn polyline_dist(pts: &[Vec2], p: Vec2) -> f32 {
    pts.windows(2)
        .map(|w| {
            let ab = w[1] - w[0];
            let t = ((p - w[0]).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            (w[0] + ab * t).distance(p)
        })
        .fold(f32::MAX, f32::min)
}

/// Where every dressing prop goes, as a pure function of the map: dense in
/// and round the places, sparse and grouped in the open, nothing on the
/// spawns, the pickup sites or the wells.
fn dressing_plan(map: &Map) -> Vec<Dressing> {
    use Clutter::*;
    let desert = map.id == arena_core::MapId::Sink;
    let mut out: Vec<Dressing> = Vec::new();
    let mut keep_clear: Vec<(Vec2, f32)> = Vec::new();
    for s in &map.spawns {
        keep_clear.push((Vec2::new(s.pos.x, s.pos.z), 7.0));
    }
    for w in &map.wells {
        keep_clear.push((Vec2::new(w.pos.x, w.pos.z), 8.0));
    }
    keep_clear.push((Vec2::new(map.outpost.x, map.outpost.z), 14.0));
    for s in &map.scrap_sites {
        keep_clear.push((Vec2::new(s.pos.x, s.pos.z), 2.5));
    }
    for p in map.repair_sites.iter().chain(&map.rare_sites).chain(&map.strike_sites) {
        keep_clear.push((Vec2::new(p.x, p.z), 3.0));
    }
    let clear_of_sites = |p: Vec2| keep_clear.iter().all(|(c, r)| c.distance(p) > *r);
    let lim = map.play_half - 4.0;
    let in_bounds = |p: Vec2| p.x.abs() < lim && p.y.abs() < lim;
    let decks: Vec<&arena_core::Obstacle> = map.obstacles.iter().filter(|o| o.cover == Cover::Flat).collect();
    let on_deck = |p: Vec2| {
        decks.iter().any(|o| {
            Vec2::new(o.pos.x, o.pos.z).distance(p) < o.radius + 4.0
                && o.world_boxes().any(|(c, r, h)| {
                    let l = r.inverse() * (Vec3::new(p.x, c.y, p.y) - c);
                    l.x.abs() < h.x + 0.4 && l.z.abs() < h.z + 0.4
                })
        })
    };
    let free = |p: Vec2, margin: f32| in_bounds(p) && !map.blocked_xz(p, margin) && clear_of_sites(p);
    let push = |out: &mut Vec<Dressing>, kind: Clutter, v: u32, pos: Vec2, yaw: f32| out.push(Dressing { kind, v, pos, yaw });

    // 1. Round the obstacles: the places' clutter.
    for (oi, o) in map.obstacles.iter().enumerate() {
        if o.cover == Cover::Flat {
            continue;
        }
        let ground = o.world_boxes().filter(|(c, _, h)| c.y - h.y < o.pos.y + 3.0).count();
        let (n, (d0, d1), table) = recipe(o.kind, ground, desert);
        let rocky = matches!(o.kind, ObstacleKind::Mesa | ObstacleKind::RockFormation);
        for k in 0..n as u32 {
            let s = oi as u32 * 7919 + k * 31 + 1;
            // Light obstacles get their few props only some of the time.
            if n == 1 && hash(s, 20) < 0.4 {
                continue;
            }
            let d = d0 + (d1 - d0) * hash(s, 21).powf(if rocky { 1.6 } else { 1.0 });
            let Some((p, n_out)) = hug(o, s, d) else { continue };
            let kind = pick(table, hash(s, 22));
            if kind == Cactus && !desert {
                continue;
            }
            let margin = if matches!(kind, Pebbles | Tuft | Sheet) { 0.1 } else { 0.35 };
            if !free(p, margin) || (on_deck(p) && !matches!(kind, Pebbles | Sheet | Tyre)) {
                continue;
            }
            // Boulders by rock: mostly small, a few big ones close in.
            let v = if kind == Boulder { (hash(s, 23) * hash(s, 24) * 6.0) as u32 + if d < 2.0 { 2 } else { 0 } } else { s };
            let yaw = if kind == GroundPipe { (-n_out.x).atan2(-n_out.y) } else { hash(s, 25) * std::f32::consts::TAU };
            push(&mut out, kind, v, p, yaw);
            // Stacks: crates and pallets come in twos and threes.
            if matches!(kind, Crates | Pallet | Drum | Cinder) && hash(s, 26) < 0.55 {
                let along = Vec2::new(-n_out.y, n_out.x) * (1.1 + 0.4 * hash(s, 27)) * if hash(s, 28) < 0.5 { -1.0 } else { 1.0 };
                let q = p + along;
                if free(q, 0.35) && !on_deck(q) {
                    push(&mut out, kind, s + 1, q, hash(s, 29) * std::f32::consts::TAU);
                }
            }
        }
        // Sand drifts against the walls, the hulls and the container rows.
        let drifts = match o.kind {
            ObstacleKind::Wall => 1 + (hash(oi as u32, 40) < 0.5) as u32,
            ObstacleKind::ShipHull => 4,
            ObstacleKind::ContainerStack => (hash(oi as u32, 41) < 0.35) as u32,
            ObstacleKind::ControlHouse | ObstacleKind::GasStation => 2,
            _ => 0,
        };
        for k in 0..drifts {
            let s = oi as u32 * 613 + k * 17 + 5;
            let Some((p, n_out)) = hug(o, s, 0.0) else { continue };
            let mid = p + n_out * 1.5;
            if !in_bounds(mid) || map.blocked_xz(mid, 0.5) || on_deck(mid) || !clear_of_sites(mid) {
                continue;
            }
            push(&mut out, Drift, s, p, n_out.x.atan2(n_out.y));
        }
    }

    // 2. Along the highway: debris trails round the wrecks on the road,
    // grass and the odd cactus on the shoulders.
    let wrecks: Vec<Vec2> = map.obstacles.iter().filter(|o| o.kind == ObstacleKind::CarWreck).map(|o| Vec2::new(o.pos.x, o.pos.z)).filter(|p| on_deck(*p)).collect();
    for (wi, w) in wrecks.iter().enumerate() {
        for k in 0..7u32 {
            let s = wi as u32 * 101 + k + 9000;
            let a = hash(s, 1) * std::f32::consts::TAU;
            let p = *w + Vec2::new(a.cos(), a.sin()) * (2.5 + 7.0 * hash(s, 2));
            if free(p, 0.1) {
                let kind = pick(&[(Pebbles, 4), (Sheet, 2), (Tyre, 2), (Cinder, 1)], hash(s, 3));
                push(&mut out, kind, s, p, hash(s, 4) * 6.28);
            }
        }
    }
    let road: Vec<&arena_core::Obstacle> = decks.iter().copied().filter(|o| o.kind == ObstacleKind::Highway).collect();
    for (ri, o) in road.iter().enumerate() {
        let fwd = o.rot * Vec3::X;
        let fwd = Vec2::new(fwd.x, fwd.z).normalize_or(Vec2::X);
        let side = Vec2::new(-fwd.y, fwd.x);
        let half = o.boxes.first().map_or(6.5, |b| b.half.z);
        for (k, sgn) in [-1.0f32, 1.0].into_iter().enumerate() {
            let s = ri as u32 * 53 + k as u32 + 7000;
            if hash(s, 1) > 0.45 {
                continue;
            }
            let c = Vec2::new(o.pos.x, o.pos.z) + side * sgn * (half + 1.5 + 3.0 * hash(s, 2)) + fwd * (hash(s, 3) - 0.5) * 12.0;
            let n = 2 + (hash(s, 4) * 3.0) as u32;
            for j in 0..n {
                let q = c + Vec2::new(hash(j, s) - 0.5, hash(j, s + 1) - 0.5) * 2.5;
                if free(q, 0.1) && !on_deck(q) {
                    let kind = if j == 0 && desert && hash(s, 5) < 0.3 { Cactus } else if hash(j, s + 2) < 0.75 { Tuft } else { Shrub };
                    push(&mut out, kind, s + j, q, hash(j, s + 3) * 6.28);
                }
            }
        }
    }

    // 3. The dirt tracks: grass on their edges, the odd lost tyre; on the
    // Refinery, pipe runs along the service tracks.
    let tracks: Vec<&arena_core::map::GroundMark> = map.marks.iter().filter(|m| m.kind == GroundMarkKind::Track).collect();
    for (ti, t) in tracks.iter().enumerate() {
        let mut along = 0.0;
        for w in t.points.windows(2) {
            let d = (w[1] - w[0]).normalize_or(Vec2::X);
            let side = Vec2::new(-d.y, d.x);
            let len = w[0].distance(w[1]);
            let mut x = 0.0;
            while x < len {
                let s = ti as u32 * 977 + (along + x) as u32;
                if hash(s, 1) < 0.4 {
                    let sgn = if hash(s, 2) < 0.5 { -1.0 } else { 1.0 };
                    let q = w[0] + d * x + side * sgn * (t.width * 0.5 + 0.4 + 1.5 * hash(s, 3));
                    if free(q, 0.1) && !on_deck(q) {
                        let kind = pick(&[(Tuft, 6), (Pebbles, 2), (Shrub, 1), (Tyre, 1)], hash(s, 4));
                        push(&mut out, kind, s, q, hash(s, 5) * 6.28);
                    }
                }
                x += 12.0;
            }
            along += len;
        }
        // Pipe runs: every other refinery track has a line of pipe on sleepers.
        if !desert && ti % 2 == 0 {
            for w in t.points.windows(2) {
                let d = (w[1] - w[0]).normalize_or(Vec2::X);
                let side = Vec2::new(-d.y, d.x);
                let len = w[0].distance(w[1]);
                let yaw = (-d.y).atan2(d.x);
                let mut x = 4.0;
                while x < len - 4.0 {
                    let q = w[0] + d * x + side * (t.width * 0.5 + 2.2);
                    if free(q, 0.6) && !on_deck(q) {
                        push(&mut out, GroundPipe, ti as u32, q, yaw);
                    }
                    x += 7.4;
                }
            }
        }
    }

    // 4. The compounds' floors: piles lining their edges, inside the walls
    // (crate stacks, drum clusters, pallets, sandbag nests, tarps).
    for (pi, m) in map.marks.iter().filter(|m| m.kind == GroundMarkKind::Packed).enumerate() {
        for w in m.points.windows(2) {
            let d = (w[1] - w[0]).normalize_or(Vec2::X);
            let side = Vec2::new(-d.y, d.x);
            let len = w[0].distance(w[1]);
            for sgn in [-1.0f32, 1.0] {
                // The two long edges, and the two ends.
                let mut edges = vec![(w[0] + side * sgn * (m.width * 0.5 - 4.0), d, len)];
                if sgn > 0.0 {
                    edges.push((w[0] + d * 3.0 - side * (m.width * 0.5 - 6.0), side, m.width - 12.0));
                    edges.push((w[1] - d * 3.0 - side * (m.width * 0.5 - 6.0), side, m.width - 12.0));
                }
                for (ei, (start, dir, l)) in edges.into_iter().enumerate() {
                    let mut x = 3.0;
                    while x < l - 3.0 {
                        let s = pi as u32 * 7717 + ei as u32 * 997 + (sgn > 0.0) as u32 * 31 + x as u32 + 50_000;
                        if hash(s, 1) < 0.55 {
                            let c = start + dir * x;
                            let n = 3 + (hash(s, 2) * 4.0) as u32;
                            let table: &[(Clutter, u32)] = &[(Crates, 4), (Pallet, 3), (Drum, 3), (Cinder, 2), (Jerry, 2), (Tarp, 1), (Sandbags, 1), (TyreStack, 1)];
                            let lead = pick(table, hash(s, 3));
                            for k in 0..n {
                                let q = c + dir * (k as f32 * 1.3 - n as f32 * 0.6) + Vec2::new(-dir.y, dir.x) * (hash(k, s) - 0.5) * 1.6;
                                let kind = if hash(k, s + 1) < 0.6 { lead } else { pick(table, hash(k, s + 2)) };
                                if free(q, 0.4) && !on_deck(q) {
                                    push(&mut out, kind, s + k, q, (-dir.y).atan2(dir.x) + (hash(k, s + 3) - 0.5) * 0.6);
                                }
                            }
                        }
                        x += 9.0;
                    }
                }
            }
        }
    }

    // 5. The open ground: a few rock outcrops, cactus groves and bush
    // clumps, well away from cover, tracks and compounds; clean sand between.
    let marks: Vec<&arena_core::map::GroundMark> = map.marks.iter().filter(|m| m.kind != GroundMarkKind::Oil).collect();
    let open = |p: Vec2, clearance: f32| {
        free(p, clearance) && !on_deck(p) && marks.iter().all(|m| polyline_dist(&m.points, p) > m.width * 0.5 + 8.0)
    };
    let groups: [(Clutter, usize, u32); 3] = if desert { [(Boulder, 12, 1), (Cactus, 8, 2), (Shrub, 28, 3)] } else { [(Boulder, 7, 1), (Shrub, 18, 3), (Pebbles, 6, 4)] };
    for (gk, count, salt_k) in groups {
        let mut made = 0;
        let mut i = 0u32;
        while made < count && i < 400 {
            i += 1;
            let s = salt_k * 10_000 + i;
            let c = Vec2::new((hash(s, 1) - 0.5) * 2.0 * lim, (hash(s, 2) - 0.5) * 2.0 * lim);
            if !open(c, 14.0) || out.iter().any(|d| d.pos.distance(c) < 18.0 && matches!(d.kind, Boulder | Cactus)) {
                continue;
            }
            made += 1;
            match gk {
                Boulder => {
                    // An outcrop: one or two big rocks, smaller ones round them, scrub.
                    let n = 4 + (hash(s, 3) * 4.0) as u32;
                    for k in 0..n {
                        let a = hash(k, s) * std::f32::consts::TAU;
                        let q = c + Vec2::new(a.cos(), a.sin()) * (if k < 2 { 1.2 } else { 2.0 + 3.0 * hash(k, s + 1) });
                        let size = if k < 2 { 3 + (hash(k, s + 2) * 3.0) as u32 } else { (hash(k, s + 2) * 3.0) as u32 };
                        push(&mut out, Boulder, size, q, hash(k, s + 3) * 6.28);
                    }
                    for k in 0..3u32 {
                        let a = hash(k, s + 4) * std::f32::consts::TAU;
                        let q = c + Vec2::new(a.cos(), a.sin()) * (4.0 + 3.0 * hash(k, s + 5));
                        push(&mut out, if k == 0 { Shrub } else { Tuft }, s + k, q, hash(k, s + 6) * 6.28);
                    }
                    push(&mut out, Pebbles, s, c + Vec2::new(3.5, -2.0), 0.0);
                }
                Cactus => {
                    // A grove: cacti in loose pairs, grass at their feet.
                    let n = 4 + (hash(s, 3) * 6.0) as u32;
                    for k in 0..n {
                        let a = hash(k, s) * std::f32::consts::TAU;
                        let q = c + Vec2::new(a.cos(), a.sin()) * (10.0 * hash(k, s + 1).sqrt());
                        push(&mut out, Cactus, s + k, q, hash(k, s + 2) * 6.28);
                        if hash(k, s + 3) < 0.6 {
                            push(&mut out, Tuft, s + k, q + Vec2::new(0.8, 0.5), 0.0);
                        }
                    }
                    push(&mut out, Shrub, s, c + Vec2::new(-4.0, 3.0), 0.0);
                    push(&mut out, Boulder, 1, c + Vec2::new(3.0, 5.0), hash(s, 4) * 6.28);
                }
                _ => {
                    let n = 2 + (hash(s, 3) * 3.0) as u32;
                    for k in 0..n {
                        let q = c + Vec2::new(hash(k, s) - 0.5, hash(k, s + 1) - 0.5) * 3.0;
                        let kind = if gk == Pebbles { if k == 0 { Boulder } else { Pebbles } } else if k == 0 { Shrub } else { Tuft };
                        push(&mut out, kind, if kind == Boulder { 1 } else { s + k }, q, hash(k, s + 2) * 6.28);
                    }
                }
            }
        }
    }
    out
}

// ── Line-ups: the consistency checks, and the end-screen podium ──────────

/// A tank parked for a line-up: `parts` bolted on, standing on y = 0.
#[allow(dead_code)] // called by the game layer (shots, end screen)
pub fn parked_tank(player: PlayerId, colour: u32, pos: Vec3, yaw: f32, gun: PartKind, parts: Vec<PartKind>) -> TankView {
    TankView {
        class: Default::default(),
        player,
        colour,
        pos: pos + Vec3::Y * tank::HULL_Y,
        rot: Quat::from_rotation_y(yaw),
        turret_yaw: 0.25,
        gun_pitch: 0.04,
        wheels: [0.15; 10],
        track_speed: [0.0; 2],
        gun,
        parts,
        alive: true,
        boosting: false,
        burning: false,
        throttle: 0.0,
        speed: 0.0,
    }
}

#[allow(dead_code)] // called by the game layer (shots, end screen)
fn stand(r: &mut Renderer, statics: &mut Vec<Instance>, m: &MeshBuilder, x: f32, z: f32, yaw: f32) {
    let id = r.add_mesh(m);
    statics.push(Instance::new(id, Mat4::from_rotation_translation(Quat::from_rotation_y(yaw), Vec3::new(x, 0.0, z))));
}

#[allow(dead_code)] // called by the game layer (shots, end screen)
fn on_ground(mut m: MeshBuilder, dy: f32) -> MeshBuilder {
    m.transform_from(0, Mat4::from_translation(Vec3::Y * dy));
    m
}

/// Every prop in a grid on flat sand, in the game's light, for checking the
/// kit reads as one family. Sets the static scene; returns the world.
#[allow(dead_code)] // called by the game layer (shots, end screen)
pub fn lineup_props(wr: &mut WorldRenderer) -> RenderWorld {
    wr.begin_scene();
    let r = &mut wr.renderer;
    let mut statics = Vec::new();
    let floor = r.add_mesh(&crate::terrain::flat(90.0));
    statics.push(Instance::new(floor, Mat4::IDENTITY));
    let small: Vec<MeshBuilder> = vec![
        props::hedgehog(),
        props::razor_wire(3.0),
        props::tyres(1, 1),
        props::tyres(3, 2),
        props::crates(1),
        on_ground(props::oil_drum(0x34587e, 0.6), props::DRUM_HALF),
        on_ground(props::oil_drum(0x8c2a1e, 0.6), props::DRUM_HALF),
        props::cactus(1),
        props::cactus(2),
        props::boulder(3, 0.9),
        props::boulder(8, 0.5),
        props::mine(),
        on_ground(props::wrecking_ball(1.4), 1.4),
        props::pickup(0),
        props::pickup(1),
        props::pickup(2),
        props::pickup(3),
        props::pickup(4),
    ];
    for (i, m) in small.iter().enumerate() {
        let (col, row) = (i % 9, i / 9);
        stand(r, &mut statics, m, -16.0 + col as f32 * 4.0, 8.0 + row as f32 * 4.5, 0.4);
    }
    let mid: Vec<(MeshBuilder, f32)> = vec![
        (on_ground(props::barrier(Vec3::new(1.7, 0.5, 0.35), false), 0.5), 0.0),
        (on_ground(props::barrier(Vec3::new(1.7, 0.5, 0.35), true), 0.5), 0.0),
        (on_ground(props::sandbags(Vec3::new(1.5, 0.45, 0.55), 3), 0.45), 0.0),
        (on_ground(props::pipe(Vec3::new(3.0, 0.55, 0.55)), 0.55), 0.3),
        (on_ground(props::car_wreck(Vec3::new(2.2, 0.5, 0.95), 2), 0.5), 0.5),
        (on_ground(props::car_wreck(Vec3::new(2.2, 0.5, 0.95), 5), 0.5), -0.4),
        (on_ground(props::pier_ruin(Vec3::new(3.4, 0.45, 2.2), 4), 0.45), 0.0),
        (props::launch_plate(Vec3::new(3.0, 0.3, 2.6)), 0.0),
        (props::fuel_tank(), 0.3),
    ];
    for (i, (m, yaw)) in mid.iter().enumerate() {
        let (col, row) = (i % 5, i / 5);
        stand(r, &mut statics, m, -18.0 + col as f32 * 9.0, -2.0 - row as f32 * 7.0, *yaw);
    }
    let big: Vec<(MeshBuilder, f32, f32)> = vec![
        (on_ground(props::container(Vec3::new(6.1, 1.3, 1.22), props::CONTAINER_COLOURS[0]), 1.3), 0.0, -24.0),
        (on_ground(props::container(Vec3::new(6.1, 1.3, 1.22), props::CONTAINER_COLOURS[1]), 1.3), 0.0, -27.0),
        (on_ground(props::container(Vec3::new(6.1, 1.3, 1.22), props::CONTAINER_COLOURS[2]), 1.3), 15.0, -24.0),
        (on_ground(props::silo(3.0, 4.5), 4.5), -16.0, -26.0),
        (on_ground(props::rock_block(Vec3::new(3.0, 3.0, 2.6), 5), 3.0), 26.0, -24.0),
        (props::power_pole(), 33.0, -14.0),
        (on_ground(props::highway(Vec3::new(8.05, 0.3, 6.5)), 0.3), 30.0, 2.0),
    ];
    for (m, x, z) in &big {
        stand(r, &mut statics, m, *x, *z, 0.0);
    }
    let crane = props::crane((Vec3::new(0.0, 11.5, 0.0), Vec3::new(1.5, 12.0, 1.5)), (Vec3::new(7.0, 23.8, 0.0), Vec3::new(9.0, 0.6, 0.6)));
    stand(r, &mut statics, &crane, -36.0, -30.0, 0.6);
    stand(r, &mut statics, &on_ground(props::mesa_block(Vec3::new(9.0, 5.0, 6.0), 11), 5.0), 4.0, -42.0, 0.0);
    wr.set_statics(statics);
    RenderWorld { camera: Some(Camera::look(Vec3::new(0.0, 19.0, 30.0), Vec3::new(0.0, 1.0, -6.0), 62.0)), ..RenderWorld::default() }
}

/// Every gun and part on its own tank, in a grid: guns on teal hulls in
/// the front row, the rest on orange hulls behind.
#[allow(dead_code)] // called by the game layer (shots, end screen)
pub fn lineup_parts(wr: &mut WorldRenderer) -> RenderWorld {
    wr.begin_scene();
    let r = &mut wr.renderer;
    let floor = r.add_mesh(&crate::terrain::flat(90.0));
    wr.set_statics(vec![Instance::new(floor, Mat4::IDENTITY)]);
    let mut world = RenderWorld::default();
    let guns: Vec<PartKind> = parts::ALL_PARTS.iter().copied().filter(|k| parts::is_gun(*k)).collect();
    let others: Vec<PartKind> = parts::ALL_PARTS.iter().copied().filter(|k| !parts::is_gun(*k)).collect();
    for (i, g) in guns.iter().enumerate() {
        world.tanks.push(parked_tank(i as PlayerId, tank::DUST_SAINT, Vec3::new(-24.0 + i as f32 * 12.0, 0.0, 10.0), 0.55, *g, vec![]));
    }
    for (i, p) in others.iter().enumerate() {
        let (col, row) = (i % 6, i / 6);
        let pos = Vec3::new(-25.0 + col as f32 * 10.0, 0.0, -1.0 - row as f32 * 9.5);
        world.tanks.push(parked_tank(20 + i as PlayerId, tank::RUSTBACK, pos, 0.55, PartKind::HeavyCannon, vec![*p]));
    }
    world.camera = Some(Camera::look(Vec3::new(0.0, 21.0, 33.0), Vec3::new(0.0, 0.0, -5.0), 60.0));
    world
}

/// The end-screen podium: up to three tanks with their final loadouts on
/// stepped blocks (winner in the middle, highest), on sand at golden hour,
/// framed for the empty middle of the UI's end screen. Sets the static
/// scene; returns the world to draw (camera included).
#[allow(dead_code)] // called by the game layer (shots, end screen)
pub fn podium(wr: &mut WorldRenderer, top: &[TankView]) -> RenderWorld {
    wr.begin_scene();
    let r = &mut wr.renderer;
    let mut statics = Vec::new();
    let floor = r.add_mesh(&crate::terrain::flat(90.0));
    statics.push(Instance::new(floor, Mat4::IDENTITY));
    // Places: 1st centre (2.2 m), 2nd left (1.5 m), 3rd right (0.9 m).
    let slots = [(Vec3::new(0.0, 0.0, 0.0), 2.2), (Vec3::new(-8.0, 0.0, 1.2), 1.5), (Vec3::new(8.0, 0.0, 1.2), 0.9)];
    let mut world = RenderWorld::default();
    for (i, t) in top.iter().take(3).enumerate() {
        let (p, h) = slots[i];
        let block = r.add_mesh(&props::podium_block(h, 0x3a3734));
        statics.push(Instance::new(block, Mat4::from_translation(p)));
        let mut v = t.clone();
        v.pos = p + Vec3::Y * (h + tank::HULL_Y);
        v.rot = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2 + [0.0, 0.35, -0.35][i]);
        v.turret_yaw = 0.0;
        v.gun_pitch = 0.12;
        v.alive = true;
        v.track_speed = [0.0; 2];
        v.wheels = [0.15; 10];
        world.tanks.push(v);
    }
    let mesa = props::mesa_block(Vec3::new(16.0, 9.0, 10.0), 7);
    let id = r.add_mesh(&on_ground(mesa, 9.0));
    statics.push(Instance::new(id, Mat4::from_translation(Vec3::new(-10.0, 0.0, -60.0))));
    wr.set_statics(statics);
    world.camera = Some(Camera::look(Vec3::new(0.0, 6.5, 24.0), Vec3::new(0.0, 2.6, 0.0), 42.0));
    world
}

#[cfg(test)]
mod shots {
    //! `cargo test -p arena-client --release -- --ignored shots` renders the
    //! line-up shots to docs/shots (needs a GPU).
    use super::*;

    fn render(name: &str, build: impl FnOnce(&mut WorldRenderer) -> RenderWorld) {
        let (w, h) = (1600u32, 900u32);
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::PRIMARY, ..wgpu::InstanceDescriptor::new_without_display_handle() });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).expect("adapter");
        let (device, queue) = pollster::block_on(crate::renderer::request_device(&adapter)).expect("device");
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let mut wr = WorldRenderer::new(Renderer::new(device, queue, format, w, h));
        let world = build(&mut wr);
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
        wr.draw(&texture.create_view(&Default::default()), &world);
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

    #[test]
    #[ignore]
    fn shots_lineups() {
        render("lineup-props", lineup_props);
        render("lineup-parts", lineup_parts);
        render("podium", |wr| {
            let top = vec![
                parked_tank(1, tank::RUSTBACK, Vec3::ZERO, 0.0, PartKind::BigBore, vec![PartKind::FlankPlates, PartKind::SensorMast, PartKind::Nitro, PartKind::ShieldGenerator]),
                parked_tank(2, tank::DUST_SAINT, Vec3::ZERO, 0.0, PartKind::LongGun, vec![PartKind::SpacedArmour, PartKind::Coilgun, PartKind::Afterburner]),
                parked_tank(3, PAINT_SET[3], Vec3::ZERO, 0.0, PartKind::TwinBarrel, vec![PartKind::Bulwark, PartKind::TeslaSpire]),
            ];
            podium(wr, &top)
        });
    }

    /// Every shader parses and validates (no GPU needed): a broken shader
    /// once panicked the native window at start.
    #[test]
    fn shaders_validate() {
        use wgpu::naga;
        let scene = include_str!("shaders/scene.wgsl");
        let sources = [
            ("scene", scene.to_string()),
            ("scene+fx", format!("{scene}
{}", include_str!("shaders/fx.wgsl"))),
            ("post", include_str!("shaders/post.wgsl").to_string()),
            ("bloom", include_str!("shaders/bloom.wgsl").to_string()),
        ];
        for (name, src) in sources {
            let module = naga::front::wgsl::parse_str(&src).unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&src)));
            naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
                .validate(&module)
                .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        }
    }

    /// Raider Waves played until the War Rig has been on the field a while.
    #[test]
    #[ignore]
    fn shots_raid() {
        render(if std::env::var("TANSK_RAID_CLOSE").is_ok() { "raid-close" } else { "raid" }, |wr| {
            let seed = 7;
            let (mut sim, _me) = arena_core::Sim::practice_raid(seed, 3, 0.7);
            wr.set_map(&arena_core::Map::the_sink(seed));
            let dt = arena_core::DT;
            let mut world: RenderWorld;
            let mut boss_ticks = 0;
            let limit = 60 * 60 * 20;
            let mut tick = 0;
            while tick < limit {
                sim.step();
                tick += 1;
                let events = sim.drain_events();
                let boss = tick % 30 == 0 && !sim.snapshot().bosses.is_empty();
                if boss || boss_ticks > 0 {
                    boss_ticks += 1;
                }
                // Effects for the last stretch before the picture.
                let late = boss_ticks > 60 * 15 || tick > limit - 600;
                if late {
                    world = RenderWorld::from_snapshot(&sim.snapshot(), &paint_for);
                    world.time = tick as f32 * dt;
                    for e in &events {
                        wr.event(e, Vec3::ZERO);
                    }
                    wr.fx.update(dt, &world);
                }
                if boss_ticks > 60 * 25 {
                    break;
                }
            }
            let snap = sim.snapshot();
            world = RenderWorld::from_snapshot(&snap, &paint_for);
            world.time = tick as f32 * dt;
            let rig = snap.bosses.first().and_then(|b| world.tanks.iter().find(|t| t.player == b.player)).map(|t| t.pos);
            let raider = world.vehicles.first().and_then(|v| world.tanks.iter().find(|t| t.player == v.0)).map(|t| t.pos);
            let focus = rig.or(raider).or(world.raid.map(|r| r.0)).unwrap_or(Vec3::ZERO);
            let close = std::env::var("TANSK_RAID_CLOSE").is_ok();
            let listener = focus + if close { Vec3::new(9.0, 4.0, 9.0) } else { Vec3::new(24.0, 12.0, 24.0) };
            world.camera = Some(Camera::look(listener, focus + Vec3::Y * 1.2, 50.0));
            eprintln!(
                "raid shot: tick {tick}, boss {}, raiders {}, tanks {}, raid {:?}",
                rig.is_some(),
                world.vehicles.len(),
                world.tanks.len(),
                world.raid
            );
            world.dt = 0.0;
            log::info!("raid shot: boss {:?} at {focus:?}, {} tanks, raid {:?}", rig.is_some(), world.tanks.len(), world.raid);
            world
        });
    }

    /// The Refinery in play: a practice match run a while, then views of
    /// each place (`refinery-*.png`) and a chase shot behind a tank.
    #[test]
    #[ignore]
    fn shots_refinery() {
        let seed = 7;
        let views: [(&str, Vec3, Vec3); 6] = [
            ("refinery-farm", Vec3::new(95.0, 30.0, -40.0), Vec3::new(180.0, 4.0, 40.0)),
            ("refinery-towers", Vec3::new(60.0, 5.0, -105.0), Vec3::new(150.0, 9.0, -190.0)),
            ("refinery-derricks", Vec3::new(-90.0, 9.0, 130.0), Vec3::new(-180.0, 3.0, 40.0)),
            ("refinery-yard", Vec3::new(-40.0, 12.0, 160.0), Vec3::new(40.0, 2.0, 225.0)),
            ("refinery-flare", Vec3::new(-110.0, 10.0, -110.0), Vec3::new(-185.0, 18.0, -185.0)),
            ("refinery-overview", Vec3::new(120.0, 110.0, 220.0), Vec3::new(0.0, 0.0, -20.0)),
        ];
        for (name, eye, look) in views {
            render(name, |wr| {
                let (mut sim, _) = arena_core::Sim::practice_on(arena_core::MapId::Refinery, seed, 7, 0.7);
                wr.set_map(&arena_core::Map::by_id(arena_core::MapId::Refinery, seed));
                let mut world = RenderWorld::default();
                for tick in 0..(60 * 30) {
                    sim.step();
                    let ev = sim.drain_events();
                    if tick > 60 * 22 {
                        world = RenderWorld::from_snapshot(&sim.snapshot(), &paint_for);
                        world.time = tick as f32 * arena_core::DT;
                        world.camera = Some(Camera::look(eye, look, 55.0));
                        for e in &ev {
                            wr.event(e, eye);
                        }
                        wr.fx.update(arena_core::DT, &world);
                    }
                }
                world.dt = 0.0;
                world
            });
        }
        render("refinery-play", |wr| {
            let (mut sim, me) = arena_core::Sim::practice_on(arena_core::MapId::Refinery, seed, 7, 0.7);
            wr.set_map(&arena_core::Map::by_id(arena_core::MapId::Refinery, seed));
            let mut world = RenderWorld::default();
            for tick in 0..(60 * 45) {
                // Drive the human tank toward the tank farm.
                if let Some((p, _)) = sim.tank_pose(me) {
                    let to = Vec3::new(150.0, p.y, 30.0) - p;
                    let yaw = sim.hull_yaw(me).unwrap_or(0.0);
                    let want = (-to.z).atan2(to.x);
                    let mut d = want - yaw;
                    while d > std::f32::consts::PI { d -= std::f32::consts::TAU; }
                    while d < -std::f32::consts::PI { d += std::f32::consts::TAU; }
                    sim.set_input(me, arena_core::Input { throttle: 1.0, steer: (-d * 2.0).clamp(-1.0, 1.0), aim_point: p + to, ..Default::default() });
                }
                sim.step();
                let ev = sim.drain_events();
                if tick > 60 * 37 {
                    world = RenderWorld::from_snapshot(&sim.snapshot(), &paint_for);
                    world.time = tick as f32 * arena_core::DT;
                    let t = world.tanks.iter().find(|t| t.player == me).map(|t| (t.pos, t.rot)).unwrap_or((Vec3::ZERO, Quat::IDENTITY));
                    let fwd = t.1 * Vec3::X;
                    let eye = t.0 - fwd * 11.0 + Vec3::Y * 4.5;
                    world.camera = Some(Camera::look(eye, t.0 + fwd * 20.0 + Vec3::Y * 2.0, 58.0));
                    for e in &ev {
                        wr.event(e, t.0);
                    }
                    wr.fx.update(arena_core::DT, &world);
                }
            }
            world.dt = 0.0;
            world
        });
    }

    /// The level layout of both maps, without a match: high overviews and
    /// chase-camera views at each place (`level-<map>-<view>.png`; set
    /// `TANSK_SHOT_TAG` to prefix them, e.g. for before/after sets).
    #[test]
    #[ignore]
    fn shots_level() {
        let tag = std::env::var("TANSK_SHOT_TAG").map(|t| format!("{t}-")).unwrap_or_default();
        let only = std::env::var("TANSK_SHOT_ONLY").unwrap_or_default();
        let seed = 7;
        // (name, eye, look at); a chase view (eye under 12 m) stands a tank 11 m ahead.
        let sink: [(&str, Vec2, f32, Vec2); 13] = [
            ("high", Vec2::new(30.0, 250.0), 170.0, Vec2::new(0.0, -20.0)),
            ("air-outpost", Vec2::new(60.0, 70.0), 45.0, Vec2::new(0.0, 0.0)),
            ("air-yard", Vec2::new(-90.0, 175.0), 45.0, Vec2::new(-150.0, 120.0)),
            ("air-gas", Vec2::new(135.0, -85.0), 40.0, Vec2::new(95.0, -135.0)),
            ("air-beach", Vec2::new(130.0, 110.0), 45.0, Vec2::new(195.0, 50.0)),
            ("outpost", Vec2::new(-20.0, 78.0), 4.5, Vec2::new(0.0, 0.0)),
            ("roadblock", Vec2::new(-130.0, -128.0), 4.5, Vec2::new(-60.0, -110.0)),
            ("gas", Vec2::new(50.0, -95.0), 4.5, Vec2::new(92.0, -138.0)),
            ("yard", Vec2::new(-80.0, 150.0), 4.5, Vec2::new(-150.0, 120.0)),
            ("beach", Vec2::new(140.0, 70.0), 4.5, Vec2::new(215.0, 80.0)),
            ("plateau", Vec2::new(-90.0, -150.0), 4.5, Vec2::new(-160.0, -205.0)),
            ("desert", Vec2::new(60.0, 160.0), 4.5, Vec2::new(120.0, 230.0)),
            ("track", Vec2::new(10.0, 160.0), 4.5, Vec2::new(25.0, 60.0)),
        ];
        let refinery: [(&str, Vec2, f32, Vec2); 13] = [
            ("high", Vec2::new(30.0, 250.0), 170.0, Vec2::new(0.0, -20.0)),
            ("air-control", Vec2::new(60.0, 70.0), 45.0, Vec2::new(0.0, 0.0)),
            ("air-farm", Vec2::new(100.0, 115.0), 50.0, Vec2::new(175.0, 40.0)),
            ("air-derricks", Vec2::new(-120.0, 150.0), 45.0, Vec2::new(-180.0, 70.0)),
            ("air-towers", Vec2::new(60.0, -110.0), 50.0, Vec2::new(150.0, -185.0)),
            ("control", Vec2::new(-20.0, 75.0), 4.5, Vec2::new(0.0, 0.0)),
            ("farm", Vec2::new(90.0, 50.0), 4.5, Vec2::new(170.0, 40.0)),
            ("towers", Vec2::new(60.0, -95.0), 4.5, Vec2::new(150.0, -190.0)),
            ("derricks", Vec2::new(-90.0, 120.0), 4.5, Vec2::new(-180.0, 50.0)),
            ("yard", Vec2::new(-40.0, 160.0), 4.5, Vec2::new(40.0, 220.0)),
            ("flare", Vec2::new(-110.0, -110.0), 4.5, Vec2::new(-185.0, -185.0)),
            ("road", Vec2::new(-200.0, -48.0), 4.5, Vec2::new(-100.0, -60.0)),
            ("track", Vec2::new(-60.0, 34.0), 4.5, Vec2::new(-150.0, 50.0)),
        ];
        for (id, views) in [(arena_core::MapId::Sink, sink), (arena_core::MapId::Refinery, refinery)] {
            let map = Map::by_id(id, seed);
            for (name, eye, up, look) in views {
                let shot = format!("level-{tag}{}-{name}", id.key());
                if !only.is_empty() && !shot.contains(&only) {
                    continue;
                }
                render(&shot, |wr| {
                    wr.set_map(&map);
                    eprintln!("{shot}: {} dressing props", wr.dressed);
                    let g = |p: Vec2| map.height_at(p.x, p.y);
                    let mut world = RenderWorld::default();
                    let chase = up < 12.0;
                    let up = if up > 12.0 { up + g(eye) } else { up };
                    let (e, t) = if chase {
                        // A chase camera behind a parked tank.
                        let dir = (look - eye).normalize();
                        let tank_xz = eye + dir * 11.0;
                        let yaw = (-dir.y).atan2(dir.x);
                        world.tanks.push(parked_tank(1, tank::RUSTBACK, Vec3::new(tank_xz.x, g(tank_xz), tank_xz.y), yaw, PartKind::HeavyCannon, vec![]));
                        let e = Vec3::new(eye.x, g(eye) + up, eye.y);
                        let t = tank_xz + dir * 30.0;
                        (e, Vec3::new(t.x, g(t) + 2.0, t.y))
                    } else {
                        (Vec3::new(eye.x, up, eye.y), Vec3::new(look.x, g(look), look.y))
                    };
                    world.camera = Some(Camera::look(e, t, if chase { 58.0 } else { 50.0 }));
                    world.time = 30.0;
                    world
                });
            }
        }
    }

    /// The dressing is clustered, not sprinkled: a bounded count, nothing
    /// inside cover or on a spawn, and most of it near the places.
    #[test]
    fn dressing_is_clustered() {
        for id in arena_core::MapId::ALL {
            let map = Map::by_id(id, 7);
            let plan = dressing_plan(&map);
            assert!(plan.len() > 400 && plan.len() < 2500, "{id:?}: {} props", plan.len());
            let solid: Vec<_> = map.obstacles.iter().filter(|o| o.cover != Cover::Flat).collect();
            let mut near = 0;
            for d in &plan {
                if d.kind != Clutter::Drift {
                    assert!(!map.blocked_xz(d.pos, 0.0), "{id:?}: {:?} inside cover at {}", d.kind, d.pos);
                }
                for s in &map.spawns {
                    assert!(Vec2::new(s.pos.x, s.pos.z).distance(d.pos) > 6.0, "{id:?}: {:?} on a spawn", d.kind);
                }
                if solid.iter().any(|o| Vec2::new(o.pos.x, o.pos.z).distance(d.pos) < o.radius + 12.0) {
                    near += 1;
                }
            }
            assert!(near * 10 > plan.len() * 6, "{id:?}: only {near} of {} props near the places", plan.len());
            assert!(!map.marks.is_empty());
        }
    }

    /// What the static scene is made of (for sizing dressing chunks).
    #[test]
    #[ignore]
    fn map_static_stats() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::PRIMARY, ..wgpu::InstanceDescriptor::new_without_display_handle() });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).expect("adapter");
        let (device, queue) = pollster::block_on(crate::renderer::request_device(&adapter)).expect("device");
        let mut wr = WorldRenderer::new(Renderer::new(device, queue, wgpu::TextureFormat::Rgba8Unorm, 64, 64));
        for id in [arena_core::MapId::Sink, arena_core::MapId::Refinery] {
            wr.renderer.recorded = Some(HashMap::new());
            wr.set_map(&Map::by_id(id, 7));
            let rec = wr.renderer.recorded.take().unwrap_or_default();
            let mut meshes = std::collections::HashSet::new();
            let (mut owned, mut verts, mut small_verts, mut small) = (0, 0usize, 0usize, 0);
            let mut by_mesh: HashMap<MeshId, (usize, usize)> = HashMap::new();
            for s in &wr.statics {
                meshes.insert(s.instance.mesh);
                owned += s.owner.is_some() as usize;
                let v = rec.get(&s.instance.mesh).map_or(0, |m| m.vertices.len());
                verts += v;
                let e = by_mesh.entry(s.instance.mesh).or_insert((0, v));
                e.0 += 1;
                if wr.renderer.mesh_radius(s.instance.mesh) < 2.0 {
                    small += 1;
                    small_verts += v;
                }
            }
            let mut top: Vec<_> = by_mesh.values().map(|(n, v)| (n * v, *n, *v)).collect();
            top.sort();
            top.reverse();
            for lim in [300usize, 600, 1000, 2000, 5000] {
                let (mut n, mut v, mut u) = (0, 0, 0);
                for (_, (cnt, verts)) in by_mesh.iter() {
                    if *verts <= lim {
                        n += cnt;
                        v += cnt * verts;
                        u += 1;
                    }
                }
                println!("  <= {lim} verts: {n} pieces of {u} meshes, {v} verts merged");
            }
            for maxn in [1usize, 2, 4, 8] {
                for lim in [4000usize, 12000, 40000] {
                    let (mut n, mut v, mut u, mut freed) = (0, 0, 0, 0);
                    for (_, (cnt, verts)) in by_mesh.iter() {
                        if *cnt <= maxn && *verts <= lim {
                            n += cnt;
                            v += cnt * verts;
                            u += 1;
                            freed += verts;
                        }
                    }
                    println!("  count <= {maxn}, verts <= {lim}: {n} pieces of {u} meshes, {v} verts merged, {freed} freed");
                }
            }
            let uniq: usize = rec.values().map(|m| m.vertices.len()).sum();
            let uniq_small: usize = rec.values().map(|m| m.vertices.len()).filter(|v| *v <= 40_000).sum();
            println!("  unique recorded: {} meshes, {uniq} verts ({uniq_small} in meshes <= 40k)", rec.len());
            println!("{id:?}: {} statics ({owned} owned), {} meshes, {verts} verts merged, {small} small ({small_verts} verts); top {:?}", wr.statics.len(), meshes.len(), &top[..8.min(top.len())]);
        }
    }

    /// Rebuilding maps and podiums over and over frees what each scene made:
    /// the GPU mesh count comes back to where it was.
    #[test]
    #[ignore]
    fn scenes_free_their_meshes() {
        let (w, h) = (64u32, 64u32);
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::PRIMARY, ..wgpu::InstanceDescriptor::new_without_display_handle() });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).expect("adapter");
        let (device, queue) = pollster::block_on(crate::renderer::request_device(&adapter)).expect("device");
        let mut wr = WorldRenderer::new(Renderer::new(device, queue, wgpu::TextureFormat::Rgba8Unorm, w, h));
        let top = vec![parked_tank(1, tank::RUSTBACK, Vec3::ZERO, 0.0, PartKind::BigBore, vec![PartKind::FlankPlates])];
        let home = Map::by_id(arena_core::MapId::Sink, 7);
        wr.set_map(&home);
        let first = wr.renderer.mesh_count();
        // Practice rolls a fresh seed every time: keys that depend on the
        // seed (sizes) must not pile up.
        for k in 0..6u64 {
            let id = if k % 2 == 0 { arena_core::MapId::Refinery } else { arena_core::MapId::Sink };
            wr.set_map(&Map::by_id(id, 100 + k));
            let _ = podium(&mut wr, &top);
        }
        wr.set_map(&home);
        let again = wr.renderer.mesh_count();
        println!("meshes: {first} first, {again} after 6 maps and podiums, {} pinned", wr.lib.pinned());
        assert_eq!(first, again, "GPU meshes leaked across map rebuilds");
        // (The wreck meshes are pinned on purpose: built with the first map.)
        assert!(wr.lib.ids.keys().all(|k| k.starts_with("wreck:")), "map pieces were pinned: {:?}", wr.lib.ids.keys().collect::<Vec<_>>());
    }

    /// Wellhead's derricks and Last Crew's wall reach the render world.
    #[test]
    fn wells_and_the_storm_wall_reach_the_render_world() {
        let (mut sim, _) = arena_core::Sim::practice_wellhead_on(arena_core::MapId::Refinery, 3, 4, 0.5);
        sim.wellhead.as_mut().unwrap().wells[2].owner = Some(1);
        sim.wellhead.as_mut().unwrap().wells[2].control = 1.0;
        sim.step();
        let world = RenderWorld::from_snapshot(&sim.snapshot(), &paint_for);
        assert_eq!(world.wells.len(), 3);
        assert!(world.well_radius > 5.0);
        assert_eq!(world.wells[2].1, 1);
        assert!(world.wells.iter().take(2).all(|w| w.1 == 255));
        assert!(world.storm_wall.is_none());
        let (mut sim, _) = arena_core::Sim::practice_last_crew_on(arena_core::MapId::Sink, 3, 7, 0.5);
        sim.step();
        let snap = sim.snapshot();
        let world = RenderWorld::from_snapshot(&snap, &paint_for);
        let (c, r) = world.storm_wall.expect("the wall");
        assert!(r > 300.0 && c.length() < 1.0);
        assert!(world.wells.is_empty());
        // Every crew paints its own colour (free-for-all).
        assert!(snap.tanks.iter().all(|t| t.team == 255));
    }

    /// The dust curtain hugs the wall, dense near the eye, and goes away
    /// when the wall has closed to nothing.
    #[test]
    fn the_dust_curtain_stands_on_the_wall() {
        let c = Vec2::new(20.0, -10.0);
        let eye = Vec3::new(20.0 + 90.0, 5.0, -10.0);
        let pts = crate::objectives::curtain_points(c, 100.0, eye, 1.0, &|_, _| 2.0);
        assert!(pts.len() > 100, "{} puffs", pts.len());
        for (p, size, alpha, _) in &pts {
            let d = Vec2::new(p.x, p.z).distance(c);
            assert!((d - 100.0).abs() < 1.0, "puff {d} m from the centre");
            assert!(p.y >= 2.0 && *size > 1.0 && *alpha > 0.0 && *alpha <= 1.0);
        }
        let near = pts.iter().filter(|(p, ..)| p.distance(eye) < 60.0).count();
        assert!(near >= 20, "{near} puffs near the eye");
        assert!(crate::objectives::curtain_points(c, 0.0, eye, 1.0, &|_, _| 0.0).is_empty());
        // A huge circle far away still shows on the horizon, sparsely.
        let far = crate::objectives::curtain_points(Vec2::ZERO, 900.0, Vec3::ZERO, 0.0, &|_, _| 0.0);
        assert!(!far.is_empty() && far.len() < 600, "{}", far.len());
    }

    /// Wellhead's derricks and the capture rings, close up (`wellhead.png`).
    #[test]
    #[ignore]
    fn shots_wellhead() {
        render("wellhead", |wr| {
            let seed = 7;
            let (mut sim, _) = arena_core::Sim::practice_wellhead_on(arena_core::MapId::Sink, seed, 4, 0.7);
            wr.set_map(&arena_core::Map::the_sink(seed));
            let mut world = RenderWorld::default();
            // The middle well half taken by the Dust Saints, before the crews arrive.
            if let Some(w) = sim.wellhead.as_mut() {
                w.wells[0].owner = Some(0);
                w.wells[0].control = -1.0;
                w.wells[1].control = 0.55;
            }
            for tick in 0..(60 * 6) {
                sim.step();
                let ev = sim.drain_events();
                if tick > 60 * 2 {
                    world = RenderWorld::from_snapshot(&sim.snapshot(), &paint_for);
                    world.time = tick as f32 * arena_core::DT;
                    for e in &ev {
                        wr.event(e, Vec3::ZERO);
                    }
                    wr.fx.update(arena_core::DT, &world);
                }
            }
            let w = world.wells[1].0;
            world.camera = Some(Camera::look(w + Vec3::new(-19.0, 9.0, 15.0), w + Vec3::Y * 2.5, 55.0));
            eprintln!("wellhead: {:?}", sim.snapshot().wellhead);
            world.dt = 0.0;
            world
        });
    }

    /// Last Crew's sandstorm wall from inside the circle (`lastcrew.png`).
    #[test]
    #[ignore]
    fn shots_lastcrew() {
        render("lastcrew", |wr| {
            let seed = 7;
            let (mut sim, _) = arena_core::Sim::practice_last_crew_on(arena_core::MapId::Sink, seed, 7, 0.7);
            wr.set_map(&arena_core::Map::the_sink(seed));
            {
                let lc = sim.last_crew.as_mut().unwrap();
                lc.center = Vec2::new(0.0, 0.0);
                lc.radius = 120.0;
                lc.from = (lc.center, lc.radius);
                lc.to = lc.from;
            }
            let mut world = RenderWorld::default();
            for tick in 0..(60 * 6) {
                sim.step();
                let ev = sim.drain_events();
                world = RenderWorld::from_snapshot(&sim.snapshot(), &paint_for);
                world.time = tick as f32 * arena_core::DT;
                for e in &ev {
                    wr.event(e, Vec3::ZERO);
                }
                wr.fx.update(arena_core::DT, &world);
            }
            let eye = Vec3::new(60.0, 7.0, 30.0);
            world.camera = Some(Camera::look(eye, Vec3::new(150.0, 12.0, 60.0), 60.0));
            let cam = world.camera.unwrap();
            let map = arena_core::Map::the_sink(seed);
            wr.fx.curtain = crate::objectives::curtain_points(Vec2::ZERO, 120.0, cam.eye, world.time, &|x, z| map.height_at(x, z));
            world.dt = 0.0;
            world
        });
    }

    /// Team Brawl's team 1 is Dust Saint teal; only raiders wear the raider scheme.
    #[test]
    fn team_colours_follow_the_mode() {
        let mut sim = arena_core::Sim::new_team_brawl(3, Map::the_sink(3), arena_core::Tuning::default());
        for k in 0..4 {
            sim.add_bot(arena_core::bots::BOT_NAMES[k], 0.5);
        }
        sim.step();
        let snap = sim.snapshot();
        let world = RenderWorld::from_snapshot(&snap, &paint_for);
        for t in &snap.tanks {
            let v = world.tanks.iter().find(|v| v.player == t.player).unwrap();
            match t.team {
                0 => assert_eq!(v.colour, tank::RUSTBACK),
                1 => assert_eq!(v.colour, tank::DUST_SAINT),
                _ => panic!("no team in Team Brawl"),
            }
        }
        let (mut raid, _) = arena_core::Sim::practice_raid(3, 1, 0.5);
        for _ in 0..(60 * 20) {
            raid.step();
        }
        let snap = raid.snapshot();
        let world = RenderWorld::from_snapshot(&snap, &paint_for);
        for t in snap.tanks.iter().filter(|t| t.team == 1) {
            assert_eq!(world.tanks.iter().find(|v| v.player == t.player).unwrap().colour, tank::RAIDER);
        }
    }

    /// THE LAND-SHIP rolling in on a raid, close (bow quarter) and far.
    #[test]
    #[ignore]
    fn shots_landship() {
        for (name, dist, height, side) in [("landship-close", 30.0f32, 9.0f32, 0.9f32), ("landship-far", 75.0, 26.0, 0.5), ("landship-stern", 26.0, 7.0, 3.6)] {
            render(name, |wr| {
                let seed = 7;
                let (mut sim, _) = arena_core::Sim::practice_raid(seed, 3, 0.7);
                wr.set_map(&arena_core::Map::the_sink(seed));
                let ship = sim.debug_spawn_landship().expect("a land-ship");
                let mut world = RenderWorld::default();
                let mut launched = false;
                for tick in 0..(60 * 50) {
                    sim.step();
                    let ev = sim.drain_events();
                    let t = tick as f32 * arena_core::DT;
                    let hit = ev.iter().any(|e| matches!(e, Event::ShipLaunchedBuggy { .. }));
                    if tick > 60 * 25 || hit || launched {
                        world = RenderWorld::from_snapshot(&sim.snapshot(), &paint_for);
                        world.time = t;
                        wr.clock = t;
                        for e in &ev {
                            wr.event(e, Vec3::ZERO);
                        }
                        wr.fx.update(arena_core::DT, &world);
                    }
                    // Stern shot: half a second after a buggy drops.
                    if hit {
                        launched = true;
                    }
                    if name == "landship-stern" && launched && world.time > 0.0 && wr.ramps.values().any(|s| t - s > 0.6) {
                        break;
                    }
                }
                let (pos, rot) = world.tanks.iter().find(|v| v.player == ship).map(|v| (v.pos, v.rot)).unwrap_or((Vec3::ZERO, Quat::IDENTITY));
                let a = side;
                let dir = rot * Vec3::new(a.cos(), 0.0, a.sin());
                world.camera = Some(Camera::look(pos + dir * dist + Vec3::Y * height, pos + Vec3::Y * 2.0, 50.0));
                eprintln!("{name}: ship at {pos:?}, launched {launched}, {} tanks", world.tanks.len());
                world.dt = 0.0;
                world
            });
        }
    }

    /// A Convoy round under way: the tanker on the road with its escorts,
    /// chevrons on the road ahead.
    #[test]
    #[ignore]
    fn shots_convoy() {
        render("convoy", |wr| {
            let seed = 7;
            let (mut sim, _) = arena_core::Sim::practice_convoy_on(arena_core::MapId::Sink, seed, 4, 0.7);
            wr.set_map(&arena_core::Map::the_sink(seed));
            let mut world = RenderWorld::default();
            for tick in 0..(60 * 45) {
                sim.step();
                let ev = sim.drain_events();
                if tick > 60 * 37 {
                    world = RenderWorld::from_snapshot(&sim.snapshot(), &paint_for);
                    world.time = tick as f32 * arena_core::DT;
                    for e in &ev {
                        wr.event(e, Vec3::ZERO);
                    }
                    wr.fx.update(arena_core::DT, &world);
                }
            }
            let snap = sim.snapshot();
            let c = snap.convoy.clone().expect("convoy");
            let truck = world.tanks.iter().find(|t| t.player == c.truck).map(|t| (t.pos, t.rot)).unwrap_or((Vec3::ZERO, Quat::IDENTITY));
            let path = sim.convoy_path().to_vec();
            let mut acc = 0.0;
            for w in path.windows(2) {
                let d = w[1] - w[0];
                acc += d.length();
                if acc > c.distance + 6.0 && acc < c.distance + 120.0 && (acc as i32) % 8 < 4 {
                    world.route.push((w[0] + Vec3::Y * 0.06, (-d.z).atan2(d.x), 1.0));
                }
            }
            let side = truck.1 * Vec3::new(0.35, 0.0, 1.0);
            world.camera = Some(Camera::look(truck.0 + side * 18.0 + Vec3::Y * 9.0, truck.0 + truck.1 * Vec3::X * 10.0, 55.0));
            eprintln!("convoy: phase {:?}, progress {:.2}, truck {:?}, {} route pts", c.phase, c.progress, truck.0, path.len());
            world.dt = 0.0;
            world
        });
    }

    /// Container stacks near and far (`containers.png`): the corrugation
    /// should read up close and fade to clean paint far off, no moire.
    #[test]
    #[ignore]
    fn shots_containers() {
        render("containers", |wr| {
            wr.begin_scene();
            let r = &mut wr.renderer;
            let mut statics = Vec::new();
            let floor = r.add_mesh(&crate::terrain::flat(400.0));
            statics.push(Instance::new(floor, Mat4::IDENTITY));
            let ids: Vec<MeshId> = props::CONTAINER_COLOURS.iter().map(|c| r.add_mesh(&on_ground(props::container(Vec3::new(6.1, 1.3, 1.22), *c), 1.3))).collect();
            for (k, (x, z)) in [(-9.0f32, -14.0f32), (14.0, -45.0), (-30.0, -90.0), (25.0, -150.0), (-10.0, -230.0)].into_iter().enumerate() {
                for level in 0..3 {
                    for row in 0..2 {
                        let id = ids[(k + level * 3 + row) % ids.len()];
                        let p = Vec3::new(x, level as f32 * 2.6, z - row as f32 * 2.5);
                        statics.push(Instance::new(id, Mat4::from_rotation_translation(Quat::from_rotation_y(0.35), p)));
                    }
                }
            }
            wr.set_statics(statics);
            RenderWorld { camera: Some(Camera::look(Vec3::new(0.0, 4.0, 8.0), Vec3::new(2.0, 2.5, -60.0), 55.0)), ..RenderWorld::default() }
        });
    }

    /// A practice match under a sandstorm.
    #[test]
    #[ignore]
    fn shots_storm() {
        render("storm", |wr| {
            let (mut sim, _) = arena_core::Sim::practice(7, 7, 0.7);
            wr.set_map(&arena_core::Map::the_sink(7));
            wr.renderer.set_storm(0.75);
            let mut world = RenderWorld::default();
            for tick in 0..(60 * 40) {
                sim.step();
                let ev = sim.drain_events();
                if tick > 60 * 30 {
                    world = RenderWorld::from_snapshot(&sim.snapshot(), &paint_for);
                    let t = world.tanks.iter().find(|t| t.alive).map(|t| t.pos).unwrap_or(Vec3::ZERO);
                    world.camera = Some(Camera::look(t + Vec3::new(-11.0, 4.5, 6.0), t + Vec3::Y * 1.5, 55.0));
                    for e in &ev {
                        wr.event(e, t);
                    }
                    wr.fx.update(arena_core::DT, &world);
                }
            }
            world
        });
    }

    /// Battle damage from full health to burning, and a wreck.
    #[test]
    #[ignore]
    fn shots_damage() {
        render("damage", |wr| {
            let mut world = lineup_parts(wr);
            world.tanks.clear();
            for (i, h) in [1.0f32, 0.6, 0.35, 0.15].into_iter().enumerate() {
                let t = parked_tank(i as PlayerId, tank::RUSTBACK, Vec3::new(-16.0 + i as f32 * 10.0, 0.0, 6.0), 0.5, PartKind::HeavyCannon, vec![PartKind::FlankPlates, PartKind::SensorMast]);
                world.health.push((t.player, h));
                world.part_health.push((t.player, PartKind::FlankPlates, h * 0.8));
                world.part_health.push((t.player, PartKind::SensorMast, h));
                world.tanks.push(t);
            }
            world.props.push(PropView { id: 9, kind: PropKind::Wreck, pos: Vec3::new(4.0, tank::HULL_Y - 0.1, -8.0), rot: Quat::from_rotation_y(0.7) * Quat::from_rotation_z(0.08) });
            world.camera = Some(Camera::look(Vec3::new(0.0, 12.0, 26.0), Vec3::new(0.0, 0.5, 0.0), 55.0));
            for _ in 0..90 {
                world.dt = 1.0 / 30.0;
                wr.fx.update(world.dt, &world);
            }
            world.dt = 0.0;
            world
        });
    }

    #[test]
    #[ignore]
    fn shots_low_quality() {
        render("podium-low", |wr| {
            wr.renderer.set_quality(crate::renderer::Quality::Low);
            wr.renderer.look = crate::renderer::Look::preset(crate::renderer::TimeOfDay::Dusk);
            let top = vec![parked_tank(1, tank::RUSTBACK, Vec3::ZERO, 0.0, PartKind::LongGun, vec![PartKind::RocketRack, PartKind::SpikedRam])];
            podium(wr, &top)
        });
    }
}
