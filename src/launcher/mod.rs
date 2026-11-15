//! Launcher adapters. Each one turns a [`Menu`] into its own protocol and
//! drops what it cannot show; the rest of urlmenu never sees launcher syntax.

pub mod dmenu;
mod presets;
pub mod rofi;

use crate::config::{Config, LauncherConfig, Shortcuts};
use crate::locale::Messages;
use crate::menu::{Choice, Menu};
use crate::paths::Paths;

/// User preferences, applied where the launcher supports them.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub icons: bool,
    pub private_entries: bool,
    pub msg: &'static Messages,
}

impl Options {
    pub fn new(cfg: &Config, msg: &'static Messages) -> Self {
        Self {
            msg,
            icons: !cfg.hide_icons,
            private_entries: cfg.private_entries,
        }
    }
}

pub trait Launcher {
    fn pick(&self, menu: &Menu, opts: Options) -> Result<Choice, String>;
}

pub const COPY_ICON: &str = "edit-copy";
/// Exit codes of rofi `kb-custom-1`/`kb-custom-2` and fuzzel `custom-1`/`custom-2`.
pub const PRIVATE_EXIT: i32 = 10;
pub const COPY_EXIT: i32 = 11;

pub fn from_config(
    cfg: &Config,
    paths: &Paths,
    msg: &Messages,
) -> Result<Box<dyn Launcher>, String> {
    let extra = &cfg.launcher_args;
    match &cfg.launcher {
        Some(LauncherConfig::Preset(name)) => presets::build(name, cfg, msg),
        Some(LauncherConfig::Command { command }) => {
            if command.is_empty() {
                return Err("`launcher.command` is empty".into());
            }
            Ok(Box::new(dmenu::Dmenu::generic(
                command.iter().chain(extra).cloned().collect(),
            )))
        }
        None => presets::AUTO_ORDER
            .iter()
            .find(|name| paths.which(name).is_some())
            .map(|name| presets::build(name, cfg, msg))
            .unwrap_or_else(|| {
                Err(format!(
                    "no launcher found: install one of {} or set `launcher`",
                    presets::AUTO_ORDER.join(", ")
                ))
            }),
    }
}

/// Key as shown to people: `Control+c` becomes `Ctrl+C`, `Return` becomes `Enter`.
pub(crate) fn display_key(key: &str) -> String {
    key.split('+')
        .map(|part| match part {
            "Control" => "Ctrl".to_string(),
            "Return" => "Enter".to_string(),
            p if p.chars().count() == 1 => p.to_uppercase(),
            p => p.to_string(),
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// One-line reminder of the shortcuts that apply to this menu.
pub fn hints(msg: &Messages, menu: &Menu, shortcuts: &Shortcuts) -> Option<String> {
    let mut out = Vec::new();
    if !shortcuts.show_hints {
        return None;
    }
    if menu.has_private() {
        out.push(
            msg.private_hint
                .replace("{key}", &display_key(&shortcuts.open_in_private_window)),
        );
    }
    if menu.copy {
        out.push(
            msg.copy_hint
                .replace("{key}", &display_key(&shortcuts.copy_link)),
        );
    }
    if out.is_empty() {
        return None;
    }
    Some(out.join("  ·  "))
}

/// Pango markup escaping, for text rofi renders as markup.
pub fn escape_markup(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::tests::sample;

    #[test]
    fn displayed_keys() {
        assert_eq!(display_key("Alt+Return"), "Alt+Enter");
        assert_eq!(display_key("Control+c"), "Ctrl+C");
    }

    #[test]
    fn hint_lines() {
        let s = Shortcuts::default();
        let mut m = sample();
        assert_eq!(
            hints(&crate::locale::EN, &m, &s).unwrap(),
            "Alt+Enter: open in a private window  ·  Ctrl+Shift+C: copy the link"
        );
        let hidden = Shortcuts {
            show_hints: false,
            ..Shortcuts::default()
        };
        assert_eq!(hints(&crate::locale::EN, &m, &hidden), None);
        m.copy = false;
        for item in &mut m.items {
            item.private = None;
        }
        assert_eq!(hints(&crate::locale::EN, &m, &s), None);
    }
}
