//! The compositions: every note of the title theme, the battle loop and the
//! stingers. All motifs are original. The language is desert scrapyard rock
//! with a western twang: E minor pentatonic with the blue fifth for swagger,
//! the Phrygian half-step (E-F) for menace, a baritone guitar riff doubled
//! by a dirty bass, a harmonica on top, a stomping, dusty kit with clanks of
//! scrap metal in it.
//!
//! Notes are semitones from E: E2 for guitar, E1 for bass and drone, E4 for
//! the harmonica. Time is in 16th-note steps; a bar is 16 steps.

/// An instrument of the band.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inst {
    /// Baritone guitar, let ring.
    Guitar,
    /// Baritone guitar, palm-muted.
    Mute,
    Bass,
    /// Harmonica-ish lead (monophonic).
    Lead,
    /// A low sustained drone.
    Drone,
    Kick,
    Snare,
    TomHi,
    TomMid,
    TomLo,
    Stomp,
    Clap,
    Clank,
    Shaker,
}

/// One note or hit.
#[derive(Clone, Copy, Debug)]
pub struct Ev {
    pub step: u32,
    pub inst: Inst,
    pub note: i8,
    /// Length in steps (how long it is held before it is damped).
    pub len: u16,
    pub vel: f32,
    /// Which adaptive layer it belongs to (battle); 0 elsewhere.
    pub layer: u8,
}

/// A piece: tempo, length, and its notes sorted by step.
#[derive(Clone, Debug)]
pub struct Song {
    pub bpm: f32,
    pub steps: u32,
    pub looping: bool,
    /// Delay of every off-beat 16th, as a share of a step: the shuffle.
    pub swing: f32,
    pub events: Vec<Ev>,
}

/// Battle layers.
pub const L_DRONE: u8 = 0;
pub const L_SPARSE: u8 = 1;
pub const L_CHUG: u8 = 2;
pub const L_BASS: u8 = 3;
pub const L_DRUMS_LIGHT: u8 = 4;
pub const L_DRUMS_FULL: u8 = 5;
pub const L_LEAD: u8 = 6;
pub const LAYERS: usize = 7;

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The battle mix at `intensity` (0 calm .. 1 full band): per-layer gains.
#[must_use]
pub fn battle_layers(intensity: f32) -> [f32; LAYERS] {
    let i = intensity.clamp(0.0, 1.0);
    [
        1.0 - 0.6 * smoothstep(0.5, 1.0, i),
        1.0 - smoothstep(0.35, 0.6, i),
        smoothstep(0.35, 0.6, i),
        smoothstep(0.3, 0.55, i),
        smoothstep(0.12, 0.4, i),
        smoothstep(0.45, 0.7, i),
        smoothstep(0.7, 0.9, i),
    ]
}

/// Builds a song bar by bar.
struct Comp {
    events: Vec<Ev>,
    layer: u8,
}

impl Comp {
    fn new() -> Self {
        Self { events: Vec::new(), layer: 0 }
    }

    fn layer(&mut self, l: u8) -> &mut Self {
        self.layer = l;
        self
    }

    /// Notes as `(step from the start of `bar`, note, length, velocity)`.
    fn line(&mut self, bar: u32, inst: Inst, notes: &[(u32, i8, u16, f32)]) -> &mut Self {
        for &(step, note, len, vel) in notes {
            self.events.push(Ev { step: bar * 16 + step, inst, note, len, vel, layer: self.layer });
        }
        self
    }

    /// The same line shifted by `by` semitones.
    fn line_t(&mut self, bar: u32, inst: Inst, by: i8, notes: &[(u32, i8, u16, f32)]) -> &mut Self {
        for &(step, note, len, vel) in notes {
            self.events.push(Ev { step: bar * 16 + step, inst, note: note + by, len, vel, layer: self.layer });
        }
        self
    }

    /// A hit pattern, one character per step (`X` 1.0, `x` 0.75, `o` 0.5,
    /// `-` 0.3, anything else a rest), repeated over `bars` bars from `bar`.
    fn hits(&mut self, bar: u32, bars: u32, inst: Inst, pattern: &str) -> &mut Self {
        let p: Vec<char> = pattern.chars().filter(|c| !c.is_whitespace()).collect();
        let len = p.len().max(1) as u32;
        let total = bars * 16;
        for s in 0..total {
            let vel = match p[(s % len) as usize] {
                'X' => 1.0,
                'x' => 0.75,
                'o' => 0.5,
                '-' => 0.3,
                _ => continue,
            };
            self.events.push(Ev { step: bar * 16 + s, inst, note: 0, len: 1, vel, layer: self.layer });
        }
        self
    }

