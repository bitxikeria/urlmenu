use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::paths::Paths;

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Preset (`rofi`, `fuzzel`, `wofi`, `dmenu`) or `{ command = [...] }` for
    /// any dmenu-compatible launcher. Unset: the first preset found in `PATH`.
    pub launcher: Option<LauncherConfig>,
    /// Appended to the launcher command, e.g. `["-theme", "x.rasi"]`.
    pub launcher_args: Vec<String>,
    /// Lists private variants as extra rows in launchers without custom keys
    /// (wofi, dmenu, custom commands).
    pub private_entries: bool,
    pub shortcuts: Shortcuts,
    /// Leaves out browser icons in rofi and fuzzel, the launchers that show them.
    pub hide_icons: bool,
    /// Offers to copy the link: a shortcut in rofi and fuzzel, a row elsewhere.
    pub clipboard_entry: bool,
    /// Target id (or browser id) suggested first when the site has no history.
    pub favorite_browser: Option<String>,
    /// Score picks per site and suggest the best one first.
    pub remember: bool,
    pub clean: bool,
    /// Extra query keys to strip; a trailing `*` matches a prefix.
    pub extra_params: Vec<String>,
    /// Desktop entry ids left out of detection.
    pub excluded_browsers: Vec<String>,
    #[serde(rename = "browser")]
    pub browsers: Vec<BrowserConfig>,
    #[serde(rename = "rule")]
    pub rules: Vec<RuleConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            launcher: None,
            launcher_args: Vec::new(),
            private_entries: true,
            shortcuts: Shortcuts::default(),
            hide_icons: false,
            clipboard_entry: true,
            favorite_browser: None,
            remember: true,
            clean: true,
            extra_params: Vec::new(),
            excluded_browsers: Vec::new(),
            browsers: Vec::new(),
            rules: Vec::new(),
        }
    }
}

/// Keys in rofi syntax (`Alt+Return`, `Control+p`), for launchers with custom keys.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Shortcuts {
    /// Opens the highlighted browser in a private window.
    pub open_in_private_window: String,
    /// Copies the link to the clipboard.
    pub copy_link: String,
    /// Shows the shortcut reminder under the URL.
    pub show_hints: bool,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            open_in_private_window: "Alt+Return".to_string(),
            copy_link: "Control+Shift+c".to_string(),
            show_hints: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum LauncherConfig {
    Preset(String),
    Command { command: Vec<String> },
}

/// Adds a browser, or overrides fields of a detected one with the same `id`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserConfig {
    pub id: String,
    pub name: Option<String>,
    pub icon: Option<String>,
    pub exec: Option<String>,
    pub private_exec: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleConfig {
    /// Glob on the host, case-insensitive.
    pub host: Option<String>,
    /// Regex on the cleaned URL.
    pub url: Option<String>,
    pub browser: String,
    /// Profile display name or directory.
    pub profile: Option<String>,
    #[serde(default)]
    pub private: bool,
}

pub fn path(paths: &Paths) -> PathBuf {
    std::env::var_os("URLMENU_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| paths.config_home.join("urlmenu/config.toml"))
}

pub fn load(path: &Path) -> Result<Config, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_example() {
        let cfg: Config = toml::from_str(
            r#"
            launcher = { command = ["bemenu", "-i"] }
            launcher_args = ["-l", "10"]

            private_entries = false
            excluded_browsers = ["nbrowser"]

            [shortcuts]
            open_in_private_window = "Control+p"

            [[browser]]
            id = "chromium"
            private_exec = "chromium --incognito %U"

            [[rule]]
            host = "*.mobicoop.io"
            browser = "firefox-developer-edition"
            profile = "work"

            [[rule]]
            url = '^https://meet\.google\.com/'
            browser = "chromium"
            private = true
            "#,
        )
        .unwrap();
        assert_eq!(
            cfg.launcher,
            Some(LauncherConfig::Command {
                command: vec!["bemenu".into(), "-i".into()]
            })
        );
        assert_eq!(cfg.launcher_args, ["-l", "10"]);
        assert_eq!(cfg.shortcuts.open_in_private_window, "Control+p");
        let preset: Config = toml::from_str(r#"launcher = "rofi""#).unwrap();
        assert_eq!(preset.launcher, Some(LauncherConfig::Preset("rofi".into())));
        assert!(!cfg.private_entries);
        assert!(cfg.clean, "unset keys keep defaults");
        assert_eq!(cfg.browsers[0].id, "chromium");
        assert_eq!(cfg.rules.len(), 2);
        assert!(cfg.rules[1].private);
    }

    #[test]
    fn sample_matches_defaults() {
        let sample: Config = toml::from_str(include_str!("../config.sample.toml")).unwrap();
        assert_eq!(format!("{sample:?}"), format!("{:?}", Config::default()));
    }

    #[test]
    fn rejects_unknown_keys() {
        assert!(toml::from_str::<Config>("lancher = []").is_err());
    }
}
