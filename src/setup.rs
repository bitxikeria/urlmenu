use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::paths::Paths;

const MIME_TYPES: &[&str] = &[
    "x-scheme-handler/http",
    "x-scheme-handler/https",
    "text/html",
];

fn desktop_entry(exe: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=urlmenu\n\
         Comment=Pick a browser for each link\n\
         Exec={} %u\n\
         Icon=web-browser\n\
         Terminal=false\n\
         Categories=Network;WebBrowser;\n\
         MimeType={};\n",
        exec_program(exe),
        MIME_TYPES.join(";")
    )
}

/// Quotes the program only when the spec requires it: xdg-mime resolves the
/// handler from the raw first word of `Exec`, quotes included, and skips the
/// entry when that lookup fails.
fn exec_program(exe: &str) -> String {
    let needs_quotes = exe
        .chars()
        .any(|c| c.is_whitespace() || "\"'\\><~|&;$*?#()`=%".contains(c));
    if !needs_quotes {
        return exe.to_string();
    }
    // Quoted-argument escapes, then string-value escaping of the backslashes.
    let escaped = exe
        .replace('\\', r"\\\\")
        .replace('"', r#"\\""#)
        .replace('$', r"\\$")
        .replace('`', r"\\`");
    format!("\"{escaped}\"")
}

/// Parts of `urlmenu setup`; none selected means all of them.
#[derive(Debug, Clone, Copy)]
pub struct Parts {
    pub handler: bool,
    pub completion: bool,
}

impl Parts {
    fn resolve(self) -> Self {
        if self.handler || self.completion {
            self
        } else {
            Self {
                handler: true,
                completion: true,
            }
        }
    }
}

/// Shell completion script, in a directory the shell loads by itself.
struct Completion {
    shell: &'static str,
    file: PathBuf,
    content: &'static str,
}

/// One script per shell; each asks urlmenu for its registration code at
/// shell start, so it follows the installed binary.
fn all_completions(paths: &Paths) -> [Completion; 3] {
    [
        Completion {
            shell: "fish",
            file: paths.config_home.join("fish/completions/urlmenu.fish"),
            content: "COMPLETE=fish urlmenu | source\n",
        },
        Completion {
            shell: "bash",
            file: paths.data_home.join("bash-completion/completions/urlmenu"),
            content: "source <(COMPLETE=bash urlmenu)\n",
        },
        Completion {
            shell: "zsh",
            file: paths.data_home.join("zsh/site-functions/_urlmenu"),
            content: "#compdef urlmenu\n\
                      source <(COMPLETE=zsh urlmenu)\n\
                      _clap_dynamic_completer_urlmenu \"$@\"\n",
        },
    ]
}

/// Scripts for the shells found in `PATH`.
fn completions(paths: &Paths) -> Vec<Completion> {
    all_completions(paths)
        .into_iter()
        .filter(|c| paths.which(c.shell).is_some())
        .collect()
}

fn zsh_note(paths: &Paths) -> String {
    format!(
        "zsh: add to ~/.zshrc, before compinit: fpath=({} $fpath)",
        paths.data_home.join("zsh/site-functions").display()
    )
}

fn desktop_file(paths: &Paths) -> PathBuf {
    paths.data_home.join("applications").join(DESKTOP_ID)
}

/// Desktop entry installed by a distribution package, if any.
fn system_desktop_file(paths: &Paths) -> Option<PathBuf> {
    paths
        .data_dirs
        .iter()
        .map(|d| d.join("applications").join(DESKTOP_ID))
        .find(|f| f.is_file())
}

/// Handlers replaced by `setup`, restored by `setup --undo`.
fn previous_file(paths: &Paths) -> PathBuf {
    paths.state_home.join("urlmenu/previous-handlers.json")
}

const DESKTOP_ID: &str = "urlmenu.desktop";

fn write(file: &Path, content: &str) -> Result<(), String> {
    std::fs::create_dir_all(file.parent().ok_or("bad path")?)
        .map_err(|e| format!("{}: {e}", file.display()))?;
    std::fs::write(file, content).map_err(|e| format!("{}: {e}", file.display()))?;
    println!("wrote {}", file.display());
    Ok(())
}

fn remove(file: &Path, print: bool) -> Result<(), String> {
    if !file.exists() {
        return Ok(());
    }
    if print {
        println!("rm {}", file.display());
        return Ok(());
    }
    std::fs::remove_file(file).map_err(|e| format!("{}: {e}", file.display()))?;
    println!("removed {}", file.display());
    Ok(())
}

fn xdg_mime(args: &[&str], print: bool) -> Result<(), String> {
    if print {
        println!("xdg-mime {}", args.join(" "));
        return Ok(());
    }
    let status = Command::new("xdg-mime")
        .args(args)
        .status()
        .map_err(|e| format!("cannot run xdg-mime: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("xdg-mime failed: {status}"))
    }
}

/// Current default desktop entry for a MIME type, if any.
fn query_default(mime: &str) -> Option<String> {
    let out = Command::new("xdg-mime")
        .args(["query", "default", mime])
        .output()
        .ok()?;
    let id = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (out.status.success() && !id.is_empty()).then_some(id)
}

pub fn run(paths: &Paths, parts: Parts, undo: bool, print: bool) -> Result<(), String> {
    let parts = parts.resolve();
    if undo {
        if parts.handler {
            undo_handler(paths, print)?;
        }
        if parts.completion {
            for c in all_completions(paths) {
                remove(&c.file, print)?;
            }
        }
        return Ok(());
    }
    if parts.handler {
        setup_handler(paths, print)?;
    }
    if parts.completion {
        let completions = completions(paths);
        for c in &completions {
            if print {
                println!("# {}\n{}", c.file.display(), c.content);
            } else {
                write(&c.file, c.content)?;
            }
        }
        if completions.iter().any(|c| c.shell == "zsh") {
            println!("{}", zsh_note(paths));
        }
    }
    Ok(())
}

fn setup_handler(paths: &Paths, print: bool) -> Result<(), String> {
    let mut args = vec!["default", DESKTOP_ID];
    args.extend(MIME_TYPES);
    let user_entry = match system_desktop_file(paths) {
        Some(system) => {
            println!("using {}", system.display());
            None
        }
        None => {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let exe = exe.to_str().ok_or("executable path is not UTF-8")?;
            Some((desktop_file(paths), desktop_entry(exe)))
        }
    };
    if print {
        if let Some((file, entry)) = &user_entry {
            println!("# {}\n{entry}", file.display());
        }
        return xdg_mime(&args, true);
    }
    let previous = previous_file(paths);
    let saved = merge_previous(load_previous(&previous), &current_defaults());
    let text = serde_json::to_string_pretty(&saved).map_err(|e| e.to_string())?;
    write(&previous, &text)?;
    if let Some((file, entry)) = &user_entry {
        write(file, entry)?;
    }
    xdg_mime(&args, false)?;
    println!(
        "urlmenu is now the default handler for {}",
        MIME_TYPES.join(", ")
    );
    Ok(())
}

fn undo_handler(paths: &Paths, print: bool) -> Result<(), String> {
    let previous = previous_file(paths);
    let (plan, missing) = restore_plan(&load_previous(&previous), &current_defaults());
    for (id, mimes) in &plan {
        let mut args = vec!["default", id.as_str()];
        args.extend(mimes.iter().map(String::as_str));
        xdg_mime(&args, print)?;
    }
    if !missing.is_empty() {
        println!(
            "no previous handler recorded for {}; set one with: xdg-mime default <browser>.desktop {}",
            missing.join(", "),
            missing.join(" ")
        );
    }
    remove(&desktop_file(paths), print)?;
    remove(&previous, print)
}

type Handlers = BTreeMap<String, String>;

fn load_previous(path: &Path) -> Handlers {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Default desktop entry of each MIME type urlmenu handles.
fn current_defaults() -> BTreeMap<String, Option<String>> {
    MIME_TYPES
        .iter()
        .map(|m| (m.to_string(), query_default(m)))
        .collect()
}

/// Records each handler that is not urlmenu, so the latest one before
/// urlmenu is the one restored.
fn merge_previous(mut saved: Handlers, current: &BTreeMap<String, Option<String>>) -> Handlers {
    for (mime, id) in current {
        if let Some(id) = id.as_deref().filter(|id| *id != DESKTOP_ID) {
            saved.insert(mime.clone(), id.to_string());
        }
    }
    saved
}

/// MIME types still handled by urlmenu, grouped by the handler to restore,
/// and those with none recorded. Types the user moved to another handler
/// since setup are left alone.
fn restore_plan(
    saved: &Handlers,
    current: &BTreeMap<String, Option<String>>,
) -> (BTreeMap<String, Vec<String>>, Vec<String>) {
    let mut plan: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut missing = Vec::new();
    for (mime, id) in current {
        if id.as_deref() != Some(DESKTOP_ID) {
            continue;
        }
        match saved.get(mime) {
            Some(prev) => plan.entry(prev.clone()).or_default().push(mime.clone()),
            None => missing.push(mime.clone()),
        }
    }
    (plan, missing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::desktop::parse_browser;

    #[test]
    fn entry_round_trips_through_parser() {
        for exe in ["/home/u/.cargo/bin/urlmenu", "/home/u/My Bin/link$pick"] {
            let b = parse_browser("x", &desktop_entry(exe), &[])
                .unwrap()
                .unwrap();
            assert_eq!(b.exec, [exe, "%u"]);
        }
    }

    #[test]
    fn completions_for_shells_in_path() {
        let dir = std::env::temp_dir().join(format!("urlmenu-setup-{}", std::process::id()));
        let bin = dir.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("zsh"), "").unwrap();
        let paths = Paths {
            home: dir.clone(),
            config_home: dir.join("config"),
            data_home: dir.join("data"),
            state_home: dir.join("state"),
            data_dirs: vec![],
            path_dirs: vec![bin],
        };
        let found = completions(&paths);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, dir.join("data/zsh/site-functions/_urlmenu"));
        assert_eq!(
            found[0].content,
            "#compdef urlmenu\nsource <(COMPLETE=zsh urlmenu)\n_clap_dynamic_completer_urlmenu \"$@\"\n"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn packaging_file(name: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("packaging")
            .join(name);
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    #[test]
    fn packaged_desktop_entry_matches_setup() {
        assert_eq!(packaging_file("urlmenu.desktop"), desktop_entry("urlmenu"));
    }

    #[test]
    fn packaged_completions_match_setup() {
        let paths = Paths {
            home: PathBuf::new(),
            config_home: PathBuf::new(),
            data_home: PathBuf::new(),
            state_home: PathBuf::new(),
            data_dirs: vec![],
            path_dirs: vec![],
        };
        for c in all_completions(&paths) {
            let name = c.file.file_name().unwrap().to_str().unwrap();
            let name = if c.shell == "bash" {
                "urlmenu.bash"
            } else {
                name
            };
            assert_eq!(packaging_file(&format!("completions/{name}")), c.content);
        }
    }

    #[test]
    fn system_entry_is_found_in_data_dirs() {
        let dir = std::env::temp_dir().join(format!("urlmenu-system-{}", std::process::id()));
        let apps = dir.join("share/applications");
        std::fs::create_dir_all(&apps).unwrap();
        let mut paths = Paths {
            home: dir.clone(),
            config_home: dir.join("config"),
            data_home: dir.join("data"),
            state_home: dir.join("state"),
            data_dirs: vec![dir.join("empty"), dir.join("share")],
            path_dirs: vec![],
        };
        assert_eq!(system_desktop_file(&paths), None);
        std::fs::write(apps.join(DESKTOP_ID), "").unwrap();
        assert_eq!(system_desktop_file(&paths), Some(apps.join(DESKTOP_ID)));
        paths.data_dirs.clear();
        assert_eq!(system_desktop_file(&paths), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn map<V: Clone>(pairs: &[(&str, V)]) -> BTreeMap<String, V> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn latest_handler_before_urlmenu_is_recorded() {
        let saved = map(&[("https", "firefox.desktop".to_string())]);
        let current = map(&[
            ("https", Some("brave.desktop".to_string())),
            ("http", Some(DESKTOP_ID.to_string())),
            ("html", None),
        ]);
        assert_eq!(
            merge_previous(saved, &current),
            map(&[("https", "brave.desktop".to_string())])
        );
    }

    #[test]
    fn undo_restores_only_types_still_on_urlmenu() {
        let saved = map(&[
            ("https", "firefox.desktop".to_string()),
            ("http", "firefox.desktop".to_string()),
        ]);
        let current = map(&[
            ("https", Some(DESKTOP_ID.to_string())),
            ("http", Some("brave.desktop".to_string())),
            ("html", Some(DESKTOP_ID.to_string())),
        ]);
        let (plan, missing) = restore_plan(&saved, &current);
        assert_eq!(plan, map(&[("firefox.desktop", vec!["https".to_string()])]));
        assert_eq!(missing, ["html"]);
        let (plan, missing) = restore_plan(&Handlers::new(), &map(&[("https", None::<String>)]));
        assert!(
            plan.is_empty() && missing.is_empty(),
            "nothing left to undo"
        );
    }

    #[test]
    fn no_part_flag_means_all() {
        let parts = |handler, completion| Parts {
            handler,
            completion,
        };
        let cases = [
            (parts(false, false), (true, true)),
            (parts(true, false), (true, false)),
            (parts(false, true), (false, true)),
            (parts(true, true), (true, true)),
        ];
        for (input, expected) in cases {
            let r = input.resolve();
            assert_eq!((r.handler, r.completion), expected, "{input:?}");
        }
    }

    #[test]
    fn plain_path_is_unquoted() {
        assert!(desktop_entry("/usr/bin/urlmenu").contains("\nExec=/usr/bin/urlmenu %u\n"));
    }
}
