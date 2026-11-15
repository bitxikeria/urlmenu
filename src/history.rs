//! Picks per site, scored by frequency and recency like zoxide, kept in
//! `$XDG_STATE_HOME/urlmenu/history.json`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::paths::Paths;

const MAX_SITES: usize = 500;
/// Total rank of a site above which its ranks age: old habits fade.
const MAX_RANK: f64 = 20.0;
const AGING: f64 = 0.9;
/// Aged ranks below this are dropped.
const MIN_RANK: f64 = 0.5;

const HOUR: u64 = 3600;
const DAY: u64 = 24 * HOUR;
const WEEK: u64 = 7 * DAY;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct History {
    sites: BTreeMap<String, Site>,
}

/// Target id to its stats.
type Site = BTreeMap<String, Stat>;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Stat {
    pub rank: f64,
    /// Unix time of the last pick.
    pub last: u64,
}

impl Stat {
    /// Rank weighted by the age of the last pick.
    pub fn score(&self, now: u64) -> f64 {
        let age = now.saturating_sub(self.last);
        let weight = match age {
            a if a < HOUR => 4.0,
            a if a < DAY => 2.0,
            a if a < WEEK => 0.5,
            _ => 0.25,
        };
        self.rank * weight
    }
}

pub fn path(paths: &Paths) -> PathBuf {
    paths.state_home.join("urlmenu/history.json")
}

/// Host without a leading `www.`; empty for no URL, which keys the browser
/// started without a link.
pub fn key(url: Option<&str>) -> String {
    url.and_then(|u| url::Url::parse(u).ok())
        .and_then(|u| u.host_str().map(normalize_host))
        .unwrap_or_default()
}

/// Key of a URL, or of a bare host such as `www.github.com`.
pub fn key_of(input: &str) -> String {
    if input.contains("://") {
        key(Some(input))
    } else {
        normalize_host(input)
    }
}

fn normalize_host(host: &str) -> String {
    let host = host.to_lowercase();
    host.strip_prefix("www.").unwrap_or(&host).to_string()
}

/// Remembered target without its `+private` suffix: the browser and profile
/// come first in both the normal and the private view.
pub fn base_id(id: &str) -> &str {
    id.strip_suffix("+private").unwrap_or(id)
}

impl History {
    /// A missing file is an empty history; a corrupt one is reported and ignored.
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                eprintln!("urlmenu: {}: {e}; starting a new history", path.display());
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// Target with the best score for this site; ties go to the latest pick.
    pub fn best(&self, key: &str, now: u64) -> Option<&str> {
        self.scores(key, now).first().map(|(id, _)| *id)
    }

    /// Targets of a site with their stats, best first.
    pub fn scores(&self, key: &str, now: u64) -> Vec<(&str, Stat)> {
        let mut out: Vec<_> = self
            .sites
            .get(key)
            .into_iter()
            .flatten()
            .map(|(id, stat)| (id.as_str(), *stat))
            .collect();
        out.sort_by(|(_, a), (_, b)| {
            b.score(now)
                .total_cmp(&a.score(now))
                .then(b.last.cmp(&a.last))
        });
        out
    }

    /// Remembered sites whose host contains `fragment`, case-insensitive.
    pub fn sites_matching(&self, fragment: &str) -> Vec<&str> {
        let fragment = fragment.to_lowercase();
        self.sites
            .keys()
            .filter(|k| !k.is_empty() && k.contains(&fragment))
            .map(String::as_str)
            .collect()
    }

    pub fn record(&mut self, key: String, target: &str, now: u64) {
        let site = self.sites.entry(key).or_default();
        let stat = site.entry(base_id(target).to_string()).or_insert(Stat {
            rank: 0.0,
            last: now,
        });
        stat.rank += 1.0;
        stat.last = now;
        if site.values().map(|s| s.rank).sum::<f64>() > MAX_RANK {
            for s in site.values_mut() {
                s.rank *= AGING;
            }
            site.retain(|_, s| s.rank >= MIN_RANK);
        }
        while self.sites.len() > MAX_SITES {
            let oldest = self
                .sites
                .iter()
                .min_by_key(|(_, site)| site.values().map(|s| s.last).max())
                .map(|(k, _)| k.clone());
            match oldest {
                Some(k) => self.sites.remove(&k),
                None => break,
            };
        }
    }