    /// A chord: every string on the same step (the player strums them a few
    /// milliseconds apart), each a little softer than the one before.
    fn chord(&mut self, bar: u32, step: u32, inst: Inst, notes: &[i8], len: u16, vel: f32) -> &mut Self {
        for (k, n) in notes.iter().enumerate() {
            let v = vel * (1.0 - 0.08 * k as f32);
            self.events.push(Ev { step: bar * 16 + step, inst, note: *n, len, vel: v, layer: self.layer });
        }
        self
    }

    fn song(self, bpm: f32, bars: u32, looping: bool, swing: f32) -> Song {
        self.song_steps(bpm, bars * 16, looping, swing)
    }

    fn song_steps(mut self, bpm: f32, steps: u32, looping: bool, swing: f32) -> Song {
        self.events.retain(|e| e.step < steps);
        self.events.sort_by_key(|e| e.step);
        Song { bpm, steps, looping, swing, events: self.events }
    }
}

// ── Title theme ─────────────────────────────────────────────────────────

/// The riff: two bars of swagger, answered, then a turnaround that climbs
/// through the blue note back home.
const RIFF_A: [(u32, i8, u16, f32); 8] =
    [(0, 0, 3, 1.0), (3, 0, 1, 0.6), (4, 3, 2, 0.9), (6, 5, 1, 0.8), (7, 6, 1, 0.8), (8, 7, 4, 1.0), (12, 5, 2, 0.8), (14, 3, 2, 0.8)];
const RIFF_B: [(u32, i8, u16, f32); 7] =
    [(0, 0, 3, 1.0), (3, 0, 1, 0.6), (4, 3, 2, 0.9), (6, 0, 2, 0.8), (8, -2, 2, 0.9), (10, -5, 4, 1.0), (14, -2, 2, 0.7)];
const TURN: [(u32, i8, u16, f32); 8] =
    [(0, 0, 2, 1.0), (2, 3, 2, 0.8), (4, 5, 2, 0.8), (6, 6, 2, 0.9), (8, 7, 2, 1.0), (10, 10, 2, 0.9), (12, 7, 2, 0.8), (14, 6, 2, 0.8)];

/// The four-bar riff at `bar`.
fn riff(c: &mut Comp, bar: u32, inst: Inst, by: i8) {
    c.line_t(bar, inst, by, &RIFF_A);
    c.line_t(bar + 1, inst, by, &RIFF_B);
    c.line_t(bar + 2, inst, by, &RIFF_A);
    c.line_t(bar + 3, inst, by, &TURN);
}

/// The harmonica's tune over the riff, four bars (steps across bars).
const TUNE_1: [(u32, i8, u16, f32); 12] = [
    (0, 7, 6, 0.9),
    (6, 10, 2, 0.8),
    (8, 12, 8, 1.0),
    (16, 10, 4, 0.8),
    (20, 7, 4, 0.8),
    (24, 5, 6, 0.9),
    (30, 3, 2, 0.7),
    (32, 5, 4, 0.8),
    (36, 7, 8, 1.0),
    (44, 6, 2, 0.8),
    (46, 5, 2, 0.8),
    (48, 3, 14, 0.9),
];
const TUNE_2: [(u32, i8, u16, f32); 12] = [
    (0, 7, 6, 0.9),
    (6, 10, 2, 0.8),
    (8, 12, 8, 1.0),
    (16, 15, 4, 0.9),
    (20, 12, 4, 0.8),
    (24, 10, 6, 0.9),
    (30, 7, 2, 0.7),
    (32, 10, 4, 0.8),
    (36, 12, 6, 1.0),
    (42, 10, 2, 0.8),
    (44, 7, 4, 0.8),
    (48, 12, 14, 1.0),
];

