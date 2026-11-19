//! "What's new" and "Coming up" on the title, from `docs/CHANGELOG.md`
//! (compiled in): `## YYYY-MM-DD Title` headings with `-` bullets, newest
//! first, and one `## Coming up` section.

use std::fmt::Write as _;

use super::esc;

pub const CHANGELOG: &str = include_str!("../../../../docs/CHANGELOG.md");

/// Releases shown before "More".
pub const SHOWN: usize = 5;

#[derive(Debug, Default, PartialEq)]
pub struct Release {
    /// `YYYY-MM-DD`, or empty.
    pub date: String,
    pub title: String,
    pub bullets: Vec<String>,
}

#[derive(Debug, Default, PartialEq)]
pub struct Changelog {
    pub coming: Vec<String>,
    pub releases: Vec<Release>,
}

fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    s.len() == 10 && b[4] == b'-' && b[7] == b'-' && s.chars().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

pub fn parse(src: &str) -> Changelog {
    let mut log = Changelog::default();
    // None: before the first section; Some(None): Coming up; Some(Some(i)): release i.
    let mut at: Option<Option<usize>> = None;
    for line in src.lines() {
        let line = line.trim_end();
        if let Some(h) = line.strip_prefix("## ") {
            let h = h.trim();
            if h.eq_ignore_ascii_case("coming up") {
                at = Some(None);
                continue;
            }
            let (date, title) = match h.split_once(' ') {
                Some((d, t)) if is_date(d) => (d.to_string(), t.trim().to_string()),
                _ if is_date(h) => (h.to_string(), String::new()),
                _ => (String::new(), h.to_string()),
            };
            log.releases.push(Release { date, title, bullets: Vec::new() });
            at = Some(Some(log.releases.len() - 1));
        } else if let Some(b) = line.trim_start().strip_prefix("- ") {
            match at {
                Some(None) => log.coming.push(b.trim().to_string()),
                Some(Some(i)) => log.releases[i].bullets.push(b.trim().to_string()),
                None => {}
            }
        } else if let (Some(cont), true) = (at, line.starts_with("  ") && !line.trim().is_empty()) {
            // A bullet wrapped onto the next line.
            let last = match cont {
                None => log.coming.last_mut(),
                Some(i) => log.releases[i].bullets.last_mut(),
            };
            if let Some(l) = last {
                l.push(' ');
                l.push_str(line.trim());
            }
        }
    }
    log
}

/// "19 Nov" from `2026-11-19`.
fn short_date(d: &str) -> String {
    const M: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    if !is_date(d) {
        return d.to_string();
    }
    let m: usize = d[5..7].parse().unwrap_or(1);
    let day: u32 = d[8..10].parse().unwrap_or(1);
    format!("{day} {}", M[m.clamp(1, 12) - 1])
}

/// The plate's body: the releases (the first [`SHOWN`], the rest behind
/// "More"), then Coming up.
pub fn html(log: &Changelog) -> String {
    let mut h = String::new();
    for (i, r) in log.releases.iter().enumerate() {
        let _ = write!(
            h,
            "<article class=\"t-news-rel\"{}><header><time>{}</time><b>{}</b></header><ul>",
            if i >= SHOWN { " data-more=\"1\"" } else { "" },
            esc(&short_date(&r.date)),
            esc(&r.title)
        );
        for b in &r.bullets {
            let _ = write!(h, "<li>{}</li>", esc(b));
        }
        h.push_str("</ul></article>");
    }
    if log.releases.len() > SHOWN {
        let _ = write!(h, "<button class=\"t-news-more\" data-action=\"news-more\">More ({} older)</button>", log.releases.len() - SHOWN);
    }
    if !log.coming.is_empty() {
        h.push_str("<section class=\"t-news-coming\"><h4>Coming up</h4><ul>");
        for b in &log.coming {
            let _ = write!(h, "<li>{}</li>", esc(b));
        }
        h.push_str("</ul></section>");
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_shipped_file_and_a_sample() {
        let log = parse(CHANGELOG);
        assert!(!log.releases.is_empty() && !log.coming.is_empty());
        assert!(log.releases.iter().all(|r| is_date(&r.date) && !r.bullets.is_empty()), "{log:?}");
        let s = "# Title\nintro\n\n## Coming up\n- Soon\n\n## 2026-11-20 Big day\n- One\n  wrapped\n- Two\n\n## 2026-11-19\n- Old\n";
        let l = parse(s);
        assert_eq!(l.coming, ["Soon"]);
        assert_eq!(l.releases.len(), 2);
        assert_eq!(l.releases[0], Release { date: "2026-11-20".into(), title: "Big day".into(), bullets: vec!["One wrapped".into(), "Two".into()] });
        assert_eq!(l.releases[1].title, "");
        assert_eq!(short_date("2026-11-20"), "20 Nov");
        let h = html(&l);
        assert!(h.contains("20 Nov") && h.contains("Coming up") && !h.contains("More ("));
    }
}
