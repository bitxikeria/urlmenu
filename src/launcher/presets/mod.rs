//! Launcher presets: the command line of each known launcher and the
//! features urlmenu uses in it. `launcher_args` is appended to every one.

mod bemenu;
mod dmenu;
mod fuzzel;
mod rofi;
mod tofi;
mod walker;
mod wofi;

use super::Launcher;
use crate::config::Config;
use crate::locale::Messages;

/// Presets tried in this order when `launcher` is unset.
pub const AUTO_ORDER: &[&str] = &[
    "rofi", "fuzzel", "wofi", "tofi", "walker", "bemenu", "dmenu",
];

pub fn build(name: &str, cfg: &Config, msg: &Messages) -> Result<Box<dyn Launcher>, String> {
    Ok(match name {
        "rofi" => rofi::build(cfg),
        "fuzzel" => fuzzel::build(cfg, msg),
        "wofi" => wofi::build(cfg, msg),
        "tofi" => tofi::build(cfg, msg),
        "walker" => walker::build(cfg, msg),
        "bemenu" => bemenu::build(cfg, msg),
        "dmenu" => dmenu::build(cfg, msg),
        other => {
            return Err(format!(
                "unknown launcher preset {other:?}; use {} or {{ command = [...] }}",
                AUTO_ORDER.join(", ")
            ));
        }
    })
}

/// The preset's base command followed by `launcher_args`.
fn command(base: &[&str], cfg: &Config) -> Vec<String> {
    base.iter()
        .map(|s| s.to_string())
        .chain(cfg.launcher_args.iter().cloned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_builds() {
        let cfg = Config::default();
        for name in AUTO_ORDER {
            assert!(build(name, &cfg, &crate::locale::EN).is_ok(), "{name}");
        }
        assert!(build("generic", &cfg, &crate::locale::EN).is_err());
    }

    #[test]
    fn launcher_args_come_last() {
        let cfg = Config {
            launcher_args: vec!["-x".into()],
            ..Config::default()
        };
        assert_eq!(command(&["menu", "-i"], &cfg), ["menu", "-i", "-x"]);
    }
}
