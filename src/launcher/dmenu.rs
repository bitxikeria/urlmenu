//! dmenu protocol: rows on stdin, the chosen row (or its index) on stdout.
//! Covers fuzzel, wofi, bemenu, tofi, walker, dmenu and any compatible command.

use std::io::Write;
use std::process::{Command, Stdio};

use super::{COPY_EXIT, COPY_ICON, Launcher, Options, PRIVATE_EXIT, hints};
use crate::config::Shortcuts;
use crate::menu::{Choice, Item, Menu};

pub struct Dmenu {
    /// `{url}` in an argument is replaced by the URL.
    pub command: Vec<String>,
    /// Understands rofi-style `\0icon\x1f<name>` row options.
    pub icons: bool,
    /// Prints the row index instead of its text.
    pub index: bool,
    /// Arguments preselecting the row at an index. Without them, the
    /// suggestion is moved to the top so `Enter` still picks it.
    pub select: Option<fn(usize) -> Vec<String>>,
    /// Shows text above the prompt with `<flag>=<text>`.
    pub mesg_flag: Option<&'static str>,
    /// Custom keys exiting with [`PRIVATE_EXIT`] and [`COPY_EXIT`]. Without
    /// them, private variants and Copy are listed as rows.
    pub shortcuts: Option<KeyBindings>,
}

pub struct KeyBindings {
    /// Launcher arguments binding the keys.
    pub args: Vec<String>,
    /// Keys as configured, for the reminder.
    pub shortcuts: Shortcuts,
}

struct Row<'a> {
    label: &'a str,
    icon: Option<&'a str>,
    choice: Choice,
}

impl Dmenu {
    pub fn generic(command: Vec<String>) -> Self {
        Self {
            command,
            icons: false,
            index: false,
            select: None,
            mesg_flag: None,
            shortcuts: None,
        }
    }

    /// Browsers in their usual order, or with the suggestion first when the
    /// launcher cannot preselect; then private and Copy rows when there are
    /// no shortcuts for them.
    fn rows<'a>(&self, menu: &'a Menu, opts: Options) -> Vec<Row<'a>> {
        let mut items: Vec<&Item> = menu.items.iter().collect();
        if self.select.is_none()
            && let Some(pos) = items
                .iter()
                .position(|i| Some(&i.id) == menu.suggested.as_ref())
        {
            let suggested = items.remove(pos);
            items.insert(0, suggested);
        }
        let mut rows: Vec<Row> = items
            .iter()
            .map(|i| Row {
                label: &i.label,
                icon: i.icon.as_deref(),
                choice: Choice::Open(i.id.clone()),
            })
            .collect();
        if self.shortcuts.is_none() {
            if opts.private_entries {
                rows.extend(items.iter().filter_map(|i| {
                    i.private.as_ref().map(|v| Row {
                        label: &v.label,
                        icon: i.icon.as_deref(),
                        choice: Choice::Open(v.id.clone()),
                    })
                }));
            }
            if menu.copy {
                rows.push(Row {
                    label: opts.msg.copy,
                    icon: Some(COPY_ICON),
                    choice: Choice::Copy,
                });
            }
        }
        rows
    }

    /// Extra launcher arguments: preselection, key bindings, message.
    fn extra_args(&self, rows: &[Row], menu: &Menu, opts: Options) -> Vec<String> {
        let mut args = Vec::new();
        if let (Some(select), Some(id)) = (self.select, &menu.suggested)
            && let Some(i) = rows
                .iter()
                .position(|r| r.choice == Choice::Open(id.clone()))
        {
            args.extend(select(i));
        }
        if let Some(keys) = &self.shortcuts {
            args.extend(keys.args.iter().cloned());
        }
        if let Some(flag) = self.mesg_flag {
            let mut lines: Vec<String> = menu.url.iter().cloned().collect();
            if let Some(keys) = &self.shortcuts {
                lines.extend(hints(opts.msg, menu, &keys.shortcuts));
            }
            if !lines.is_empty() {
                args.push(format!("{flag}={}", lines.join("\n")));
            }
        }
        args
    }

    fn line(&self, row: &Row, opts: Options) -> String {
        match row.icon {
            Some(icon) if self.icons && opts.icons => format!("{}\0icon\x1f{icon}", row.label),
            _ => row.label.to_string(),
        }
    }

    fn parse(&self, rows: Vec<Row>, output: &str) -> Choice {
        let output = output.trim_end_matches(['\n', '\r']);
        let found = if self.index {
            output
                .trim()
                .parse::<usize>()
                .ok()
                .and_then(|i| rows.into_iter().nth(i))
        } else {
            rows.into_iter().find(|r| r.label == output)
        };
        found.map_or(Choice::Cancel, |r| r.choice)
    }
}

