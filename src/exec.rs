//! `Exec=` handling per the Desktop Entry Specification: value unescaping,
//! quoted-argument splitting and field-code expansion.

/// Undoes the string-value escapes (`\s`, `\n`, `\t`, `\r`, `\\`).
pub fn unescape_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Splits an unescaped `Exec` value into arguments, honouring double quotes.
pub fn split(exec: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_arg = false;
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_arg = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(e @ ('"' | '`' | '$' | '\\')) => current.push(e),
                            Some(e) => {
                                current.push('\\');
                                current.push(e);
                            }
                            None => return Err(format!("unterminated quote in Exec: {exec}")),
                        },
                        Some(ch) => current.push(ch),
                        None => return Err(format!("unterminated quote in Exec: {exec}")),
                    }
                }
            }
            c if c.is_whitespace() => {
                if in_arg {
                    args.push(std::mem::take(&mut current));
                    in_arg = false;
                }
            }
            c => {
                in_arg = true;
                current.push(c);
            }
        }
    }
    if in_arg {
        args.push(current);
    }
    if args.is_empty() {
        return Err("empty Exec".to_string());
    }
    Ok(args)
}

/// Builds the final argv. `%u %U %f %F` become the URL; other field codes are
/// dropped. When the template carries no URL field code, the URL is appended,
/// which covers actions such as `Exec=chromium --incognito`.
pub fn expand(template: &[String], url: Option<&str>) -> Vec<String> {
    let mut argv = Vec::with_capacity(template.len() + 1);
    let mut url_placed = false;
    for token in template {
        if matches!(token.as_str(), "%u" | "%U" | "%f" | "%F") {
            url_placed = true;
            argv.extend(url.map(str::to_string));
            continue;
        }
        let mut out = String::with_capacity(token.len());
        let mut chars = token.chars();
        while let Some(c) = chars.next() {
            if c != '%' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('%') => out.push('%'),
                Some('u' | 'U' | 'f' | 'F') => {
                    url_placed = true;
                    out.push_str(url.unwrap_or(""));
                }
                _ => {}
            }
        }
        if !out.is_empty() {
            argv.push(out);
        }
    }
    if !url_placed && let Some(url) = url {
        argv.push(url.to_string());
    }
    argv
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn split_cases() {
        let cases: &[(&str, &[&str])] = &[
            ("firefox %u", &["firefox", "%u"]),
            ("  a   b  ", &["a", "b"]),
            (
                r#""/opt/My App/bin" --x %U"#,
                &["/opt/My App/bin", "--x", "%U"],
            ),
            (r#"sh "a \"q\" \$b \\c""#, &["sh", r#"a "q" $b \c"#]),
            (r#"x """#, &["x", ""]),
        ];
        for (input, expected) in cases {
            assert_eq!(split(input).unwrap(), s(expected), "input: {input}");
        }
        assert!(split(r#"x "open"#).is_err());
        assert!(split("   ").is_err());
    }

    #[test]
    fn unescape_cases() {
        assert_eq!(unescape_value(r"a\sb\\c\x"), r"a b\c\x");
    }

    #[test]
    fn expand_cases() {
        let url = Some("https://e.com/?a=1&b=2");
        let cases: &[(&[&str], Option<&str>, &[&str])] = &[
            (&["ff", "%u"], url, &["ff", "https://e.com/?a=1&b=2"]),
            (&["ff", "%u"], None, &["ff"]),
            (
                &["chromium", "--incognito"],
                url,
                &["chromium", "--incognito", "https://e.com/?a=1&b=2"],
            ),
            (
                &["chromium", "--incognito"],
                None,
                &["chromium", "--incognito"],
            ),
            (
                &["app", "--icon", "%i", "%c", "%k", "%F"],
                url,
                &["app", "--icon", "https://e.com/?a=1&b=2"],
            ),
            (
                &["app", "--url=%u", "100%%"],
                url,
                &["app", "--url=https://e.com/?a=1&b=2", "100%"],
            ),
        ];
        for (template, url, expected) in cases {
            assert_eq!(
                expand(&s(template), *url),
                s(expected),
                "template: {template:?}"
            );
        }
    }

    #[test]
    fn hostile_url_stays_one_argument() {
        let url = r#"https://e.com/"$(touch /tmp/pwn)"; rm -rf ~"#;
        assert_eq!(expand(&s(&["ff", "%u"]), Some(url)), s(&["ff", url]));
    }
}