fn title_drums(c: &mut Comp, bar: u32, bars: u32, full: bool) {
    c.hits(bar, bars, Inst::Kick, "X.....x.X.....x.");
    c.hits(bar, bars, Inst::Stomp, "....X.......X...");
    c.hits(bar, bars, Inst::Shaker, "x.o.x.o.x.o.x.oo");
    if full {
        c.hits(bar, bars, Inst::Snare, "....X.......X..-");
        c.hits(bar, bars, Inst::Clap, "............X...");
    }
}

fn fill(c: &mut Comp, bar: u32) {
    c.hits(bar, 1, Inst::TomHi, "........X.x.....");
    c.hits(bar, 1, Inst::TomMid, "............X.x.");
    c.hits(bar, 1, Inst::TomLo, "..............XX");
}

/// 32 bars at 96 bpm: 80 s, looping. A: the riff and the stomp. B: the
/// harmonica's tune on top. C: the bridge, twangy arpeggios over C-D-Em-B.
/// A': everything, the tune's answer, and back round.
#[must_use]
pub fn title() -> Song {
    let mut c = Comp::new();
    // A (bars 0-7).
    riff(&mut c, 0, Inst::Mute, 0);
    riff(&mut c, 4, Inst::Guitar, 0);
    riff(&mut c, 4, Inst::Bass, 0);
    c.chord(0, 0, Inst::Drone, &[0], 64, 0.6);
    title_drums(&mut c, 0, 2, false);
    c.hits(1, 1, Inst::Clank, "..............x.");
    title_drums(&mut c, 2, 6, true);
    c.hits(3, 1, Inst::Clank, "............X...");
    fill(&mut c, 7);
    // B (8-15).
    riff(&mut c, 8, Inst::Guitar, 0);
    riff(&mut c, 8, Inst::Bass, 0);
    riff(&mut c, 12, Inst::Guitar, 0);
    riff(&mut c, 12, Inst::Bass, 0);
    c.line(8, Inst::Lead, &TUNE_1);
    c.line(12, Inst::Lead, &TUNE_2);
    title_drums(&mut c, 8, 8, true);
    c.hits(9, 1, Inst::Clank, "..............X.");
    c.hits(13, 1, Inst::Clank, "..............X.");
    fill(&mut c, 15);
    // C, the bridge (16-23): C C D D Em Em B B, arpeggiated and let ring.
    let chords: [(i8, [i8; 4]); 4] = [(-4, [-4, 3, 8, 12]), (-2, [-2, 5, 10, 14]), (0, [0, 7, 12, 15]), (-5, [-5, 2, 7, 11])];
    for (k, (root, tones)) in chords.iter().enumerate() {
        for half in 0..2u32 {
            let bar = 16 + k as u32 * 2 + half;
            c.line(
                bar,
                Inst::Guitar,
                &[(0, tones[0], 6, 1.0), (3, tones[1], 5, 0.7), (6, tones[2], 4, 0.75), (9, tones[3], 4, 0.8), (12, tones[2], 4, 0.6)],
            );
            c.line(bar, Inst::Bass, &[(0, *root, 6, 1.0), (6, *root, 2, 0.6), (8, *root + 7, 6, 0.8), (14, *root, 2, 0.6)]);
        }
        c.chord(16 + k as u32 * 2, 0, Inst::Drone, &[*root], 32, 0.45);
    }
    c.hits(16, 8, Inst::Kick, "X.......X..x....");
    c.hits(16, 8, Inst::Stomp, "........X.......");
    c.hits(16, 8, Inst::TomLo, "....x.......x...");
    c.hits(16, 8, Inst::Shaker, "x.o.x.o.x.o.x.o.");
    c.hits(17, 1, Inst::Clank, "............X...");
    c.hits(21, 1, Inst::Clank, "............X...");
    c.line(18, Inst::Lead, &[(0, 12, 12, 0.8), (12, 10, 4, 0.7), (16, 7, 16, 0.8)]);
    c.line(22, Inst::Lead, &[(0, 11, 8, 0.9), (8, 9, 4, 0.7), (12, 7, 20, 0.8)]);
    fill(&mut c, 23);
    // A' (24-31).
    riff(&mut c, 24, Inst::Guitar, 0);
    riff(&mut c, 24, Inst::Bass, 0);
    riff(&mut c, 28, Inst::Guitar, 0);
    riff(&mut c, 28, Inst::Bass, 0);
    riff(&mut c, 28, Inst::Lead, 12);
    c.line(24, Inst::Lead, &TUNE_2);
    title_drums(&mut c, 24, 8, true);
    c.hits(24, 8, Inst::Clank, "..............o.");
    fill(&mut c, 31);
    c.song(96.0, 32, true, 0.12)
}