impl Launcher for Dmenu {
    fn pick(&self, menu: &Menu, opts: Options) -> Result<Choice, String> {
        let (program, args) = self.command.split_first().ok_or("empty launcher command")?;
        let url = menu.url.as_deref().unwrap_or("");
        let rows = self.rows(menu, opts);
        let input: Vec<String> = rows.iter().map(|r| self.line(r, opts)).collect();
        let mut child = Command::new(program)
            .args(args.iter().map(|a| a.replace("{url}", url)))
            .args(self.extra_args(&rows, menu, opts))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot run launcher {program:?}: {e}"))?;
        {
            let mut stdin = child.stdin.take().ok_or("launcher stdin unavailable")?;
            // A launcher may exit before reading everything; that is a cancel, not an error.
            let _ = stdin.write_all(input.join("\n").as_bytes());
        }
        let out = child
            .wait_with_output()
            .map_err(|e| format!("launcher {program:?}: {e}"))?;
        let has_keys = self.shortcuts.is_some();
        let choice = self.parse(rows, &String::from_utf8_lossy(&out.stdout));
        Ok(match out.status.code() {
            Some(0) => choice,
            Some(PRIVATE_EXIT) if has_keys => match choice {
                Choice::Open(id) => Choice::Open(menu.private_of(&id)),
                other => other,
            },
            Some(COPY_EXIT) if has_keys && menu.copy => Choice::Copy,
            _ => Choice::Cancel,
        })
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

    fn sh(script: &str, icons: bool, index: bool) -> Dmenu {
        Dmenu {
            icons,
            index,
            ..Dmenu::generic(vec![
                "sh".into(),
                "-c".into(),
                script.into(),
                "{url}".into(),
            ])
        }
    }

    fn fuzzel_like(script: &str) -> Dmenu {
        Dmenu {
            select: Some(|i| vec![format!("--select-index={i}")]),
            mesg_flag: Some("--mesg"),
            shortcuts: Some(KeyBindings {
                args: vec!["--bind".into()],
                shortcuts: Shortcuts::default(),
            }),
            ..sh(script, true, true)
        }
    }

    fn lines(d: &Dmenu, menu: &Menu, opts: Options) -> Vec<String> {
        d.rows(menu, opts).iter().map(|r| d.line(r, opts)).collect()
    }

    #[test]
    fn generic_rows() {
        let menu = sample();
        assert_eq!(
            lines(&sh("", true, false), &menu, ALL),
            [
                "Firefox\0icon\x1ffirefox",
                "qutebrowser",
                "[Private] Firefox\0icon\x1ffirefox",
                "Copy to clipboard\0icon\x1fedit-copy",
            ]
        );
        let plain = Options {
            private_entries: false,
            ..ALL
        };
        assert_eq!(
            lines(&sh("", false, false), &menu, plain),
            ["Firefox", "qutebrowser", "Copy to clipboard"]
        );
    }

    #[test]
    fn suggestion_without_select_flag_moves_first() {
        let mut menu = sample();
        menu.suggested = Some("qute".into());
        let d = sh("", false, false);
        assert!(d.extra_args(&d.rows(&menu, ALL), &menu, ALL).is_empty());
        assert_eq!(
            lines(&d, &menu, ALL),
            [
                "qutebrowser",
                "Firefox",
                "[Private] Firefox",
                "Copy to clipboard"
            ]
        );
    }

    #[test]
    fn shortcuts_replace_private_and_copy_rows() {
        let mut menu = sample();
        menu.suggested = Some("qute".into());
        let d = fuzzel_like("");
        let rows = d.rows(&menu, ALL);
        assert_eq!(
            d.extra_args(&rows, &menu, ALL),
            [
                "--select-index=1",
                "--bind",
                "--mesg=https://e.com/?a=1&b=2\nAlt+Enter: open in a private window  ·  Ctrl+Shift+C: copy the link",
            ]
        );
        assert_eq!(
            lines(&d, &menu, ALL),
            ["Firefox\0icon\x1ffirefox", "qutebrowser"]
        );
    }

    #[test]
    fn pick_results() {
        let menu = sample();
        let cases: &[(Dmenu, Choice)] = &[
            (
                sh("sed -n 3p", false, false),
                Choice::Open("ff+private".into()),
            ),
            (
                sh("cat >/dev/null; echo 1", true, true),
                Choice::Open("qute".into()),
            ),
            (sh("tail -n1", false, false), Choice::Copy),
            (
                sh("cat >/dev/null; echo unknown", false, false),
                Choice::Cancel,
            ),
            (sh("cat >/dev/null; echo 9", true, true), Choice::Cancel),
            (sh("head -n1; exit 1", false, false), Choice::Cancel),
            (sh("head -n1; exit 10", false, false), Choice::Cancel),
            (
                fuzzel_like("cat >/dev/null; echo 0; exit 10"),
                Choice::Open("ff+private".into()),
            ),
            (
                fuzzel_like("cat >/dev/null; echo 1; exit 10"),
                Choice::Open("qute".into()),
            ),
            (fuzzel_like("cat >/dev/null; echo 1; exit 11"), Choice::Copy),
        ];
        for (launcher, expected) in cases {
            assert_eq!(
                &launcher.pick(&menu, ALL).unwrap(),
                expected,
                "{:?}",
                launcher.command
            );
        }
    }

    #[test]
    fn url_placeholder() {
        let d = sh(
            r#"cat >/dev/null; [ "$0" = "https://e.com/?a=1&b=2" ] && echo qutebrowser"#,
            false,
            false,
        );
        assert_eq!(d.pick(&sample(), ALL).unwrap(), Choice::Open("qute".into()));
    }
}
