//! Client-side prediction of your own tank.
//!
//! A local `Sim` from the room's seed holds just your tank. Each tick your
//! input is applied locally at once. On each server snapshot the tank is
//! snapped to the server's state for the last input the server applied
//! (`ack`), then the inputs it hasn't applied yet are re-simulated. The
//! visible pose eases out the difference instead of popping.

use std::collections::VecDeque;

use arena_core::snapshot::{q_rot, Snapshot, TankSnap};
use arena_core::{Input, Map, MapId, PlayerId, Quat, Sim, Tuning, Vec3};

/// Unacknowledged inputs kept at most (2 s): more means the link is gone.
const MAX_PENDING: usize = 120;
/// How fast a correction is eased out, 1/s.
const SMOOTH_RATE: f32 = 12.0;
/// Corrections bigger than this snap instead of easing (respawn, teleport), m.
const SNAP_DIST: f32 = 4.0;

pub struct Predictor {
    pub sim: Sim,
    pub me: PlayerId,
    pending: VecDeque<(u32, Input)>,
    /// Pose before and after the newest local tick, to draw between them.
    prev: (Vec3, Quat),
    cur: (Vec3, Quat),
    /// Visual correction still being eased out.
    off_pos: Vec3,
    off_rot: Quat,
}

/// The room's world, holding only player `me` (ids must match the server's).
pub fn local_sim(seed: u64, me: PlayerId, map: MapId) -> Sim {
    let mut tuning = Tuning::default();
    // The server decides when a match ends; the local copy never does.
    tuning.mode.time_limit = 1.0e9;
    tuning.mode.kill_limit = 0;
    let mut sim = Sim::new(seed, Map::by_id(map, seed), tuning);
    let mut filler = Vec::new();
    loop {
        let id = sim.add_player("You");
        if id == me {
            break;
        }
        filler.push(id);
    }
    for id in filler {
        sim.remove_player(id);
    }
    sim.drain_events();
    sim
}

impl Predictor {
    pub fn new(seed: u64, me: PlayerId, map: MapId) -> Self {
        let sim = local_sim(seed, me, map);
        let pose = sim.tank_pose(me).unwrap_or((Vec3::ZERO, Quat::IDENTITY));
        Predictor {
            sim,
            me,
            pending: VecDeque::new(),
            prev: pose,
            cur: pose,
            off_pos: Vec3::ZERO,
            off_rot: Quat::IDENTITY,
        }
    }

    fn pose(&self) -> (Vec3, Quat) {
        self.sim.tank_pose(self.me).unwrap_or(self.cur)
    }

    fn step(&mut self) {
        self.sim.step();
        // Local shots and pickups are not real: the server's are drawn.
        self.sim.projectiles.clear();
        self.sim.drain_events();
    }

    /// One local tick with input `seq`.
    pub fn tick(&mut self, seq: u32, input: Input) {
        self.pending.push_back((seq, input));
        while self.pending.len() > MAX_PENDING {
            self.pending.pop_front();
        }
        self.sim.set_input(self.me, input);
        self.step();
        self.prev = self.cur;
        self.cur = self.pose();
    }

    /// Server state for input `ack`: rewind to it and replay the rest.
    pub fn reconcile(&mut self, snap: &Snapshot, ack: u32) {
        while self.pending.front().is_some_and(|(s, _)| *s <= ack) {
            self.pending.pop_front();
        }
        let Some(server) = snap.tank(self.me) else { return };
        let (old_pos, old_rot) = self.cur;
        self.sim.phase = snap.mode.phase;
        self.sim.apply_tank_snap(server);
        let pending: Vec<Input> = self.pending.iter().map(|(_, i)| *i).collect();
        for input in pending {
            self.sim.set_input(self.me, input);
            self.step();
        }
        let (pos, rot) = self.pose();
        let jump = old_pos - pos;
        if jump.length() > SNAP_DIST || !server.alive {
            self.off_pos = Vec3::ZERO;
            self.off_rot = Quat::IDENTITY;
            self.prev = (pos, rot);
        } else {
            self.off_pos += jump;
            self.off_rot = (old_rot * rot.inverse() * self.off_rot).normalize();
            // Keep the in-between draw continuous too.
            self.prev = (self.prev.0 - jump, (rot * old_rot.inverse() * self.prev.1).normalize());
        }
        self.cur = (pos, rot);
    }

    /// Ease the visual correction out over `dt` seconds.
    pub fn decay(&mut self, dt: f32) {
        let k = (-SMOOTH_RATE * dt).exp();
        self.off_pos *= k;
        self.off_rot = Quat::IDENTITY.slerp(self.off_rot, k);
    }

    /// Your tank to draw: the server's newest data (health, parts, ...) with
    /// the predicted pose, `alpha` (0..1) of the way through the current tick.
    pub fn view(&self, server: &TankSnap, alpha: f32) -> TankSnap {
        let mut t = server.clone();
        // Just our tank: a full snapshot every frame cost a copy of the
        // whole world.
        if let Some(l) = self.sim.tank_snap(self.me) {
            t.linvel = l.linvel;
            t.angvel = l.angvel;
            t.turret_yaw = l.turret_yaw;
            t.gun_pitch = l.gun_pitch;
            t.wheels = l.wheels.clone();
            t.track_speed = l.track_speed;
        }
        let pos = self.prev.0.lerp(self.cur.0, alpha) + self.off_pos;
        let rot = self.off_rot * self.prev.1.slerp(self.cur.1, alpha);
        t.pos = pos.to_array();
        t.rot = q_rot(rot);
        t
    }

    /// The raw predicted pose after the newest local tick (no smoothing).
    pub fn predicted_pose(&self) -> (Vec3, Quat) {
        self.cur
    }

    pub fn unacked(&self) -> usize {
        self.pending.len()
    }
}