// ── Battle ──────────────────────────────────────────────────────────────

/// The chugging battle riff, one bar, Phrygian: E E E F E . G F E . D.
const CHUG: [(u32, i8, u16, f32); 10] = [
    (0, 0, 1, 1.0),
    (2, 0, 1, 0.7),
    (3, 0, 1, 0.6),
    (4, 1, 2, 0.9),
    (6, 0, 1, 0.7),
    (8, 0, 1, 1.0),
    (10, 3, 1, 0.8),
    (11, 1, 1, 0.7),
    (12, 0, 2, 0.9),
    (14, -2, 2, 0.8),
];

/// 16 bars at 112 bpm (34 s), looping, in seven layers that the intensity
/// mixes (see [`battle_layers`]).
#[must_use]
pub fn battle() -> Song {
    let mut c = Comp::new();
    // The bars' roots: E E A E | E E C B, twice.
    let roots: [i8; 8] = [0, 0, 5, 0, 0, 0, -4, -5];
    for half in 0..2u32 {
        let b0 = half * 8;
        // Drone: always under it.
        c.layer(L_DRONE).chord(b0, 0, Inst::Drone, &[0], 64, 0.7).chord(b0 + 4, 0, Inst::Drone, &[if half == 0 { 0 } else { 1 }], 64, 0.7);
        // Sparse: a few ringing notes, like someone tuning in the heat.
        c.layer(L_SPARSE)
            .line(b0, Inst::Guitar, &[(0, 12, 16, 0.7), (10, 7, 6, 0.4)])
            .line(b0 + 1, Inst::Guitar, &[(8, 10, 8, 0.5)])
            .line(b0 + 2, Inst::Guitar, &[(0, 13, 16, 0.6), (12, 12, 4, 0.4)])
            .line(b0 + 3, Inst::Guitar, &[(4, 0, 12, 0.6)])
            .line(b0 + 4, Inst::Guitar, &[(0, 12, 8, 0.6), (8, 15, 8, 0.5)])
            .line(b0 + 5, Inst::Guitar, &[(0, 13, 12, 0.5)])
            .line(b0 + 6, Inst::Guitar, &[(0, 8, 8, 0.6), (8, 7, 8, 0.5)])
            .line(b0 + 7, Inst::Guitar, &[(0, 1, 6, 0.5), (8, 0, 8, 0.6)]);
        for (k, root) in roots.iter().enumerate() {
            let bar = b0 + k as u32;
            c.layer(L_CHUG).line_t(bar, Inst::Mute, *root, &CHUG);
            c.layer(L_BASS).line(bar, Inst::Bass, &[(0, *root, 3, 1.0), (4, *root + 1, 2, 0.8), (8, *root, 3, 1.0), (12, *root, 2, 0.8), (14, *root - 2, 2, 0.8)]);
        }
        // Accent chords where the riff turns.
        c.layer(L_CHUG).chord(b0 + 6, 0, Inst::Guitar, &[-4, 3, 8], 8, 0.9).chord(b0 + 7, 0, Inst::Guitar, &[-5, 2, 7], 8, 0.9);
        c.layer(L_DRUMS_LIGHT)
            .hits(b0, 8, Inst::Kick, "X.......X.x.....")
            .hits(b0, 8, Inst::Stomp, "....X.......X...")
            .hits(b0, 8, Inst::Shaker, "xoxoxoxoxoxoxoxo");
        c.layer(L_DRUMS_FULL)
            .hits(b0, 8, Inst::Snare, "....X.......X..o")
            .hits(b0, 8, Inst::Kick, "......x.......x.")
            .hits(b0, 8, Inst::Clap, "....o.......X...")
            .hits(b0 + 1, 1, Inst::Clank, "..............X.")
            .hits(b0 + 3, 1, Inst::Clank, "......X.......x.")
            .hits(b0 + 5, 1, Inst::Clank, "..............X.");
        c.layer(L_DRUMS_FULL)
            .hits(b0 + 7, 1, Inst::TomHi, "........XxX.....")
            .hits(b0 + 7, 1, Inst::TomMid, "...........XxX..")
            .hits(b0 + 7, 1, Inst::TomLo, "..............XX");
    }
    // The harmonica wails, a different answer each half.
    c.layer(L_LEAD).line(
        0,
        Inst::Lead,
        &[
            (0, 7, 4, 0.9),
            (4, 8, 4, 0.8),
            (8, 7, 8, 1.0),
            (16, 5, 4, 0.8),
            (20, 3, 4, 0.8),
            (24, 1, 8, 0.9),
            (32, 0, 14, 1.0),
            (48, 12, 4, 0.9),
            (52, 13, 2, 0.8),
            (54, 12, 2, 0.8),
            (56, 10, 4, 0.8),
            (60, 8, 4, 0.8),
            (64, 7, 16, 1.0),
            (80, 5, 4, 0.8),
            (84, 7, 4, 0.8),
            (88, 8, 8, 0.9),
            (96, 7, 4, 0.8),
            (100, 5, 4, 0.8),
            (104, 3, 4, 0.8),
            (108, 1, 4, 0.8),
            (112, 0, 16, 1.0),
        ],
    );
    c.layer(L_LEAD).line(
        8,
        Inst::Lead,
        &[
            (0, 12, 8, 1.0),
            (8, 13, 4, 0.9),
            (12, 12, 4, 0.8),
            (16, 10, 8, 0.9),
            (24, 8, 8, 0.8),
            (32, 7, 6, 0.9),
            (38, 8, 2, 0.7),
            (40, 7, 8, 0.9),
            (48, 3, 4, 0.8),
            (52, 5, 4, 0.8),
            (56, 7, 8, 0.9),
            (64, 15, 8, 1.0),
            (72, 13, 4, 0.9),
            (76, 12, 4, 0.8),
            (80, 13, 16, 1.0),
            (96, 12, 8, 0.9),
            (104, 8, 4, 0.8),
            (108, 7, 4, 0.8),
            (112, 7, 16, 1.0),
        ],
    );
    c.song(112.0, 16, true, 0.06)
}

