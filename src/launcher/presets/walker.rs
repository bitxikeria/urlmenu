//! walker in dmenu mode: rows only. Its `--index` output is a score, not a
//! row index, so the choice is read back by label.

use super::command;
use crate::config::Config;
use crate::launcher::Launcher;
use crate::launcher::dmenu::Dmenu;
use crate::locale::Messages;

pub fn build(cfg: &Config, msg: &Messages) -> Box<dyn Launcher> {
    Box::new(Dmenu::generic(command(
        &["walker", "--dmenu", "--placeholder", msg.prompt],
        cfg,
    )))
}
