//! rofi in script mode. rofi calls `urlmenu __rofi` to list rows and again on
//! selection; that child only records the choice in `$URLMENU_RESULT`, and the
//! parent reads it after rofi exits and performs the action. The URL travels
//! through `$URLMENU_URL`, never through a command string.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::{COPY_EXIT, Launcher, Options, PRIVATE_EXIT, escape_markup, hints};
use crate::config::Shortcuts;
use crate::menu::{Choice, Item, Menu};

pub const URL_ENV: &str = "URLMENU_URL";
pub const RESULT_ENV: &str = "URLMENU_RESULT";
const MODE: &str = "browsers";
/// rofi `ROFI_RETV` for an accepted entry; custom keys use the exit codes.
const RETV_SELECTED: i32 = 1;

pub struct Rofi {
    pub args: Vec<String>,
    pub shortcuts: Shortcuts,
}

impl Launcher for Rofi {
    fn pick(&self, menu: &Menu, opts: Options) -> Result<Choice, String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe = exe.to_str().ok_or("executable path is not UTF-8")?;
        if exe.contains(',') {
            return Err(format!(
                "rofi -modi cannot run a path containing ',': {exe}"
            ));
        }
        let modi = format!("{MODE}:{} __rofi", shell_quote(exe));

        let result = result_path();
        let _ = std::fs::remove_file(&result);
        let mut cmd = Command::new("rofi");
        cmd.args(["-show", MODE, "-modi", &modi])
            .args([&format!("-display-{MODE}"), opts.msg.browsers])
            .args(["-kb-custom-1", &self.shortcuts.open_in_private_window]);
        if menu.copy {
            cmd.args(["-kb-custom-2", &self.shortcuts.copy_link]);
        }
        if opts.icons {
            cmd.arg("-show-icons");
        }
        // `-selected-row` is the only initial-selection option rofi honours
        // outside dmenu mode.
        if let Some(index) = suggested_row(menu) {
            cmd.args(["-selected-row", &index.to_string()]);
        }
        cmd.args(&self.args)
            .env(RESULT_ENV, &result)
            .stdin(Stdio::null());
        match &menu.url {
            Some(url) => cmd.env(URL_ENV, url),
            None => cmd.env_remove(URL_ENV),
        };
        cmd.status().map_err(|e| format!("cannot run rofi: {e}"))?;

        let choice = std::fs::read_to_string(&result)
            .ok()
            .map_or(Choice::Cancel, |s| parse_result(&s));
        let _ = std::fs::remove_file(&result);
        Ok(choice)
    }
}

fn result_path() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("urlmenu-{}.choice", std::process::id()))
}

/// Quoting understood by `g_shell_parse_argv`, which rofi uses for `-modi` commands.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

pub enum Script {
    Rows(String),
    Done(Choice),
}

/// One invocation of the script mode, as a pure function of rofi's variables.
pub fn script(
    menu: &Menu,
    opts: Options,
    shortcuts: &Shortcuts,
    retv: i32,
    info: Option<&str>,
) -> Script {
    match (retv, info) {
        (COPY_EXIT, _) if menu.copy => return Script::Done(Choice::Copy),
        (PRIVATE_EXIT, Some(id)) => return Script::Done(Choice::Open(menu.private_of(id))),
        (RETV_SELECTED, Some(id)) => return Script::Done(Choice::Open(id.to_string())),
        _ => {}
    }
    let mut out = vec![
        format!("\0prompt\x1f{}", opts.msg.prompt),
        "\0no-custom\x1ftrue".to_string(),
        "\0use-hot-keys\x1ftrue".to_string(),
    ];
    let message = message(menu, opts, shortcuts);
    if !message.is_empty() {
        out.push(format!("\0message\x1f{message}"));
    }
    out.extend(menu.items.iter().map(|i| row(i, opts)));
    let mut text = out.join("\n");
    text.push('\n');
    Script::Rows(text)
}

/// URL, then the shortcut reminder dimmed. Pango markup.
fn message(menu: &Menu, opts: Options, shortcuts: &Shortcuts) -> String {
    let mut lines: Vec<String> = menu.url.iter().map(|u| escape_markup(u)).collect();
    lines.extend(
        hints(opts.msg, menu, shortcuts)
            .map(|h| format!("<span alpha=\"50%\">{}</span>", escape_markup(&h))),
    );
    // A raw `\n` would end the row; the markup entity is decoded by Pango.
    clean(&lines.join("&#10;"))
}