// ── Stingers ────────────────────────────────────────────────────────────
// Each is a few seconds of notes; the last ones are left to ring out.

/// Round start: a tom roll, a power chord with a clank, a twang slide up.
#[must_use]
pub fn round_start() -> Song {
    let mut c = Comp::new();
    c.hits(0, 1, Inst::TomHi, "x.x.x...........")
        .hits(0, 1, Inst::TomMid, "......x.........")
        .hits(0, 1, Inst::TomLo, ".......xX.......");
    c.chord(0, 8, Inst::Guitar, &[0, 7, 12], 16, 1.0)
        .line(0, Inst::Bass, &[(8, 0, 14, 1.0)])
        .hits(0, 1, Inst::Kick, "........X.......")
        .hits(0, 1, Inst::Clank, "........X.......")
        .hits(0, 1, Inst::Stomp, "........X.......");
    c.line(0, Inst::Guitar, &[(14, 5, 2, 0.6), (16, 6, 2, 0.6), (18, 7, 8, 0.8)]);
    c.song_steps(112.0, 24, false, 0.0)
}

/// Victory: the harmonica turns major and struts; the band lands on E major.
#[must_use]
pub fn victory() -> Song {
    let mut c = Comp::new();
    c.line(0, Inst::Lead, &[(0, 7, 2, 0.9), (2, 9, 2, 0.8), (4, 12, 4, 1.0), (8, 11, 2, 0.8), (10, 9, 2, 0.8), (12, 7, 4, 0.9), (16, 4, 4, 0.8), (20, 7, 4, 0.9), (24, 12, 14, 1.0)]);
    c.line(0, Inst::Guitar, &[(0, 0, 3, 1.0), (4, 4, 3, 0.8), (8, 7, 3, 0.9), (12, 4, 3, 0.8), (16, 0, 3, 1.0), (20, 7, 3, 0.9)])
        .line(0, Inst::Bass, &[(0, 0, 4, 1.0), (4, 4, 4, 0.8), (8, 7, 4, 0.9), (12, 9, 4, 0.8), (16, 0, 4, 1.0), (20, 7, 4, 0.9)])
        .hits(0, 2, Inst::Kick, "X...X...X...X...")
        .hits(0, 2, Inst::Clap, "....X.......X...");
    c.chord(0, 24, Inst::Guitar, &[0, 7, 12, 16, 19], 14, 1.0)
        .line(0, Inst::Bass, &[(24, 0, 14, 1.0)])
        .hits(1, 1, Inst::Clank, "........X.......")
        .hits(1, 1, Inst::TomLo, "....x.x.........");
    c.song_steps(112.0, 38, false, 0.0)
}

