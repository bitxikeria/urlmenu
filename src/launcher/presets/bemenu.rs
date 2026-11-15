//! bemenu: vertical list with preselection.

use super::command;
use crate::config::Config;
use crate::launcher::Launcher;
use crate::launcher::dmenu::Dmenu;
use crate::locale::Messages;

pub fn build(cfg: &Config, msg: &Messages) -> Box<dyn Launcher> {
    Box::new(Dmenu {
        select: Some(|i| vec!["--index".into(), i.to_string()]),
        ..Dmenu::generic(command(
            &[
                "bemenu",
                "--ignorecase",
                "--list",
                "10",
                "--prompt",
                msg.prompt,
            ],
            cfg,
        ))
    })
}
