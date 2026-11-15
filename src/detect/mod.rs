pub mod desktop;
pub mod profiles;

use crate::config::Config;
use crate::paths::Paths;

#[derive(Debug, Clone, PartialEq)]
pub struct Browser {
    /// Desktop entry id, without `.desktop`.
    pub id: String,
    pub name: String,
    /// Icon theme name or absolute path, from `Icon=`.
    pub icon: Option<String>,
    pub exec: Vec<String>,
    pub private_exec: Option<Vec<String>>,
    pub profiles: Vec<Profile>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    pub name: String,
    /// Stable identifier: Firefox profile name or Chromium profile directory.
    pub key: String,
    /// Inserted right after the program in the argv.
    pub args: Vec<String>,
}

/// Detects browsers from desktop entries, applies config overrides and attaches profiles.
pub fn browsers(paths: &Paths, cfg: &Config, languages: &[String]) -> Vec<Browser> {
    let mut found = desktop::scan(paths, &cfg.excluded_browsers, languages);
    for over in &cfg.browsers {
        if let Err(e) = desktop::apply_override(&mut found, over) {
            eprintln!("urlmenu: browser '{}': {e}", over.id);
        }
    }
    for browser in &mut found {
        if browser.private_exec.is_none() {
            browser.private_exec = profiles::private_flag(&browser.exec[0]).map(|flag| {
                let mut argv = browser.exec.clone();
                argv.insert(1, flag.to_string());
                argv
            });
        }
        browser.profiles = profiles::detect(paths, &browser.exec[0]);
    }
    found.sort_by_key(|a| a.name.to_lowercase());
    found
}
