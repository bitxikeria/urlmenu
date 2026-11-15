use std::collections::HashSet;
use std::path::Path;

use super::Browser;
use crate::config::BrowserConfig;
use crate::exec;
use crate::ini::Ini;
use crate::paths::Paths;

const SELF_ID: &str = "urlmenu";
const MAIN: &str = "Desktop Entry";

/// Collects web browsers: application entries handling `x-scheme-handler/https`
/// in the `WebBrowser` category. Earlier directories shadow later ones by id.
pub fn scan(paths: &Paths, excluded: &[String], languages: &[String]) -> Vec<Browser> {
    let mut seen = HashSet::new();
    let mut browsers = Vec::new();
    for dir in paths.application_dirs() {
        let mut entries = Vec::new();
        collect(&dir, &dir, &mut entries);
        entries.sort();
        for (id, file) in entries {
            if !seen.insert(id.clone()) {
                continue;
            }
            if id == SELF_ID || excluded.contains(&id) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            match parse_browser(&id, &text, languages) {
                Ok(Some(b)) => browsers.push(b),
                Ok(None) => {}
                Err(e) => eprintln!("urlmenu: {}: {e}", file.display()),
            }
        }
    }
    browsers
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, std::path::PathBuf)>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, out);
        } else if path.extension().is_some_and(|e| e == "desktop")
            && let Ok(rel) = path.strip_prefix(root)
        {
            let id = rel.with_extension("").to_string_lossy().replace('/', "-");
            out.push((id, path));
        }
    }
}

fn list_contains(value: Option<&str>, item: &str) -> bool {
    value.is_some_and(|v| v.split(';').any(|x| x == item))
}

/// Returns `Ok(None)` for entries that are not visible web browsers.
pub fn parse_browser(
    id: &str,
    text: &str,
    languages: &[String],
) -> Result<Option<Browser>, String> {
    let ini = Ini::parse(text);
    let get = |key| ini.get(MAIN, key);
    let is_true = |key| get(key) == Some("true");
    if get("Type") != Some("Application")
        || is_true("Hidden")
        || is_true("NoDisplay")
        || !list_contains(get("MimeType"), "x-scheme-handler/https")
        || !list_contains(get("Categories"), "WebBrowser")
    {
        return Ok(None);
    }
    let exec_value = get("Exec").ok_or("missing Exec")?;
    let exec = exec::split(&exec::unescape_value(exec_value))?;
    let private_exec = get("Actions")
        .into_iter()
        .flat_map(|a| a.split(';'))
        .filter(|a| {
            let a = a.to_ascii_lowercase();
            a.contains("private") || a.contains("incognito")
        })
        .find_map(|action| ini.get(&format!("Desktop Action {action}"), "Exec"))
        .map(|v| exec::split(&exec::unescape_value(v)))
        .transpose()?;
    Ok(Some(Browser {
        id: id.to_string(),
        name: languages
            .iter()
            .find_map(|l| ini.get(MAIN, &format!("Name[{l}]")))
            .or_else(|| get("Name"))
            .unwrap_or(id)
            .to_string(),
        icon: get("Icon").map(String::from),
        exec,
        private_exec,
        profiles: Vec::new(),
    }))
}

pub fn apply_override(browsers: &mut Vec<Browser>, over: &BrowserConfig) -> Result<(), String> {
    let parse = |s: &str| exec::split(s);
    let private_exec = over.private_exec.as_deref().map(parse).transpose()?;
    if let Some(b) = browsers.iter_mut().find(|b| b.id == over.id) {
        if let Some(name) = &over.name {
            b.name = name.clone();
        }
        if let Some(icon) = &over.icon {
            b.icon = Some(icon.clone());
        }
        if let Some(e) = &over.exec {
            b.exec = parse(e)?;
        }
        if private_exec.is_some() {
            b.private_exec = private_exec;
        }
        return Ok(());
    }
    let exec = over
        .exec
        .as_deref()
        .ok_or("not detected; `exec` is required to add it")?;
    browsers.push(Browser {
        id: over.id.clone(),
        name: over.name.clone().unwrap_or_else(|| over.id.clone()),
        icon: over.icon.clone(),
        exec: parse(exec)?,
        private_exec,
        profiles: Vec::new(),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIREFOX: &str = "[Desktop Entry]
Name=Firefox Developer Edition
Name[fr]=Firefox Édition Développeur
Icon=firefox-developer-edition
Exec=/usr/lib/firefox-developer-edition/firefox %u
Type=Application
Categories=Network;WebBrowser;
MimeType=text/html;x-scheme-handler/http;x-scheme-handler/https;
Actions=new-window;new-private-window;

[Desktop Action new-window]
Exec=/usr/lib/firefox-developer-edition/firefox --new-window %u

[Desktop Action new-private-window]
Exec=/usr/lib/firefox-developer-edition/firefox --private-window %u
";

    #[test]
    fn parses_browser_with_private_action() {
        let fr = ["fr_FR".to_string(), "fr".to_string()];
        let localized = parse_browser("x", FIREFOX, &fr).unwrap().unwrap();
        assert_eq!(localized.name, "Firefox Édition Développeur");
        let b = parse_browser("firefox-developer-edition", FIREFOX, &[])
            .unwrap()
            .unwrap();
        assert_eq!(b.name, "Firefox Developer Edition");
        assert_eq!(b.icon.as_deref(), Some("firefox-developer-edition"));
        assert_eq!(b.exec, ["/usr/lib/firefox-developer-edition/firefox", "%u"]);
        assert_eq!(
            b.private_exec.unwrap(),
            [
                "/usr/lib/firefox-developer-edition/firefox",
                "--private-window",
                "%u"
            ]
        );
    }

    #[test]
    fn skips_non_browsers() {
        let cases = [
            FIREFOX.replace("Categories=Network;WebBrowser;", "Categories=Network;"),
            FIREFOX.replace("x-scheme-handler/https;", ""),
            FIREFOX.replace("Type=Application", "Type=Link"),
            FIREFOX.replace("Type=Application", "Type=Application\nNoDisplay=true"),
        ];
        for text in cases {
            assert_eq!(parse_browser("x", &text, &[]).unwrap(), None);
        }
    }

    #[test]
    fn override_and_add() {
        let mut list = vec![parse_browser("ff", FIREFOX, &[]).unwrap().unwrap()];
        let over = |id: &str, exec: Option<&str>| BrowserConfig {
            id: id.into(),
            name: Some("Renamed".into()),
            icon: None,
            exec: exec.map(String::from),
            private_exec: None,
        };
        apply_override(&mut list, &over("ff", None)).unwrap();
        assert_eq!(list[0].name, "Renamed");
        assert!(list[0].private_exec.is_some(), "untouched fields stay");
        assert!(list[0].icon.is_some(), "untouched fields stay");
        assert!(apply_override(&mut list, &over("new", None)).is_err());
        apply_override(&mut list, &over("new", Some("qutebrowser %u"))).unwrap();
        assert_eq!(list[1].exec, ["qutebrowser", "%u"]);
    }
}
