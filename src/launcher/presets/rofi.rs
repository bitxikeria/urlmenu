//! rofi in script mode: icons, preselection, custom keys and a message line.

use crate::config::Config;
use crate::launcher::Launcher;
use crate::launcher::rofi::Rofi;

pub fn build(cfg: &Config) -> Box<dyn Launcher> {
    Box::new(Rofi {
        args: cfg.launcher_args.clone(),
        shortcuts: cfg.shortcuts.clone(),
    })
}
