//! fuzzel: icons, index output, preselection, custom keys and a message line.

use super::command;
use crate::config::{Config, Shortcuts};
use crate::launcher::Launcher;
use crate::launcher::dmenu::{Dmenu, KeyBindings};
use crate::locale::Messages;

pub fn build(cfg: &Config, msg: &Messages) -> Box<dyn Launcher> {
    let prompt = format!("{}: ", msg.prompt);
    Box::new(Dmenu {
        command: command(&["fuzzel", "--dmenu", "--index", "--prompt", &prompt], cfg),
        icons: true,
        index: true,
        select: Some(|i| vec![format!("--select-index={i}")]),
        mesg_flag: Some("--mesg"),
        shortcuts: Some(key_bindings(&cfg.shortcuts)),
    })
}

/// fuzzel's `custom-1` and `custom-2` exit with the codes urlmenu reads.
fn key_bindings(s: &Shortcuts) -> KeyBindings {
    KeyBindings {
        args: vec![
            format!(
                "--override=key-bindings.custom-1={}",
                xkb_modifiers(&s.open_in_private_window)
            ),
            format!(
                "--override=key-bindings.custom-2={}",
                xkb_modifiers(&s.copy_link)
            ),
        ],
        shortcuts: s.clone(),
    }
}

/// rofi key syntax to the XKB modifier names fuzzel expects: `Alt+Return`
/// becomes `Mod1+Return`.
fn xkb_modifiers(key: &str) -> String {
    key.split('+')
        .map(|part| match part {
            "Alt" => "Mod1",
            "Super" | "Win" | "Meta" => "Mod4",
            "Ctrl" => "Control",
            other => other,
        })
        .collect::<Vec<_>>()
        .join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings() {
        assert_eq!(xkb_modifiers("Alt+Return"), "Mod1+Return");
        assert_eq!(xkb_modifiers("Ctrl+Shift+p"), "Control+Shift+p");
        assert_eq!(
            key_bindings(&Shortcuts::default()).args,
            [
                "--override=key-bindings.custom-1=Mod1+Return",
                "--override=key-bindings.custom-2=Control+Shift+c",
            ]
        );
    }
}
