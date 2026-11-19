//! Every gameplay and feel number, in one serde struct with defaults.
//!
//! `Tuning::default()` is the shipped feel. `Tuning::from_ron(text)` loads a
//! (partial) RON override: every struct is `#[serde(default)]`, so a file only
//! needs the numbers it changes, e.g. `(tank: (max_speed: 15.0))`.
//!
//! Units: metres, seconds, kilograms, newtons, radians (unless the field name
//! says `_deg`). Rates are per second.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Tuning {
    pub tank: TankTuning,
    pub turret: TurretTuning,
    pub guns: GunTuning,
    pub alt: AltTuning,
    pub damage: DamageTuning,
    pub parts: PartTuning,
    pub upgrades: UpgradeTuning,
    pub pickups: PickupTuning,
    pub mode: ModeTuning,
    pub bots: BotTuning,
    /// Combo parts, rare parts and the air strike.
    pub special: SpecialTuning,
    /// Destructible cover and map hazards.
    pub world: WorldTuning,
    /// Raider buggy chassis (fast, light, flips).
    pub buggy: TankTuning,
    /// Raider technical (pickup with a rocket rack).
    pub technical: TankTuning,
    /// THE WAR RIG boss chassis (about 3x a tank).
    pub war_rig: TankTuning,
    /// Raider Waves (co-op PvE).
    pub raid: RaidTuning,
    /// Garage chassis classes.
    pub chassis: ChassisTuning,
    /// The sandstorm (brawl modes).
    pub storm: StormTuning,
    /// THE LAND-SHIP boss chassis (a ship hull on tracks, about 30 x 9 x 8 m).
    pub land_ship: TankTuning,
    /// Convoy: the fuel tanker truck's chassis.
    pub fuel_tanker: TankTuning,
    /// Convoy mode.
    pub convoy: ConvoyTuning,
}

impl Tuning {
    /// Parse a RON override on top of the defaults.
    pub fn from_ron(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    /// The whole tuning as pretty RON (for a tweak panel or a starter file).
    pub fn to_ron(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).unwrap_or_default()
    }
}

/// Hull, suspension and drive.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TankTuning {
    /// Bare hull mass, kg. Parts add their own mass on top.
    pub base_mass: f32,
    /// Hitpoints of a bare tank. Steel tiers add `steel_health_per_tier`.
    pub base_health: f32,
    /// Collider half extents of the hull box (x forward, y up, z right).
    pub hull_half: [f32; 3],
    /// Centre of mass offset below the hull centre (negative = lower = harder to tip).
    pub com_height: f32,
    /// Road wheels per side.
    pub wheels_per_side: usize,
    /// Distance between road wheels along x.
    pub wheel_spacing: f32,
    /// |z| of the wheel rays (track centre line).
    pub track_half_width: f32,
    /// y of the suspension ray origins, hull-local.
    pub ray_origin_y: f32,
    /// Suspension travel (ray length minus wheel radius).
    pub suspension_travel: f32,
    pub wheel_radius: f32,
    /// Natural frequency of the springs, Hz (stiffness scales with mass so
    /// heavy tanks don't bottom out).
    pub spring_hz: f32,
    pub damping_ratio: f32,
    /// Forward top speed of a bare tank, m/s.
    pub max_speed: f32,
    pub reverse_speed: f32,
    /// Top speed scales by (base_mass / mass) ^ this.
    pub mass_speed_exponent: f32,
    /// Max drive acceleration, m/s^2 (scaled by mass like speed).
    pub accel: f32,
    /// Deceleration when throttle opposes motion.
    pub brake: f32,
    /// Deceleration with no throttle (track friction).
    pub coast_decel: f32,
    /// Velocity error time constant, s: lower = snappier.
    pub accel_tau: f32,
    /// Lateral slip is killed with this time constant (lower = more grip).
    pub lateral_tau: f32,
    /// Lateral friction coefficient (limits sideways force to mu * load).
    pub lateral_mu: f32,
    /// Longitudinal traction coefficient.
    pub traction_mu: f32,
    /// Yaw rate turning in place, rad/s.
    pub yaw_rate_in_place: f32,
    /// Yaw rate at top speed, rad/s.
    pub yaw_rate_at_speed: f32,
    pub yaw_tau: f32,
    /// Max yaw angular acceleration, rad/s^2.
    pub yaw_accel_max: f32,
    /// Forward speed lost while steering hard (fraction).
    pub steer_speed_loss: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub hull_friction: f32,
    /// up.y below this counts as tipped over.
    pub tipped_up_y: f32,
    /// Seconds tipped before the crew rights the tank.
    pub righting_delay: f32,
    // Boost (needs Fuel tier 1).
    pub boost_speed_mult: f32,
    pub boost_accel_mult: f32,
    pub fuel_max: f32,
    pub boost_drain: f32,
    pub fuel_regen: f32,
    /// Fuel needed to start a boost.
    pub boost_min_fuel: f32,
    /// A crippled tank (one track blown) drives at this fraction of speed.
    pub crippled_speed: f32,
    /// How hard a crippled tank pulls toward its dead side.
    pub crippled_pull: f32,
    /// Ramming: closing speed (m/s) above which damage happens.
    pub ram_min_speed: f32,
    /// Ramming damage per m/s of closing speed above the minimum.
    pub ram_damage_per_ms: f32,
    /// Ram damage multiplier of this chassis (buggies ram hard).
    pub ram_mult: f32,
}

