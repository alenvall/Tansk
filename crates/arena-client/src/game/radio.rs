//! The caller on the radio: terse, dry, a little tired of everyone.

/// Gap between ordinary callouts, s; urgent ones cut in.
const GAP: f32 = 3.5;

#[derive(Default)]
pub struct Radio {
    cooldown: f32,
    n: usize,
    /// A line waiting for `take`.
    pending: Option<String>,
}

impl Radio {
    pub fn tick(&mut self, dt: f32) {
        self.cooldown = (self.cooldown - dt).max(0.0);
    }

    /// Picks one of `lines` (rotating so repeats are rarer) and queues it
    /// unless the band is busy and the line isn't `urgent`.
    pub fn say(&mut self, lines: &[String], urgent: bool) {
        if lines.is_empty() || (!urgent && self.cooldown > 0.0) {
            return;
        }
        self.n = self.n.wrapping_add(1);
        self.pending = Some(lines[self.n % lines.len()].clone());
        self.cooldown = GAP;
    }

    pub fn take(&mut self) -> Option<String> {
        self.pending.take()
    }
}

pub fn first_blood(killer: &str, victim: &str, you_killed: bool) -> Vec<String> {
    if you_killed {
        vec![
            "First blood's yours. Don't let it go to your head.".into(),
            format!("First blood. {victim} won't be the last."),
        ]
    } else {
        vec![
            format!("{killer} draws first blood on {victim}. Here we go."),
            format!("First one down. {victim} had a good run. Short, but good."),
        ]
    }
}

pub fn streak(name: &str, n: u32, you: bool) -> Vec<String> {
    match (you, n) {
        (true, 3) => vec!["Three down. The Sink's taking notes.".into(), "Hat trick. Keep the engine warm.".into()],
        (true, 5) => vec!["Five. They're painting your outline on their hulls now.".into()],
        (true, _) => vec![format!("{n} in a row. Someone fetch a bigger scrapyard.")],
        (false, 3) => vec![format!("{name}'s on three. Somebody do something."), format!("{name}, three straight. Watch that one.")],
        (false, 5) => vec![format!("{name}'s on five. That's not luck anymore.")],
        (false, _) => vec![format!("{name}'s on {n}. Everyone, please. Anyone.")],
    }
}

pub fn upgraded(part: &str, tier: u8) -> Vec<String> {
    if tier >= 3 {
        vec![format!("{part}, tier three. Now you're a problem."), format!("Tier three {part}. They'll hear you coming.")]
    } else {
        vec![
            format!("{part}'s on. Weld's still hot."),
            format!("{part} bolted down. Try not to lose it."),
            format!("{part}. Wear it well."),
        ]
    }
}

pub fn part_lost(part: &str) -> Vec<String> {
    vec![format!("Lost the {part}. It's on the sand somewhere."), format!("There goes the {part}. Go get it back.")]
}

pub fn track_blown() -> Vec<String> {
    vec!["Track's off. Crew's on it.".into(), "Threw a track. Sit tight, they're swinging hammers.".into()]
}

pub fn you_died(killer: Option<&str>) -> Vec<String> {
    match killer {
        Some(k) => vec![format!("Brewed up by {k}. Shake it off."), format!("{k} opened you like a tin. Back in a sec.")],
        None => vec!["The Sink got you. It does that.".into()],
    }
}

pub fn bounty(name: &str, parts: usize, dir: &str, dist: f32) -> Vec<String> {
    let m = (dist / 10.0).round() as i32 * 10;
    vec![
        format!("Big one heading {dir}. {name}, {parts} parts and counting."),
        format!("{name}'s built like a refinery. {m} metres {dir}. Take it apart."),
    ]
}

pub fn air_strike(by_you: bool, dir: &str) -> Vec<String> {
    if by_you {
        vec!["Flare's lit. Bombers inbound. Maybe don't stand under it.".into()]
    } else {
        vec![format!("Flare's up, {dir}. Get clear of the red smoke."), format!("Air strike called, {dir}. Off the red smoke, now.")]
    }
}

pub fn hazard(dir: &str, close: bool) -> Vec<String> {
    if close {
        vec!["Fuel went up right next to you. Eyebrows still on?".into()]
    } else {
        vec![format!("Fuel went up to the {dir}. Lovely colour."), format!("Something big just cooked off, {dir}.")]
    }
}

pub fn match_over(winner: Option<&str>, you_won: bool) -> Vec<String> {
    match (you_won, winner) {
        (true, _) => vec!["That's the match. Yours. Drinks are warm.".into()],
        (false, Some(w)) => vec![format!("That's the match. {w} takes it. Pencils down.")],
        (false, None) => vec!["That's the match. Nobody wins. Typical.".into()],
    }
}

pub fn branch_taken(part: &str) -> Vec<String> {
    vec![format!("{part} it is."), format!("Going {part}. Bold.")]
}

fn number(n: u16) -> String {
    const W: [&str; 13] =
        ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve"];
    W.get(n as usize).map_or_else(|| n.to_string(), |w| w.to_string())
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
}

pub fn wave(wave: u16, raiders: u16, boss: bool) -> Vec<String> {
    let w = cap(&number(wave));
    if boss {
        vec![format!("Wave {}. Something big's on the horizon. Bring spanners.", number(wave))]
    } else if wave <= 1 {
        vec![format!("Here they come. {raiders} raiders, light stuff. Warm-up.")]
    } else if raiders > 10 {
        vec![format!("Wave {}. {raiders} of them. Buggies first, then the heavies.", number(wave))]
    } else {
        vec![format!("Wave {}. Buggies first, then the heavies.", number(wave)), format!("{w}. Same as before, only meaner.")]
    }
}

pub fn wave_cleared(wave: u16) -> Vec<String> {
    vec![
        format!("Wave {} done. Breathe. Patch what you can.", number(wave)),
        "That's the lot. Grab their scrap before the next lot.".into(),
    ]
}

pub fn outpost(frac: f32) -> Vec<String> {
    if frac <= 0.25 {
        vec!["Outpost's hanging by a bolt. Get back here.".into()]
    } else if frac <= 0.5 {
        vec!["They're tearing into the outpost. Half gone.".into()]
    } else {
        vec!["Raiders on the outpost. Shoo them off.".into()]
    }
}

pub fn raid_lost(waves: u16, outpost_fell: bool) -> Vec<String> {
    let n = number(waves);
    if outpost_fell {
        vec![format!("Outpost's gone. {} waves. They'll write songs. Short ones.", cap(&n))]
    } else {
        vec![format!("Whole crew's scrap. {} waves held. Not bad, not good.", cap(&n))]
    }
}

pub fn boss_spawned(dir: &str) -> Vec<String> {
    vec![format!("That's the War Rig, {dir}. Take its engines out."), "War Rig on the field. Plates first, then the core.".into()]
}

pub fn boss_module(part: &str) -> Vec<String> {
    vec![format!("Rig's lost its {part}. Keep cutting."), format!("{part} off the rig. It noticed.")]
}

pub fn boss_core() -> Vec<String> {
    vec!["Plates are off. The core's open. Everything into it.".into()]
}

pub fn boss_down(killer: Option<&str>, you: bool) -> Vec<String> {
    match (you, killer) {
        (true, _) => vec!["You dropped the War Rig. Somebody frame that.".into()],
        (false, Some(k)) => vec![format!("War Rig's down. {k} gets the drinks.")],
        (false, None) => vec!["War Rig's down. Nobody's sure who did it.".into()],
    }
}
