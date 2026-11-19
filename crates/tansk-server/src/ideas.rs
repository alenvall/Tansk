//! The idea box: players post a bug, a tuning note or an idea from the game
//! (`POST /api/ideas`); each lands as one JSON line in
//! `$IDEAS_DIR/ideas.jsonl` (default `./data`). No IP is stored: addresses
//! only key an in-memory rate limit. `GET /api/ideas` lists them, with
//! `Authorization: Bearer $IDEAS_TOKEN` only (unset: no listing at all).

use std::collections::{HashMap, VecDeque};
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Longest text, characters.
pub const MAX_TEXT: usize = 400;
const MAX_NAME: usize = 24;
const MAX_TAG: usize = 40;
/// Largest request body, bytes.
pub const MAX_BODY: usize = 4096;
/// Posts per address per hour.
pub const PER_HOUR: usize = 5;
/// The file stops growing here.
pub const MAX_FILE: u64 = 5 * 1024 * 1024;

/// What the client sends.
#[derive(Debug, Deserialize)]
pub struct IdeaIn {
    pub category: String,
    pub text: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub map: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
}

/// One stored line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Idea {
    /// Unix seconds.
    pub time: u64,
    pub category: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub map: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    /// Bad input, with the reason for the player.
    Invalid(&'static str),
    TooMany,
    Full,
    Storage,
}

impl Refused {
    pub fn message(&self) -> &'static str {
        match self {
            Refused::Invalid(m) => m,
            Refused::TooMany => "That's a lot of ideas at once. Try again in a while.",
            Refused::Full => "The idea box is full for now.",
            Refused::Storage => "The idea box is stuck. Try again later.",
        }
    }
}

/// Control characters out (newlines become spaces), whitespace squeezed,
/// at most `max` characters.
fn clean(s: &str, max: usize) -> String {
    let s: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let squeezed = s.split_whitespace().collect::<Vec<_>>().join(" ");
    squeezed.chars().take(max).collect()
}

fn tag(s: Option<String>, max: usize) -> Option<String> {
    s.map(|s| clean(&s, max)).filter(|s| !s.is_empty())
}

/// Checks and tidies a post.
pub fn validate(input: IdeaIn, time: u64) -> Result<Idea, Refused> {
    let category = match input.category.trim().to_ascii_lowercase().as_str() {
        "bug" => "bug",
        "tuning" => "tuning",
        "idea" => "idea",
        _ => return Err(Refused::Invalid("Pick Bug, Tuning or Idea.")),
    }
    .to_string();
    if input.text.chars().count() > MAX_TEXT + 40 {
        return Err(Refused::Invalid("That's too long: 400 characters at most."));
    }
    let text = clean(&input.text, MAX_TEXT);
    if text.chars().count() < 3 {
        return Err(Refused::Invalid("Write a few words first."));
    }
    Ok(Idea {
        time,
        category,
        text,
        name: tag(input.name, MAX_NAME),
        version: tag(input.version, MAX_TAG),
        map: tag(input.map, MAX_TAG),
        mode: tag(input.mode, MAX_TAG),
    })
}

pub struct Ideas {
    dir: PathBuf,
    token: Option<String>,
    recent: Mutex<HashMap<String, VecDeque<Instant>>>,
    file: Mutex<()>,
    max_file: u64,
}

impl Ideas {
    pub fn new(dir: impl Into<PathBuf>, token: Option<String>) -> Self {
        Ideas { dir: dir.into(), token: token.filter(|t| !t.trim().is_empty()), recent: Mutex::default(), file: Mutex::new(()), max_file: MAX_FILE }
    }

    /// `IDEAS_DIR` (default `./data`) and `IDEAS_TOKEN`.
    pub fn from_env() -> Self {
        Ideas::new(std::env::var("IDEAS_DIR").unwrap_or_else(|_| "data".into()), std::env::var("IDEAS_TOKEN").ok())
    }

    fn path(&self) -> PathBuf {
        self.dir.join("ideas.jsonl")
    }

    /// Counts a post from `who` against the hourly limit.
    fn allow(&self, who: &str, now: Instant) -> bool {
        let mut recent = self.recent.lock().unwrap();
        let hour = Duration::from_secs(3600);
        recent.retain(|_, q| {
            while q.front().is_some_and(|t| now.duration_since(*t) > hour) {
                q.pop_front();
            }
            !q.is_empty()
        });
        let q = recent.entry(who.to_string()).or_default();
        if q.len() >= PER_HOUR {
            return false;
        }
        q.push_back(now);
        true
    }