impl Default for TankTuning {
    fn default() -> Self {
        Self {
            base_mass: 30_000.0,
            base_health: 400.0,
            hull_half: [3.0, 0.45, 1.8],
            com_height: -0.35,
            wheels_per_side: 5,
            wheel_spacing: 1.1,
            track_half_width: 1.45,
            ray_origin_y: -0.2,
            suspension_travel: 0.5,
            wheel_radius: 0.35,
            spring_hz: 1.5,
            damping_ratio: 0.6,
            max_speed: 13.5,
            reverse_speed: 7.0,
            mass_speed_exponent: 0.6,
            accel: 5.0,
            brake: 6.5,
            coast_decel: 2.5,
            accel_tau: 0.55,
            lateral_tau: 0.12,
            lateral_mu: 0.9,
            traction_mu: 1.0,
            yaw_rate_in_place: 0.9,
            yaw_rate_at_speed: 0.6,
            yaw_tau: 0.25,
            yaw_accel_max: 3.0,
            steer_speed_loss: 0.25,
            linear_damping: 0.05,
            angular_damping: 0.4,
            hull_friction: 0.5,
            tipped_up_y: 0.3,
            righting_delay: 2.0,
            boost_speed_mult: 1.35,
            boost_accel_mult: 1.9,
            fuel_max: 100.0,
            boost_drain: 75.0,
            fuel_regen: 12.0,
            boost_min_fuel: 25.0,
            crippled_speed: 0.3,
            crippled_pull: 0.55,
            ram_min_speed: 4.0,
            ram_damage_per_ms: 5.0,
            ram_mult: 1.0,
        }
    }
}

/// Turret and gun mount geometry and rates.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TurretTuning {
    /// Turret traverse rate, rad/s.
    pub yaw_rate: f32,
    /// Gun elevation rate, rad/s.
    pub pitch_rate: f32,
    /// Gun depression / elevation limits relative to the turret, rad.
    pub pitch_min: f32,
    pub pitch_max: f32,
    /// Turret ring centre, hull-local (the turret frame's origin).
    pub pivot: [f32; 3],
    /// Gun trunnion, turret-local (the gun frame's origin).
    pub gun_pivot: [f32; 3],
}

impl Default for TurretTuning {
    fn default() -> Self {
        Self {
            yaw_rate: 0.95,
            pitch_rate: 0.9,
            pitch_min: -0.14,
            pitch_max: 0.40,
            pivot: [-0.3, 0.45, 0.0],
            gun_pivot: [1.05, 0.42, 0.0],
        }
    }
}

/// One main gun.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GunStats {
    pub shell_speed: f32,
    /// Multiplier on gravity for its shells.
    pub gravity_scale: f32,
    pub damage: f32,
    pub splash_damage: f32,
    pub splash_radius: f32,
    /// Knockback impulse at the centre of the blast, N*s.
    pub knockback: f32,
    pub reload: f32,
    pub shells_per_shot: u32,
    pub spread_deg: f32,
    /// Shell lifetime, s (range = speed * lifetime, roughly).
    pub lifetime: f32,
    /// Recoil impulse on the hull, N*s.
    pub recoil: f32,
    /// Direct damage falls to `falloff_min` of full between these ranges (m).
    pub falloff_start: f32,
    pub falloff_end: f32,
    pub falloff_min: f32,
    /// Muzzle distance from the gun pivot along the barrel.
    pub barrel_length: f32,
    /// Turret traverse multiplier (big guns are slow).
    pub traverse_mult: f32,
    /// Gunsight zoom factor the client may use.
    pub zoom: f32,
}

impl Default for GunStats {
    fn default() -> Self {
        Self {
            shell_speed: 120.0,
            gravity_scale: 1.0,
            damage: 45.0,
            splash_damage: 15.0,
            splash_radius: 3.0,
            knockback: 25_000.0,
            reload: 2.0,
            shells_per_shot: 1,
            spread_deg: 0.3,
            lifetime: 3.0,
            recoil: 10_000.0,
            falloff_start: 1000.0,
            falloff_end: 2000.0,
            falloff_min: 1.0,
            barrel_length: 2.0,
            traverse_mult: 1.0,
            zoom: 2.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GunTuning {
    pub stub: GunStats,
    pub heavy_cannon: GunStats,
    pub big_bore: GunStats,
    pub twin_barrel: GunStats,
    pub long_gun: GunStats,
    /// Tier 3 ("Mk II") of a branch gun: damage multiplier.
    pub mk2_damage: f32,
    /// Tier 3 reload multiplier.
    pub mk2_reload: f32,
    /// Extra spread (cone half-angle, deg) at full bloom.
    pub bloom_max_deg: f32,
    /// Bloom rises toward its target with this time constant (moving, turning).
    pub bloom_rise_tau: f32,
    /// Seconds for bloom to settle from full to ~nothing once still.
    pub bloom_settle: f32,
    /// Bloom added by firing the main gun.
    pub bloom_per_shot: f32,
}

impl Default for GunTuning {
    fn default() -> Self {
        Self {
            stub: GunStats::default(),
            heavy_cannon: GunStats {
                shell_speed: 140.0,
                damage: 75.0,
                splash_damage: 25.0,
                splash_radius: 4.0,
                knockback: 45_000.0,
                reload: 2.6,
                recoil: 22_000.0,
                barrel_length: 3.2,
                traverse_mult: 0.95,
                ..GunStats::default()
            },
            big_bore: GunStats {
                shell_speed: 90.0,
                damage: 110.0,
                splash_damage: 50.0,
                splash_radius: 7.5,
                knockback: 140_000.0,
                reload: 3.8,
                recoil: 60_000.0,
                spread_deg: 0.5,
                lifetime: 4.0,
                barrel_length: 2.8,
                traverse_mult: 0.75,
                ..GunStats::default()
            },
            twin_barrel: GunStats {
                shell_speed: 135.0,
                damage: 36.0,
                splash_damage: 10.0,
                splash_radius: 2.5,
                knockback: 30_000.0,
                reload: 3.0,
                shells_per_shot: 4,
                spread_deg: 2.6,
                lifetime: 0.9,
                recoil: 30_000.0,
                falloff_start: 25.0,
                falloff_end: 90.0,
                falloff_min: 0.35,
                barrel_length: 3.0,
                traverse_mult: 1.0,
                ..GunStats::default()
            },
            long_gun: GunStats {
                shell_speed: 300.0,
                gravity_scale: 0.6,
                damage: 100.0,
                splash_damage: 12.0,
                splash_radius: 2.5,
                knockback: 35_000.0,
                reload: 3.4,
                spread_deg: 0.05,
                lifetime: 2.5,
                recoil: 25_000.0,
                barrel_length: 5.2,
                traverse_mult: 0.85,
                zoom: 4.5,
                ..GunStats::default()
            },
            mk2_damage: 1.25,
            mk2_reload: 0.85,
            bloom_max_deg: 3.0,
            bloom_rise_tau: 0.15,
            bloom_settle: 1.0,
            bloom_per_shot: 0.35,
        }
    }
}

/// Alt-fire weapons: the coaxial MG (everyone), Coilgun and Tesla Spire.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AltTuning {
    pub mg_rate: f32,
    pub mg_damage: f32,
    pub mg_range: f32,
    pub mg_spread_deg: f32,
    /// Max angle between the aim line and the barrel for hitscan to follow the aim.
    pub hitscan_cone_deg: f32,
    pub coil_charge_time: f32,
    /// Below this fraction of a charge, releasing does nothing.
    pub coil_min_charge: f32,
    pub coil_damage: f32,
    pub coil_range: f32,
    pub coil_pierce: u32,
    pub coil_cooldown: f32,
    pub coil_knockback: f32,
    pub tesla_range: f32,
    pub tesla_chain_range: f32,
    pub tesla_chain_count: u32,
    pub tesla_damage: f32,
    pub tesla_chain_falloff: f32,
    pub tesla_cooldown: f32,
    /// Tier 3 multiplier on Volts weapon damage.
    pub mk2_damage: f32,
}

impl Default for AltTuning {
    fn default() -> Self {
        Self {
            mg_rate: 8.0,
            mg_damage: 3.5,
            mg_range: 160.0,
            mg_spread_deg: 1.2,
            hitscan_cone_deg: 12.0,
            coil_charge_time: 1.2,
            coil_min_charge: 0.35,
            coil_damage: 95.0,
            coil_range: 400.0,
            coil_pierce: 3,
            coil_cooldown: 1.0,
            coil_knockback: 50_000.0,
            tesla_range: 40.0,
            tesla_chain_range: 18.0,
            tesla_chain_count: 3,
            tesla_damage: 32.0,
            tesla_chain_falloff: 0.75,
            tesla_cooldown: 2.2,
            mk2_damage: 1.3,
        }
    }
}

/// How damage is routed between hit locations.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DamageTuning {
    /// Fraction of a part hit that also reaches the tank's health.
    pub part_passthrough: f32,
    /// Fraction of a track hit that also reaches the tank's health.
    pub track_passthrough: f32,
    pub track_health: f32,
    /// Seconds a blown track stays blown before the crew has it patched.
    pub track_repair_time: f32,
    /// Splash also hits each part within the radius by this fraction.
    pub splash_part_fraction: f32,
    /// Splash taken by your own shells (fraction).
    pub self_splash: f32,
    /// Seconds after spawning during which a tank takes no damage (ends when it fires).
    pub spawn_protection: f32,
    /// Hull/turret/track damage multipliers by which face was hit.
    pub facing_front: f32,
    pub facing_side: f32,
    pub facing_rear: f32,
}

impl Default for DamageTuning {
    fn default() -> Self {
        Self {
            part_passthrough: 0.3,
            track_passthrough: 0.5,
            track_health: 110.0,
            track_repair_time: 4.0,
            splash_part_fraction: 0.5,
            self_splash: 0.3,
            spawn_protection: 2.0,
            facing_front: 0.75,
            facing_side: 1.0,
            facing_rear: 1.4,
        }
    }
}

/// Health and mass of one part.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct PartStat {
    pub health: f32,
    pub mass: f32,
}