    /// Drops a site; false when nothing was remembered for it.
    pub fn forget(&mut self, key: &str) -> bool {
        self.sites.remove(key).is_some()
    }

    pub fn clear(&mut self) {
        self.sites.clear();
    }

    /// Written to a temporary file then renamed, so a crash never leaves half a file.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let dir = path.parent().ok_or("bad history path")?;
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let tmp = path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
    }
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: u64 = 1_000_000_000;

    #[test]
    fn keys() {
        let cases: &[(Option<&str>, &str)] = &[
            (Some("https://www.GitHub.com/o/r"), "github.com"),
            (
                Some("https://openproject.mobicoop.io/wp/1"),
                "openproject.mobicoop.io",
            ),
            (Some("file:///tmp/a.html"), ""),
            (Some("/tmp/a.html"), ""),
            (None, ""),
        ];
        for (url, expected) in cases {
            assert_eq!(key(*url), *expected, "{url:?}");
        }
        assert_eq!(key_of("https://www.github.com/x"), "github.com");
        assert_eq!(key_of("WWW.GitHub.com"), "github.com");
    }

    #[test]
    fn recency_weights() {
        let stat = |age| Stat {
            rank: 2.0,
            last: T - age,
        };
        let cases = [(0, 8.0), (2 * HOUR, 4.0), (2 * DAY, 1.0), (2 * WEEK, 0.5)];
        for (age, expected) in cases {
            assert_eq!(stat(age).score(T), expected, "age {age}");
        }
    }

    #[test]
    fn frequent_pick_beats_one_off() {
        let mut h = History::default();
        for i in 0..4 {
            h.record("a".into(), "ff", T + i);
        }
        h.record("a".into(), "chromium+private", T + 10);
        assert_eq!(h.best("a", T + 20), Some("ff"));
        let scores = h.scores("a", T + 20);
        assert_eq!(scores[1].0, "chromium", "private suffix stripped");
    }

    #[test]
    fn recent_pick_beats_stale_habit() {
        let mut h = History::default();
        for i in 0..3 {
            h.record("a".into(), "ff", T + i);
        }
        let later = T + 2 * WEEK;
        h.record("a".into(), "chromium", later);
        // ff: 3 × 0.25; chromium: 1 × 4.
        assert_eq!(h.best("a", later), Some("chromium"));
    }

    #[test]
    fn ties_go_to_latest() {
        let mut h = History::default();
        h.record("a".into(), "ff", T);
        h.record("a".into(), "chromium", T + 1);
        assert_eq!(h.best("a", T + 2), Some("chromium"));
    }

    #[test]
    fn ranks_age_and_drop() {
        let mut h = History::default();
        h.record("a".into(), "rare", T);
        for i in 0..40 {
            h.record("a".into(), "ff", T + 1 + i);
        }
        let total: f64 = h.sites["a"].values().map(|s| s.rank).sum();
        assert!(total <= MAX_RANK + 1.0, "total {total}");
        assert!(!h.sites["a"].contains_key("rare"), "aged out");
    }

    #[test]
    fn prunes_oldest_site_and_forgets() {
        let mut h = History::default();
        h.record("a".into(), "ff", 1);
        for i in 0..MAX_SITES as u64 {
            h.record(format!("k{i}"), "x", 10 + i);
        }
        assert_eq!(h.sites.len(), MAX_SITES);
        assert_eq!(h.best("a", T), None, "oldest site pruned");
        let matching = h.sites_matching("K49");
        assert_eq!(matching.len(), 11, "k49 and k490..k499");
        assert_eq!(matching[0], "k49");
        assert!(h.forget("k0"));
        assert!(!h.forget("k0"));
        h.clear();
        assert_eq!(h.best("k1", T), None);
    }

    #[test]
    fn save_and_load() {
        let dir = std::env::temp_dir().join(format!("urlmenu-history-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("state/history.json");
        assert_eq!(
            History::load(&path).best("x", T),
            None,
            "missing file is empty"
        );
        let mut h = History::default();
        h.record("github.com".into(), "chromium/Profile 1", 5);
        h.save(&path).unwrap();
        assert_eq!(
            History::load(&path).best("github.com", T),
            Some("chromium/Profile 1")
        );
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(History::load(&path).best("github.com", T), None);
    }
}
