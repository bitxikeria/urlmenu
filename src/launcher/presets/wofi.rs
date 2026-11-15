//! wofi: rows only.

use super::command;
use crate::config::Config;
use crate::launcher::Launcher;
use crate::launcher::dmenu::Dmenu;
use crate::locale::Messages;

pub fn build(cfg: &Config, msg: &Messages) -> Box<dyn Launcher> {
    Box::new(Dmenu::generic(command(
        &["wofi", "--dmenu", "-i", "-p", msg.prompt],
        cfg,
    )))
}