/// Defeat: the line walks down, the harmonica sags, one clank, a minor
/// chord left ringing in the wind.
#[must_use]
pub fn defeat() -> Song {
    let mut c = Comp::new();
    c.line(0, Inst::Guitar, &[(0, 7, 4, 0.9), (4, 5, 4, 0.8), (8, 3, 4, 0.8), (12, 2, 4, 0.7)])
        .line(0, Inst::Bass, &[(0, 0, 8, 0.8), (8, -2, 8, 0.7), (16, -5, 14, 0.8)])
        .line(0, Inst::Lead, &[(2, 7, 6, 0.8), (8, 6, 2, 0.7), (10, 5, 6, 0.7), (16, 0, 14, 0.7)]);
    c.chord(0, 16, Inst::Guitar, &[0, 7, 12, 15], 14, 0.7)
        .chord(0, 0, Inst::Drone, &[0], 30, 0.5)
        .hits(1, 1, Inst::Clank, "X...............")
        .hits(1, 1, Inst::TomLo, "X...............");
    c.song_steps(100.0, 30, false, 0.0)
}

/// Wave start: stomp, stomp, clank; the chug; a power chord.
#[must_use]
pub fn wave_start() -> Song {
    let mut c = Comp::new();
    c.hits(0, 1, Inst::Stomp, "X.X.............")
        .hits(0, 1, Inst::Kick, "X.X.............")
        .hits(0, 1, Inst::Clank, "....X...........");
    for &(step, note, len, vel) in &CHUG {
        c.line(0, Inst::Mute, &[(step + 8, note, len, vel)]);
    }
    c.line(0, Inst::Bass, &[(8, 0, 3, 1.0), (12, 1, 2, 0.8), (16, 0, 3, 1.0), (20, 0, 4, 0.8)])
        .line(0, Inst::Snare, &[(12, 0, 1, 1.0), (20, 0, 1, 1.0), (22, 0, 1, 0.6), (23, 0, 1, 0.8)])
        .line(0, Inst::Kick, &[(8, 0, 1, 1.0), (16, 0, 1, 1.0), (18, 0, 1, 0.7)]);
    c.chord(0, 24, Inst::Guitar, &[0, 7, 12], 10, 1.0)
        .line(0, Inst::Bass, &[(24, 0, 10, 1.0)])
        .line(0, Inst::Clank, &[(24, 0, 1, 1.0)])
        .line(0, Inst::Kick, &[(24, 0, 1, 1.0)]);
    c.song_steps(112.0, 32, false, 0.0)
}

/// Boss appears: a drone swells, two heavy half-step hits (F, then E), toms
/// rumble, a huge clank, the harmonica screams.
#[must_use]
pub fn boss_appears() -> Song {
    let mut c = Comp::new();
    c.chord(0, 0, Inst::Drone, &[0], 26, 0.9);
    c.chord(0, 6, Inst::Guitar, &[1, 8, 13], 6, 1.0)
        .line(0, Inst::Bass, &[(6, 1, 6, 1.0), (12, 0, 20, 1.0)])
        .chord(0, 12, Inst::Guitar, &[0, 7, 12], 20, 1.0)
        .line(0, Inst::Kick, &[(6, 0, 1, 1.0), (12, 0, 1, 1.0), (28, 0, 1, 1.0)])
        .line(0, Inst::Clank, &[(6, 0, 1, 0.8), (12, 0, 1, 1.0), (28, 0, 1, 1.0)])
        .hits(1, 1, Inst::TomLo, "..xx..xxxx......");
    c.line(0, Inst::Lead, &[(20, 13, 4, 1.0), (24, 12, 12, 1.0)]);
    c.song_steps(100.0, 36, false, 0.0)
}