const fn ps(health: f32, mass: f32) -> PartStat {
    PartStat { health, mass }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PartTuning {
    pub heavy_cannon: PartStat,
    pub big_bore: PartStat,
    pub twin_barrel: PartStat,
    pub long_gun: PartStat,
    pub sensor_mast: PartStat,
    pub coilgun: PartStat,
    pub tesla_spire: PartStat,
    pub flank_plates: PartStat,
    pub bulwark: PartStat,
    pub spaced_armour: PartStat,
    pub nitro: PartStat,
    pub afterburner: PartStat,
    pub twin_engines: PartStat,
    pub flamethrower: PartStat,
    pub shield_generator: PartStat,
    pub rocket_rack: PartStat,
    pub spiked_ram: PartStat,
    pub mortar: PartStat,
    pub harpoon: PartStat,
    pub smoke_launcher: PartStat,
    pub mine_layer: PartStat,
    pub repair_rig: PartStat,
    pub rig_cannons: PartStat,
    pub rig_rockets: PartStat,
    pub rig_flamer: PartStat,
    pub rig_plate: PartStat,
    pub rig_engine: PartStat,
    pub rig_mast: PartStat,
    /// Tier 3 parts: health multiplier.
    pub mk2_health: f32,
    /// Tier 3 parts: mass multiplier.
    pub mk2_mass: f32,
    /// Sight height gained from the sensor mast, m.
    pub mast_sight: f32,
    pub twin_engines_speed: f32,
    pub twin_engines_traverse: f32,
    pub afterburner_fuel: f32,
    pub afterburner_ram: f32,
    pub bulwark_ram: f32,
    /// Spaced Armour: splash damage multiplier.
    pub spaced_splash: f32,
    /// Max health added per Steel tier.
    pub steel_health_per_tier: f32,
    // --- THE LAND-SHIP's modules.
    pub ship_gun: PartStat,
    pub ship_mortar: PartStat,
    pub ship_mg: PartStat,
    pub ship_bridge: PartStat,
    pub ship_track: PartStat,
    pub ship_plates: PartStat,
    pub ship_hangar: PartStat,
}

impl Default for PartTuning {
    fn default() -> Self {
        Self {
            heavy_cannon: ps(140.0, 1500.0),
            big_bore: ps(190.0, 3200.0),
            twin_barrel: ps(160.0, 2200.0),
            long_gun: ps(120.0, 1800.0),
            sensor_mast: ps(60.0, 300.0),
            coilgun: ps(110.0, 1200.0),
            tesla_spire: ps(110.0, 900.0),
            flank_plates: ps(260.0, 3000.0),
            bulwark: ps(420.0, 6500.0),
            spaced_armour: ps(300.0, 3500.0),
            nitro: ps(70.0, 400.0),
            afterburner: ps(110.0, 900.0),
            twin_engines: ps(140.0, 1600.0),
            flamethrower: ps(90.0, 700.0),
            shield_generator: ps(90.0, 900.0),
            rocket_rack: ps(110.0, 1100.0),
            spiked_ram: ps(260.0, 2500.0),
            mortar: ps(120.0, 1400.0),
            harpoon: ps(90.0, 600.0),
            smoke_launcher: ps(80.0, 300.0),
            mine_layer: ps(110.0, 900.0),
            repair_rig: ps(100.0, 800.0),
            rig_cannons: ps(650.0, 3000.0),
            rig_rockets: ps(350.0, 1500.0),
            rig_flamer: ps(300.0, 800.0),
            rig_plate: ps(450.0, 2500.0),
            rig_engine: ps(380.0, 2000.0),
            rig_mast: ps(140.0, 300.0),
            mk2_health: 1.4,
            mk2_mass: 1.2,
            mast_sight: 3.2,
            twin_engines_speed: 1.22,
            twin_engines_traverse: 1.5,
            afterburner_fuel: 2.0,
            afterburner_ram: 2.5,
            bulwark_ram: 2.2,
            spaced_splash: 0.6,
            steel_health_per_tier: 40.0,
            ship_gun: ps(400.0, 6000.0),
            ship_mortar: ps(320.0, 4000.0),
            ship_mg: ps(180.0, 1200.0),
            ship_bridge: ps(360.0, 5000.0),
            ship_track: ps(340.0, 8000.0),
            ship_plates: ps(500.0, 12_000.0),
            ship_hangar: ps(390.0, 5000.0),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct UpgradeTuning {
    pub scrap_per_tier: u8,
    pub max_tier: u8,
    /// Seconds to pick a branch at tier 2 before the default sticks.
    pub branch_choice_time: f32,
    /// Scrap pickups dropped per tier lost when a part is shot off.
    pub break_scrap_per_tier: u32,
    /// Scrap pickups dropped per tier lost on death.
    pub death_scrap_per_tier: u32,
    /// Extra scrap always dropped on death (so every kill pays).
    pub death_bonus_scrap: u32,
    /// Tier kept on respawn.
    pub respawn_keep_tier: u8,
    /// Health restored by scrap picked up for a maxed track.
    pub overflow_heal: f32,
}

impl Default for UpgradeTuning {
    fn default() -> Self {
        Self {
            scrap_per_tier: 3,
            max_tier: 3,
            branch_choice_time: 4.0,
            break_scrap_per_tier: 2,
            death_scrap_per_tier: 2,
            death_bonus_scrap: 1,
            respawn_keep_tier: 1,
            overflow_heal: 30.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PickupTuning {
    pub radius: f32,
    /// Seconds before a scrap site spawns again.
    pub site_respawn: f32,
    pub repair_respawn: f32,
    /// Dropped scrap lives this long, s.
    pub dropped_lifetime: f32,
    /// Spilled scrap can't be taken for this long, s.
    pub pickup_delay: f32,
    pub spill_speed: f32,
    /// Repair kit heals this fraction of max health (spread over the repair).
    pub repair_heal: f32,
    pub repair_duration: f32,
    /// Seconds the crew takes to patch a blown track (animated; part of track_repair_time).
    pub track_patch_duration: f32,
}

impl Default for PickupTuning {
    fn default() -> Self {
        Self {
            radius: 3.4,
            site_respawn: 9.0,
            repair_respawn: 28.0,
            dropped_lifetime: 30.0,
            pickup_delay: 0.6,
            spill_speed: 7.0,
            repair_heal: 0.5,
            repair_duration: 2.5,
            track_patch_duration: 1.5,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ModeTuning {
    /// Scrap Brawl length, s.
    pub time_limit: f32,
    pub kill_limit: u32,
    /// Team Brawl: first team to this many kills wins.
    pub team_kill_limit: u32,
    pub respawn_delay: f32,
    /// Seconds wrecks and debris stay around.
    pub wreck_lifetime: f32,
    pub debris_lifetime: f32,
}

impl Default for ModeTuning {
    fn default() -> Self {
        Self {
            time_limit: 7.0 * 60.0,
            kill_limit: 10,
            team_kill_limit: 20,
            respawn_delay: 3.0,
            wreck_lifetime: 12.0,
            debris_lifetime: 10.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct BotTuning {
    pub sight_range: f32,
    pub engage_range: f32,
    pub preferred_range: f32,
    /// Aim error (cone, deg) at difficulty 0.5; scales down with difficulty.
    pub aim_error_deg: f32,
    /// Fire when the gun is within this many degrees of the solution.
    pub fire_tolerance_deg: f32,
    /// Seconds a bot remembers a target after losing sight.
    pub memory: f32,
    pub repath_interval: f32,
    /// Health fraction below which a bot goes for a repair kit.
    pub repair_threshold: f32,
    /// Scrap farther than this is ignored.
    pub scrap_range: f32,
    pub nav_cell: f32,
}

impl Default for BotTuning {
    fn default() -> Self {
        Self {
            sight_range: 240.0,
            engage_range: 165.0,
            preferred_range: 45.0,
            aim_error_deg: 1.6,
            fire_tolerance_deg: 2.5,
            memory: 3.0,
            repath_interval: 1.0,
            repair_threshold: 0.4,
            scrap_range: 160.0,
            nav_cell: 4.0,
        }
    }
}

/// Combo parts (two tracks at tier 2+), rare golden-crate parts, and the air strike.
/// "Fun beats balance": these are rare, telegraphed and loud on purpose.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SpecialTuning {
    // Flamethrower (Powder + Fuel): alt fire, a cone that sets tanks alight.
    pub flame_range: f32,
    pub flame_cone_deg: f32,
    /// Damage per second to everything in the cone.
    pub flame_dps: f32,
    pub flame_fuel_per_s: f32,
    /// Seconds a hit tank keeps burning, and burn damage per second.
    pub burn_time: f32,
    pub burn_dps: f32,
    // Shield generator (Volts + Steel): a bubble that soaks damage first.
    pub shield_hp: f32,
    pub shield_recharge_delay: f32,
    pub shield_recharge_rate: f32,
    // Rocket rack (Powder + Volts): alt fire, lock on by holding the aim on a tank.
    pub rocket_lock_time: f32,
    pub rocket_lock_cone_deg: f32,
    pub rocket_lock_range: f32,
    pub rocket_count: u32,
    pub rocket_interval: f32,
    pub rocket_speed: f32,
    pub rocket_turn_rate: f32,
    pub rocket_damage: f32,
    pub rocket_splash: f32,
    pub rocket_radius: f32,
    pub rocket_cooldown: f32,
    // Spiked ram (Steel + Fuel): ram damage grows with speed.
    pub ram_spike_min_speed: f32,
    /// Extra ram multiplier per m/s of closing speed.
    pub ram_spike_per_ms: f32,
    // Rare parts (golden crates), fired with `Input::special`.
    pub mortar_ammo: u8,
    pub mortar_cooldown: f32,
    pub mortar_speed: f32,
    pub mortar_damage: f32,
    pub mortar_splash: f32,
    pub mortar_radius: f32,
    pub mortar_knockback: f32,
    pub harpoon_range: f32,
    pub harpoon_speed: f32,
    pub harpoon_cooldown: f32,
    pub harpoon_damage: f32,
    /// Seconds a harpoon line stays taut.
    pub harpoon_reel_time: f32,
    /// Reel force as acceleration on the lighter end, m/s^2.
    pub harpoon_reel_accel: f32,
    pub smoke_charges: u8,
    pub smoke_cooldown: f32,
    pub smoke_radius: f32,
    pub smoke_life: f32,
    pub smoke_distance: f32,
    pub mine_charges: u8,
    pub mine_max_active: u8,
    pub mine_cooldown: f32,
    pub mine_arm_time: f32,
    pub mine_trigger_radius: f32,
    pub mine_damage: f32,
    pub mine_radius: f32,
    pub mine_knockback: f32,
    /// A spent charge comes back after this many seconds (smoke, mines, mortar).
    pub charge_regen: f32,
    pub repair_rig_delay: f32,
    pub repair_rig_hps: f32,
    // Golden crates on the map.
    pub rare_respawn_min: f32,
    pub rare_respawn_max: f32,
    pub strike_respawn: f32,
    // Air strike.
    pub strike_delay: f32,
    pub strike_length: f32,
    pub strike_bombs: u32,
    pub strike_duration: f32,
    pub strike_damage: f32,
    pub strike_radius: f32,
    pub strike_knockback: f32,
    pub strike_max_range: f32,
}

impl Default for SpecialTuning {
    fn default() -> Self {
        Self {
            flame_range: 18.0,
            flame_cone_deg: 16.0,
            flame_dps: 55.0,
            flame_fuel_per_s: 22.0,
            burn_time: 3.0,
            burn_dps: 12.0,
            shield_hp: 160.0,
            shield_recharge_delay: 5.0,
            shield_recharge_rate: 35.0,
            rocket_lock_time: 0.9,
            rocket_lock_cone_deg: 7.0,
            rocket_lock_range: 220.0,
            rocket_count: 4,
            rocket_interval: 0.18,
            rocket_speed: 75.0,
            rocket_turn_rate: 2.2,
            rocket_damage: 26.0,
            rocket_splash: 20.0,
            rocket_radius: 4.0,
            rocket_cooldown: 6.0,
            ram_spike_min_speed: 2.5,
            ram_spike_per_ms: 0.35,
            mortar_ammo: 4,
            mortar_cooldown: 3.5,
            mortar_speed: 80.0,
            mortar_damage: 90.0,
            mortar_splash: 75.0,
            mortar_radius: 10.0,
            mortar_knockback: 160_000.0,
            harpoon_range: 75.0,
            harpoon_speed: 140.0,
            harpoon_cooldown: 3.0,
            harpoon_damage: 25.0,
            harpoon_reel_time: 2.5,
            harpoon_reel_accel: 9.0,
            smoke_charges: 3,
            smoke_cooldown: 2.0,
            smoke_radius: 11.0,
            smoke_life: 16.0,
            smoke_distance: 14.0,
            mine_charges: 3,
            mine_max_active: 3,
            mine_cooldown: 1.0,
            mine_arm_time: 1.5,
            mine_trigger_radius: 3.5,
            mine_damage: 120.0,
            mine_radius: 6.0,
            mine_knockback: 220_000.0,
            charge_regen: 18.0,
            repair_rig_delay: 3.0,
            repair_rig_hps: 9.0,
            rare_respawn_min: 80.0,
            rare_respawn_max: 130.0,
            strike_respawn: 140.0,
            strike_delay: 4.0,
            strike_length: 60.0,
            strike_bombs: 9,
            strike_duration: 1.4,
            strike_damage: 170.0,
            strike_radius: 11.0,
            strike_knockback: 380_000.0,
            strike_max_range: 260.0,
        }
    }
}

/// Destructible cover and hazards.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WorldTuning {
    pub barrier_health: f32,
    pub sandbags_health: f32,
    pub car_wreck_health: f32,
    pub crates_health: f32,
    pub pipe_health: f32,
    pub fuel_silo_health: f32,
    pub fuel_tank_health: f32,
    /// The Refinery: big storage tanks and rail tank cars.
    pub storage_tank_health: f32,
    pub tank_car_health: f32,
    pub storage_blast_damage: f32,
    pub storage_blast_radius: f32,
    /// Chain reactions: a blown storage tank or tank car lights fuel hazards
    /// whose skin is within this of its own, after about this fuse (s).
    pub chain_reach: f32,
    pub chain_fuse: f32,
    pub oil_drum_health: f32,
    /// Fuel explosions (silos, tanks, drums): damage, radius, knockback.
    pub fuel_blast_damage: f32,
    pub fuel_blast_radius: f32,
    pub fuel_blast_knockback: f32,
    pub drum_blast_damage: f32,
    pub drum_blast_radius: f32,
    /// A container topples when a blast pushes it with at least this impulse (N*s).
    pub container_topple_impulse: f32,
    pub container_mass: f32,
    pub rubble_lifetime: f32,
    // Launch plate: a hydraulic plate that throws a tank driving over it.
    pub plate_min_speed: f32,
    pub plate_up_speed: f32,
    pub plate_forward_speed: f32,
    pub plate_recharge: f32,
    // Wrecking-ball crane.
    pub crane_ball_mass: f32,
    pub crane_ball_radius: f32,
    /// Seconds per swing push.
    pub crane_period: f32,
    pub crane_push: f32,
    /// Ball hits above this relative speed hurt, damage per m/s.
    pub crane_hit_speed: f32,
    pub crane_damage_per_ms: f32,
}

impl Default for WorldTuning {
    fn default() -> Self {
        Self {
            barrier_health: 260.0,
            sandbags_health: 200.0,
            car_wreck_health: 240.0,
            crates_health: 90.0,
            pipe_health: 420.0,
            fuel_silo_health: 260.0,
            fuel_tank_health: 160.0,
            storage_tank_health: 520.0,
            tank_car_health: 220.0,
            storage_blast_damage: 180.0,
            storage_blast_radius: 22.0,
            chain_reach: 16.0,
            chain_fuse: 0.55,
            oil_drum_health: 30.0,
            fuel_blast_damage: 150.0,
            fuel_blast_radius: 16.0,
            fuel_blast_knockback: 300_000.0,
            drum_blast_damage: 60.0,
            drum_blast_radius: 7.0,
            container_topple_impulse: 120_000.0,
            container_mass: 4_000.0,
            rubble_lifetime: 8.0,
            plate_min_speed: 4.0,
            plate_up_speed: 11.0,
            plate_forward_speed: 6.0,
            plate_recharge: 6.0,
            crane_ball_mass: 9_000.0,
            crane_ball_radius: 1.4,
            crane_period: 7.0,
            crane_push: 45_000.0,
            crane_hit_speed: 3.0,
            crane_damage_per_ms: 9.0,
        }
    }
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            tank: TankTuning::default(),
            turret: TurretTuning::default(),
            guns: GunTuning::default(),
            alt: AltTuning::default(),
            damage: DamageTuning::default(),
            parts: PartTuning::default(),
            upgrades: UpgradeTuning::default(),
            pickups: PickupTuning::default(),
            mode: ModeTuning::default(),
            bots: BotTuning::default(),
            special: SpecialTuning::default(),
            world: WorldTuning::default(),
            buggy: TankTuning::buggy(),
            technical: TankTuning::technical(),
            war_rig: TankTuning::war_rig(),
            raid: RaidTuning::default(),
            chassis: ChassisTuning::default(),
            storm: StormTuning::default(),
            land_ship: TankTuning::land_ship(),
            fuel_tanker: TankTuning::fuel_tanker(),
            convoy: ConvoyTuning::default(),
        }
    }
}

impl TankTuning {
    /// Raider buggy: light, fast, slidey, top-heavy.
    pub fn buggy() -> Self {
        Self {
            base_mass: 5_500.0,
            base_health: 110.0,
            hull_half: [2.2, 0.35, 1.1],
            com_height: -0.05,
            wheels_per_side: 2,
            wheel_spacing: 2.8,
            track_half_width: 1.0,
            ray_origin_y: -0.15,
            suspension_travel: 0.45,
            wheel_radius: 0.4,
            spring_hz: 1.8,
            damping_ratio: 0.45,
            max_speed: 21.0,
            reverse_speed: 10.0,
            accel: 9.0,
            brake: 10.0,
            accel_tau: 0.3,
            lateral_tau: 0.25,
            lateral_mu: 0.65,
            yaw_rate_in_place: 1.6,
            yaw_rate_at_speed: 1.1,
            yaw_tau: 0.15,
            yaw_accel_max: 8.0,
            angular_damping: 0.3,
            tipped_up_y: 0.35,
            ram_min_speed: 3.0,
            ram_mult: 2.5,
            ..Self::default()
        }
    }

    /// Raider technical: a pickup truck with a rocket rack in the bed.
    pub fn technical() -> Self {
        Self {
            base_mass: 8_000.0,
            base_health: 170.0,
            hull_half: [2.6, 0.45, 1.2],
            max_speed: 17.0,
            accel: 7.0,
            ram_mult: 1.3,
            ..Self::buggy()
        }
    }

    /// THE WAR RIG: an armoured truck-tank about three times the size of a tank.
    pub fn war_rig() -> Self {
        Self {
            base_mass: 120_000.0,
            base_health: 1_400.0,
            hull_half: [7.5, 1.4, 3.5],
            com_height: -0.8,
            wheels_per_side: 6,
            wheel_spacing: 2.5,
            track_half_width: 3.0,
            ray_origin_y: -0.9,
            suspension_travel: 0.8,
            wheel_radius: 0.7,
            spring_hz: 1.2,
            max_speed: 7.5,
            reverse_speed: 4.0,
            mass_speed_exponent: 0.0,
            accel: 2.5,
            yaw_rate_in_place: 0.35,
            yaw_rate_at_speed: 0.3,
            yaw_accel_max: 1.5,
            ram_min_speed: 2.0,
            ram_mult: 4.0,
            righting_delay: 1.0,
            ..Self::default()
        }
    }
}

impl TankTuning {
    /// THE LAND-SHIP: a beached ship hull on four giant track units, about 30 x 9 x 8 m.
    pub fn land_ship() -> Self {
        Self {
            base_mass: 420_000.0,
            base_health: 1_650.0,
            hull_half: [15.0, 2.0, 4.5],
            com_height: -1.6,
            wheels_per_side: 8,
            wheel_spacing: 3.6,
            track_half_width: 4.0,
            ray_origin_y: -1.6,
            suspension_travel: 1.0,
            wheel_radius: 0.9,
            spring_hz: 1.0,
            damping_ratio: 0.7,
            max_speed: 3.8,
            reverse_speed: 1.8,
            mass_speed_exponent: 0.0,
            accel: 1.0,
            brake: 2.0,
            coast_decel: 1.5,
            yaw_rate_in_place: 0.14,
            yaw_rate_at_speed: 0.12,
            yaw_tau: 0.6,
            yaw_accel_max: 0.4,
            angular_damping: 0.8,
            ram_min_speed: 1.5,
            ram_mult: 4.0,
            righting_delay: 1.0,
            tipped_up_y: 0.5,
            ..Self::default()
        }
    }

    /// Convoy: a six-wheel fuel tanker truck. Slow, tough, no guns.
    pub fn fuel_tanker() -> Self {
        Self {
            base_mass: 24_000.0,
            base_health: 2_400.0,
            hull_half: [4.6, 0.9, 1.4],
            com_height: -0.5,
            wheels_per_side: 3,
            wheel_spacing: 3.2,
            track_half_width: 1.25,
            ray_origin_y: -0.5,
            suspension_travel: 0.5,
            wheel_radius: 0.55,
            spring_hz: 1.4,
            max_speed: 5.0,
            reverse_speed: 3.0,
            mass_speed_exponent: 0.0,
            accel: 2.0,
            yaw_rate_in_place: 0.55,
            yaw_rate_at_speed: 0.45,
            ram_mult: 1.5,
            righting_delay: 1.5,
            ..Self::default()
        }
    }
}

/// Convoy: one team escorts a fuel tanker along the map's route, the other stops it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ConvoyTuning {
    /// Seconds before the truck rolls each round (both sides set up).
    pub setup_time: f32,
    /// A round ends as a stop (the attackers hold) if the truck hasn't arrived by then, s.
    pub round_time: f32,
    /// Seconds between the rounds.
    pub intermission: f32,
    /// The truck stops when an attacker is this close and no escort is within `escort_radius`.
    pub stop_radius: f32,
    pub escort_radius: f32,
    /// Escorts within this distance patch the truck, `repair_hps` each (up to `repair_max_crews`).
    pub repair_radius: f32,
    pub repair_hps: f32,
    pub repair_max_crews: u32,
    /// Repairs pause for this long after the truck is hit, s.
    pub repair_delay: f32,
    /// Spawns: escorts this far behind the truck, attackers this far ahead (m, along the route).
    pub escort_spawn_behind: f32,
    pub attacker_spawn_ahead_min: f32,
    pub attacker_spawn_ahead_max: f32,
    /// Lateral spread of the spawns off the route, m.
    pub escort_spawn_side: f32,
    pub attacker_spawn_side: f32,
    /// The truck counts as delivered this close to the end of the route, m.
    pub arrive_distance: f32,
    /// The tanker's armour: damage taken is multiplied by this, and MG bullets
    /// by `truck_mg_mult` on top.
    pub truck_armour: f32,
    pub truck_mg_mult: f32,
}

impl Default for ConvoyTuning {
    fn default() -> Self {
        Self {
            setup_time: 10.0,
            round_time: 300.0,
            intermission: 8.0,
            stop_radius: 32.0,
            escort_radius: 26.0,
            repair_radius: 14.0,
            repair_hps: 20.0,
            repair_max_crews: 3,
            repair_delay: 1.5,
            escort_spawn_behind: 30.0,
            attacker_spawn_ahead_min: 120.0,
            attacker_spawn_ahead_max: 180.0,
            escort_spawn_side: 14.0,
            attacker_spawn_side: 40.0,
            arrive_distance: 8.0,
            truck_armour: 0.175,
            truck_mg_mult: 0.35,

        }
    }
}

/// Raider Waves: hold the outpost against escalating raider waves.

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RaidTuning {
    pub outpost_health: f32,
    pub outpost_radius: f32,
    /// Seconds between waves, and the share of outpost max health repaired then.
    pub breather: f32,
    pub breather_repair: f32,
    /// Seconds before the first wave.
    pub first_wave_delay: f32,
    /// Seconds a dead defender waits (the breather also brings everyone back).
    pub defender_respawn: f32,
    pub spawn_interval: f32,
    pub max_alive: usize,
    /// Wave n: buggies = base + per_wave * n; tanks = per_wave * (n - 1); technicals = per_wave * (n - 2).
    pub buggies_base: f32,
    pub buggies_per_wave: f32,
    pub tanks_per_wave: f32,
    pub technicals_per_wave: f32,
    pub boss_every: u32,
    /// Raider health multiplier grows by this per wave.
    pub health_per_wave: f32,
    /// Raider tanks get this many tier points per wave (capped).
    pub tank_tiers_per_wave: f32,
    pub tank_tiers_max: u32,
    /// Outpost drain per second for a raider inside the zone.
    pub sabotage_dps: f32,
    pub boss_sabotage_dps: f32,
    /// A buggy reaching the outpost blows itself up for this much.
    pub buggy_kamikaze: f32,
    /// Raider shells landing on the outpost: splash damage times this.
    pub outpost_splash_mult: f32,
    /// Extra scrap a raider spills on death.
    pub drop_buggy: u32,
    pub drop_technical: u32,
    pub drop_tank: u32,
    pub drop_boss: u32,
    /// Boss: hull hits while any plate stands are multiplied by this.
    pub boss_core_shielded: f32,
    /// Boss module and core health grow by this much per boss cycle (wave 10, 15, ...).
    pub boss_health_per_cycle: f32,
    /// The War Rig's twin cannon.
    pub rig_cannon: GunStats,
    pub rig_flame_range: f32,
    pub rig_flame_dps: f32,
    /// Aim error once the sensor mast is shot off (deg).
    pub rig_blind_error_deg: f32,
    /// Raider bots aim worse than defenders by this factor.
    pub raider_aim_error_mult: f32,
    // --- THE LAND-SHIP (boss waves alternate: Rig, Land-Ship, Rig, ...).
    /// Alternate the boss kind on boss waves (false: always the War Rig).
    pub boss_alternate: bool,
    /// Land-Ship waves bring this share of a normal wave's escorts (its hangar adds buggies).
    pub ship_escort_scale: f32,
    /// Extra scrap the Land-Ship spills on death.
    pub drop_ship: u32,
    /// The deck guns (each turret fires on its own).
    pub deck_gun: GunStats,
    /// Deck gun aim error, deg (and once the bridge is gone).
    pub ship_aim_error_deg: f32,
    pub ship_blind_error_deg: f32,
    /// Deck gun turret traverse, rad/s.
    pub ship_gun_traverse: f32,
    /// Mortar battery: a salvo of `ship_mortar_shells` every `ship_mortar_interval` s,
    /// at the biggest cluster of defenders between the ranges.
    pub ship_mortar_interval: f32,
    pub ship_mortar_shells: u32,
    pub ship_mortar_min_range: f32,
    pub ship_mortar_max_range: f32,
    /// Cluster radius for picking the mortar target, m.
    pub ship_mortar_cluster: f32,
    /// Scatter of each mortar shell around the aim point, m (doubled when blind).
    pub ship_mortar_scatter: f32,
    /// Broadside MG nests: shots per second, damage per shot, range.
    pub ship_mg_rate: f32,
    pub ship_mg_damage: f32,
    pub ship_mg_range: f32,
    /// The hangar drops a raider buggy this often while it stands, s.
    pub ship_hangar_interval: f32,
    /// The ship stops this far from the outpost and bombards it.
    pub ship_standoff: f32,
    /// Top-speed multiplier with one track unit lost on each side / a whole side gone.
    pub ship_one_track_speed: f32,
    pub ship_side_lost_speed: f32,
    /// Steer bias toward a side with both track units gone (slewing).
    pub ship_slew: f32,
}

impl Default for RaidTuning {
    fn default() -> Self {
        Self {
            outpost_health: 3600.0,
            outpost_radius: 16.0,
            breather: 15.0,
            breather_repair: 0.15,
            first_wave_delay: 10.0,
            defender_respawn: 6.0,
            spawn_interval: 0.8,
            max_alive: 16,
            buggies_base: 2.0,
            buggies_per_wave: 0.9,
            tanks_per_wave: 0.45,
            technicals_per_wave: 0.34,
            boss_every: 5,
            health_per_wave: 0.05,
            tank_tiers_per_wave: 0.7,
            tank_tiers_max: 9,
            sabotage_dps: 10.0,
            boss_sabotage_dps: 60.0,
            buggy_kamikaze: 90.0,
            outpost_splash_mult: 0.3,
            drop_buggy: 1,
            drop_technical: 2,
            drop_tank: 3,
            drop_boss: 14,
            boss_core_shielded: 0.12,
            boss_health_per_cycle: 0.35,
            rig_cannon: GunStats {
                shell_speed: 110.0,
                damage: 70.0,
                splash_damage: 40.0,
                splash_radius: 6.0,
                knockback: 90_000.0,
                reload: 2.2,
                shells_per_shot: 2,
                spread_deg: 0.8,
                lifetime: 4.0,
                recoil: 0.0,
                barrel_length: 6.0,
                traverse_mult: 0.6,
                ..GunStats::default()
            },
            rig_flame_range: 22.0,
            rig_flame_dps: 70.0,
            rig_blind_error_deg: 9.0,
            raider_aim_error_mult: 1.6,
            boss_alternate: true,
            ship_escort_scale: 0.4,
            drop_ship: 20,
            deck_gun: GunStats {
                shell_speed: 120.0,
                damage: 60.0,
                splash_damage: 38.0,
                splash_radius: 6.5,
                knockback: 110_000.0,
                reload: 3.2,
                shells_per_shot: 1,
                spread_deg: 0.5,
                lifetime: 4.0,
                recoil: 0.0,
                barrel_length: 5.4,
                traverse_mult: 1.0,
                ..GunStats::default()
            },
            ship_aim_error_deg: 1.4,
            ship_blind_error_deg: 7.0,
            ship_gun_traverse: 0.7,
            ship_mortar_interval: 8.0,
            ship_mortar_shells: 3,
            ship_mortar_min_range: 35.0,
            ship_mortar_max_range: 240.0,
            ship_mortar_cluster: 18.0,
            ship_mortar_scatter: 7.0,
            ship_mg_rate: 6.0,
            ship_mg_damage: 3.0,
            ship_mg_range: 75.0,
            ship_hangar_interval: 30.0,
            ship_standoff: 62.0,
            ship_one_track_speed: 0.8,
            ship_side_lost_speed: 0.4,
            ship_slew: 0.45,
        }
    }
}

/// One Garage chassis class, applied on top of the tank chassis.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct ClassStat {
    /// Hitpoints of the bare chassis (replaces `tank.base_health`).
    pub health: f32,
    /// Multiplier on the bare hull mass (lighter = faster, flips sooner).
    pub mass_mult: f32,
    /// Extra top-speed multiplier on top of what the mass gives.
    pub speed_mult: f32,
    pub traverse_mult: f32,
    /// Scrap of a kind per tier.
    pub scrap_per_tier: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ChassisTuning {
    pub scout: ClassStat,
    pub brawler: ClassStat,
    pub hauler: ClassStat,
}

impl Default for ChassisTuning {
    fn default() -> Self {
        Self {
            scout: ClassStat { health: 330.0, mass_mult: 0.72, speed_mult: 1.12, traverse_mult: 1.3, scrap_per_tier: 3 },
            brawler: ClassStat { health: 400.0, mass_mult: 1.0, speed_mult: 1.0, traverse_mult: 1.15, scrap_per_tier: 3 },
            hauler: ClassStat { health: 430.0, mass_mult: 1.4, speed_mult: 1.0, traverse_mult: 0.75, scrap_per_tier: 2 },
        }
    }
}

/// The sandstorm: about once per brawl match.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct StormTuning {
    /// The storm arrives between these match times, s (0 disables storms).
    pub earliest: f32,
    pub latest: f32,
    pub warning: f32,
    pub duration_min: f32,
    pub duration_max: f32,
    /// Seconds to build up and to die down.
    pub ramp: f32,
    /// Bot sight range at full intensity (fraction of normal).
    pub sight_mult: f32,
    /// Extra shot spread at full intensity, deg.
    pub spread_deg: f32,
    /// Scrap sites respawn this many times faster at full intensity.
    pub scrap_respawn_mult: f32,
}

impl Default for StormTuning {
    fn default() -> Self {
        Self {
            earliest: 120.0,
            latest: 300.0,
            warning: 10.0,
            duration_min: 45.0,
            duration_max: 60.0,
            ramp: 5.0,
            sight_mult: 0.25,
            spread_deg: 1.2,
            scrap_respawn_mult: 3.0,
        }
    }
}