/// Row index of the suggestion, when it is not already the first row.
fn suggested_row(menu: &Menu) -> Option<usize> {
    let wanted = menu.suggested.as_deref()?;
    menu.items
        .iter()
        .position(|i| i.id == wanted)
        .filter(|&i| i > 0)
}

fn row(item: &Item, opts: Options) -> String {
    let mut options = Vec::new();
    if let Some(icon) = item.icon.as_deref().filter(|_| opts.icons) {
        options.push(format!("icon\x1f{}", clean(icon)));
    }
    options.push(format!("info\x1f{}", item.id));
    format!("{}\0{}", clean(&item.label), options.join("\x1f"))
}

/// Drops the separators of rofi's row protocol from free text.
fn clean(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '\0' | '\n' | '\x1f'))
        .collect()
}

pub fn write_result(path: &Path, choice: &Choice) -> Result<(), String> {
    let text = match choice {
        Choice::Open(id) => format!("open\t{id}\n"),
        Choice::Copy => "copy\n".to_string(),
        Choice::Cancel => return Ok(()),
    };
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

fn parse_result(text: &str) -> Choice {
    let text = text.trim_end_matches('\n');
    match text.split_once('\t') {
        Some(("open", id)) if !id.is_empty() => Choice::Open(id.to_string()),
        _ if text == "copy" => Choice::Copy,
        _ => Choice::Cancel,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::tests::sample;

    const ALL: Options = Options {
        icons: true,
        private_entries: true,
        msg: &crate::locale::EN,
    };

    fn rows_text(menu: &Menu, opts: Options) -> String {
        match script(menu, opts, &Shortcuts::default(), 0, None) {
            Script::Rows(r) => r,
            Script::Done(c) => panic!("unexpected choice {c:?}"),
        }
    }

    #[test]
    fn rows_with_hints() {
        assert_eq!(
            rows_text(&sample(), ALL),
            "\0prompt\x1fOpen with\n\0no-custom\x1ftrue\n\0use-hot-keys\x1ftrue\n\
             \0message\x1fhttps://e.com/?a=1&amp;b=2&#10;\
             <span alpha=\"50%\">Alt+Enter: open in a private window  ·  Ctrl+Shift+C: copy the link</span>\n\
             Firefox\0icon\x1ffirefox\x1finfo\x1fff\n\
             qutebrowser\0info\x1fqute\n"
        );
    }

    #[test]
    fn no_message_without_url_or_private() {
        let mut m = sample();
        m.url = None;
        m.copy = false;
        for item in &mut m.items {
            item.private = None;
        }
        let opts = Options {
            icons: false,
            ..ALL
        };
        assert!(!rows_text(&m, opts).contains("message"));
    }

    #[test]
    fn suggestion_row() {
        let mut m = sample();
        m.suggested = Some("qute".into());
        assert_eq!(suggested_row(&m), Some(1));
        m.suggested = Some("ff".into());
        assert_eq!(suggested_row(&m), None, "already first");
    }

    #[test]
    fn selections() {
        let m = sample();
        let s = Shortcuts::default();
        let cases: &[(i32, Option<&str>, Choice)] = &[
            (1, Some("qute"), Choice::Open("qute".into())),
            (10, Some("ff"), Choice::Open("ff+private".into())),
            (10, Some("qute"), Choice::Open("qute".into())),
            (11, Some("qute"), Choice::Copy),
            (11, None, Choice::Copy),
        ];
        for (retv, info, expected) in cases {
            match script(&m, ALL, &s, *retv, *info) {
                Script::Done(c) => assert_eq!(&c, expected, "{retv} {info:?}"),
                Script::Rows(_) => panic!("expected a choice for {retv} {info:?}"),
            }
        }
        assert!(matches!(script(&m, ALL, &s, 1, None), Script::Rows(_)));
    }

    #[test]
    fn result_round_trip() {
        let dir = std::env::temp_dir().join(format!("urlmenu-rofi-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("r");
        for choice in [
            Choice::Open("chromium/Profile 1+private".into()),
            Choice::Copy,
        ] {
            write_result(&path, &choice).unwrap();
            assert_eq!(
                parse_result(&std::fs::read_to_string(&path).unwrap()),
                choice
            );
        }
        assert_eq!(parse_result("garbage"), Choice::Cancel);
    }

    #[test]
    fn quoting_and_cleaning() {
        assert_eq!(shell_quote("/a b/it's"), r"'/a b/it'\''s'");
        assert_eq!(clean("a\nb\0c\x1fd"), "abcd");
    }
}
