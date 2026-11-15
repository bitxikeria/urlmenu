//! Profile discovery for Firefox-like and Chromium-like browsers.
//!
//! Firefox forks share profile roots (Firefox and Developer Edition both use
//! `~/.mozilla/firefox`), so a profile is attached to a browser only when its
//! `compatibility.ini` `LastPlatformDir` is the browser's install directory.

use std::path::{Path, PathBuf};

use super::Profile;
use crate::ini::Ini;
use crate::paths::Paths;

const FIREFOX_NAMES: &[&str] = &[
    "firefox",
    "librewolf",
    "floorp",
    "zen",
    "waterfox",
    "icecat",
];

const CHROMIUM_DIRS: &[(&str, &str)] = &[
    ("chromium", "chromium"),
    ("chromium-browser", "chromium"),
    ("google-chrome", "google-chrome"),
    ("google-chrome-stable", "google-chrome"),
    ("google-chrome-beta", "google-chrome-beta"),
    ("google-chrome-unstable", "google-chrome-unstable"),
    ("brave", "BraveSoftware/Brave-Browser"),
    ("brave-browser", "BraveSoftware/Brave-Browser"),
    ("vivaldi", "vivaldi"),
    ("vivaldi-stable", "vivaldi"),
    ("microsoft-edge", "microsoft-edge"),
    ("microsoft-edge-stable", "microsoft-edge"),
];

enum Family {
    Firefox,
    Chromium(&'static str),
}

fn family(program: &str) -> Option<Family> {
    let base = Path::new(program).file_name()?.to_str()?;
    if FIREFOX_NAMES.iter().any(|n| base.starts_with(n)) {
        return Some(Family::Firefox);
    }
    CHROMIUM_DIRS
        .iter()
        .find(|(name, _)| *name == base)
        .map(|(_, dir)| Family::Chromium(dir))
}

/// Private-window flag for browsers whose desktop entry has no private action.
pub fn private_flag(program: &str) -> Option<&'static str> {
    match family(program)? {
        Family::Firefox => Some("--private-window"),
        Family::Chromium(_) => Some("--incognito"),
    }
}

/// Lists profiles, or nothing when the browser has a single one.
pub fn detect(paths: &Paths, program: &str) -> Vec<Profile> {
    let profiles = match family(program) {
        Some(Family::Firefox) => firefox(paths, program),
        Some(Family::Chromium(dir)) => chromium(&paths.config_home.join(dir)),
        None => Vec::new(),
    };
    if profiles.len() < 2 {
        Vec::new()
    } else {
        profiles
    }
}

fn firefox_roots(paths: &Paths) -> Vec<PathBuf> {
    let h = &paths.home;
    vec![
        h.join(".mozilla/firefox"),
        paths.config_home.join("mozilla/firefox"),
        h.join(".librewolf"),
        h.join(".floorp"),
        h.join(".zen"),
        h.join(".waterfox"),
        h.join(".mozilla/icecat"),
    ]
}

fn canonical(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

fn firefox(paths: &Paths, program: &str) -> Vec<Profile> {
    let Some(install_dir) = paths
        .which(program)
        .map(|p| canonical(&p))
        .and_then(|p| p.parent().map(Path::to_path_buf))
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for root in firefox_roots(paths) {
        let Ok(text) = std::fs::read_to_string(root.join("profiles.ini")) else {
            continue;
        };
        let ini = Ini::parse(&text);
        for section in ini.sections().filter(|s| s.starts_with("Profile")) {
            let (Some(name), Some(rel)) = (ini.get(section, "Name"), ini.get(section, "Path"))
            else {
                continue;
            };
            let dir = if ini.get(section, "IsRelative") == Some("0") {
                PathBuf::from(rel)
            } else {
                root.join(rel)
            };
            let platform = std::fs::read_to_string(dir.join("compatibility.ini"))
                .ok()
                .and_then(|t| {
                    Ini::parse(&t)
                        .get("Compatibility", "LastPlatformDir")
                        .map(PathBuf::from)
                });
            if platform.is_some_and(|p| canonical(&p) == install_dir) {
                out.push(Profile {
                    name: name.to_string(),
                    key: name.to_string(),
                    args: vec!["-P".to_string(), name.to_string()],
                });
            }
        }
    }
    out
}

fn chromium(user_data_dir: &Path) -> Vec<Profile> {
    let Ok(text) = std::fs::read_to_string(user_data_dir.join("Local State")) else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let Some(cache) = json
        .pointer("/profile/info_cache")
        .and_then(|v| v.as_object())
    else {
        return Vec::new();
    };
    let mut out: Vec<Profile> = cache
        .iter()
        .map(|(dir, info)| Profile {
            name: info
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or(dir)
                .to_string(),
            key: dir.clone(),
            args: vec![format!("--profile-directory={dir}")],
        })
        .collect();
    out.sort_by(|a, b| (a.key != "Default", &a.key).cmp(&(b.key != "Default", &b.key)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("urlmenu-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn paths(home: &Path) -> Paths {
        Paths {
            home: home.to_path_buf(),
            config_home: home.join(".config"),
            data_home: home.join(".local/share"),
            state_home: home.join(".local/state"),
            data_dirs: Vec::new(),
            path_dirs: Vec::new(),
        }
    }

    fn write(path: PathBuf, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn firefox_profiles_split_by_install_dir() {
        let home = tmp("ff");
        let root = home.join(".mozilla/firefox");
        write(
            root.join("profiles.ini"),
            "[Profile0]\nName=default-release\nIsRelative=1\nPath=a.release\n\
             [Profile1]\nName=dev-edition-default\nIsRelative=1\nPath=b.dev\n\
             [Profile2]\nName=work\nIsRelative=1\nPath=c.work\n\
             [Profile3]\nName=never-started\nIsRelative=1\nPath=d.new\n",
        );
        write(
            root.join("a.release/compatibility.ini"),
            "[Compatibility]\nLastPlatformDir=/opt/ff\n",
        );
        write(
            root.join("b.dev/compatibility.ini"),
            "[Compatibility]\nLastPlatformDir=/opt/ffdev\n",
        );
        write(
            root.join("c.work/compatibility.ini"),
            "[Compatibility]\nLastPlatformDir=/opt/ffdev\n",
        );
        let p = paths(&home);

        let dev = detect(&p, "/opt/ffdev/firefox");
        let names: Vec<_> = dev.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["dev-edition-default", "work"]);
        assert_eq!(dev[1].args, ["-P", "work"]);

        assert!(
            detect(&p, "/opt/ff/firefox").is_empty(),
            "single profile → no entries"
        );
    }

    #[test]
    fn chromium_profiles_default_first() {
        let home = tmp("chromium");
        write(
            home.join(".config/chromium/Local State"),
            r#"{"profile":{"info_cache":{"Profile 1":{"name":"Chill"},"Default":{"name":"Travail"}}}}"#,
        );
        let got = detect(&paths(&home), "chromium");
        assert_eq!(got[0].name, "Travail");
        assert_eq!(got[0].args, ["--profile-directory=Default"]);
        assert_eq!(got[1].key, "Profile 1");
    }

    #[test]
    fn private_flags() {
        assert_eq!(
            private_flag("/usr/lib/librewolf/librewolf"),
            Some("--private-window")
        );
        assert_eq!(private_flag("brave"), Some("--incognito"));
        assert_eq!(private_flag("qutebrowser"), None);
    }
}
