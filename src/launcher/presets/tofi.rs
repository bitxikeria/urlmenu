//! tofi: rows only.

use super::command;
use crate::config::Config;
use crate::launcher::Launcher;
use crate::launcher::dmenu::Dmenu;
use crate::locale::Messages;

pub fn build(cfg: &Config, msg: &Messages) -> Box<dyn Launcher> {
    let prompt = format!("{}: ", msg.prompt);
    Box::new(Dmenu::generic(command(
        &["tofi", "--prompt-text", &prompt],
        cfg,
    )))
}