    /// Takes a post from address `who`.
    pub fn post(&self, who: &str, input: IdeaIn) -> Result<Idea, Refused> {
        let time = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let idea = validate(input, time)?;
        if !self.allow(who, Instant::now()) {
            return Err(Refused::TooMany);
        }
        let mut line = serde_json::to_string(&idea).map_err(|_| Refused::Storage)?;
        line.push('\n');
        let _guard = self.file.lock().unwrap();
        let size = std::fs::metadata(self.path()).map_or(0, |m| m.len());
        if size + line.len() as u64 > self.max_file {
            return Err(Refused::Full);
        }
        std::fs::create_dir_all(&self.dir).map_err(|_| Refused::Storage)?;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(self.path()).map_err(|_| Refused::Storage)?;
        f.write_all(line.as_bytes()).map_err(|_| Refused::Storage)?;
        Ok(idea)
    }

    /// May this `Authorization` header read the ideas? Never without a token set.
    pub fn authorized(&self, header: Option<&str>) -> bool {
        let Some(token) = &self.token else { return false };
        let Some(given) = header.and_then(|h| h.strip_prefix("Bearer ")) else { return false };
        // Same length and every byte compared, so the time doesn't hint.
        given.len() == token.len() && given.bytes().zip(token.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
    }

    pub fn listing_enabled(&self) -> bool {
        self.token.is_some()
    }

    /// Everything posted, oldest first.
    pub fn all(&self) -> Vec<Idea> {
        let _guard = self.file.lock().unwrap();
        std::fs::read_to_string(self.path())
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(category: &str, text: &str) -> IdeaIn {
        IdeaIn { category: category.into(), text: text.into(), name: Some(" Ann\u{7} ".into()), version: Some("0.1.0".into()), map: None, mode: None }
    }

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tansk-ideas-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn validation() {
        let i = validate(input("BUG", "  Tank\nstuck \u{1b}[31m under the  bridge "), 9).unwrap();
        assert_eq!(i.category, "bug");
        assert_eq!(i.text, "Tank stuck [31m under the bridge");
        assert_eq!(i.name.as_deref(), Some("Ann"));
        assert_eq!(validate(input("rant", "hello there"), 0), Err(Refused::Invalid("Pick Bug, Tuning or Idea.")));
        assert!(matches!(validate(input("idea", " \n "), 0), Err(Refused::Invalid(_))));
        assert!(matches!(validate(input("idea", &"x".repeat(500)), 0), Err(Refused::Invalid(_))));
        let long = validate(input("tuning", &"y ".repeat(210)), 0).unwrap();
        assert!(long.text.chars().count() <= MAX_TEXT);
        let mut no_name = input("idea", "More horns");
        no_name.name = Some("   ".into());
        assert_eq!(validate(no_name, 0).unwrap().name, None);
    }

    #[test]
    fn stores_lines_rate_limits_and_caps() {
        let dir = temp("store");
        let box_ = Ideas::new(&dir, None);
        for i in 0..PER_HOUR {
            box_.post("1.2.3.4", input("idea", &format!("Idea number {i}"))).unwrap();
        }
        assert_eq!(box_.post("1.2.3.4", input("idea", "one more")), Err(Refused::TooMany));
        // Another address is fine; a bad post doesn't use up the limit.
        assert!(box_.post("5.6.7.8", input("nope", "x")).is_err());
        box_.post("5.6.7.8", input("bug", "Another one")).unwrap();
        let all = box_.all();
        assert_eq!(all.len(), PER_HOUR + 1);
        assert_eq!(all[0].text, "Idea number 0");
        let raw = std::fs::read_to_string(dir.join("ideas.jsonl")).unwrap();
        assert!(!raw.contains("1.2.3.4") && !raw.contains("5.6.7.8"), "no addresses stored");
        assert_eq!(raw.lines().count(), PER_HOUR + 1);
        // The file cap.
        let mut small = Ideas::new(&dir, None);
        small.max_file = raw.len() as u64 + 10;
        assert_eq!(small.post("9.9.9.9", input("idea", "This one does not fit")), Err(Refused::Full));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn listing_needs_the_token() {
        let off = Ideas::new(temp("auth0"), None);
        assert!(!off.listing_enabled() && !off.authorized(Some("Bearer ")));
        let on = Ideas::new(temp("auth1"), Some("s3cret".into()));
        assert!(on.authorized(Some("Bearer s3cret")));
        assert!(!on.authorized(Some("Bearer s3cre")));
        assert!(!on.authorized(Some("s3cret")));
        assert!(!on.authorized(None));
        assert!(!Ideas::new(temp("auth2"), Some("  ".into())).listing_enabled(), "a blank token is no token");
    }
}
