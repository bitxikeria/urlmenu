//! Message language, resolved like gettext: `LANGUAGE`, then `LC_ALL`,
//! `LC_MESSAGES`, `LANG`. Candidates follow the Desktop Entry Specification
//! order for localized keys: `lang_COUNTRY@MOD`, `lang_COUNTRY`, `lang@MOD`, `lang`.

#[derive(Debug)]
pub struct Messages {
    pub prompt: &'static str,
    /// Shortcut reminder; `{key}` is replaced by the configured key.
    pub private_hint: &'static str,
    pub copy_hint: &'static str,
    pub copy: &'static str,
    pub browsers: &'static str,
    pub private: &'static str,
    /// Profile tag around the profile name: `(before, after)`.
    pub profile: (&'static str, &'static str),
}

pub static EN: Messages = Messages {
    prompt: "Open with",
    private_hint: "{key}: open in a private window",
    copy_hint: "{key}: copy the link",
    copy: "Copy to clipboard",
    browsers: "Browsers",
    private: "Private",
    profile: ("profile “", "”"),
};

pub static FR: Messages = Messages {
    prompt: "Ouvrir avec",
    private_hint: "{key}\u{a0}: ouvrir dans une fenêtre privée",
    copy_hint: "{key}\u{a0}: copier le lien",
    copy: "Copier dans le presse-papiers",
    browsers: "Navigateurs",
    private: "Privé",
    profile: ("profil «\u{a0}", "\u{a0}»"),
};

pub fn languages_from_env() -> Vec<String> {
    let var = |k| std::env::var(k).ok().filter(|v: &String| !v.is_empty());
    let locale = var("LC_ALL")
        .or_else(|| var("LC_MESSAGES"))
        .or_else(|| var("LANG"));
    languages(var("LANGUAGE").as_deref(), locale.as_deref())
}

/// `LANGUAGE` is ignored when the locale is `C`/`POSIX`, as gettext does.
pub fn languages(language: Option<&str>, locale: Option<&str>) -> Vec<String> {
    let locale = locale.filter(|l| !matches!(*l, "C" | "POSIX") && !l.starts_with("C."));
    let Some(locale) = locale else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for name in language
        .into_iter()
        .flat_map(|l| l.split(':'))
        .chain([locale])
    {
        for candidate in candidates(name) {
            if !out.contains(&candidate) {
                out.push(candidate);
            }
        }
    }
    out
}

fn candidates(locale: &str) -> Vec<String> {
    let (rest, modifier) = match locale.split_once('@') {
        Some((r, m)) => (r, Some(m)),
        None => (locale, None),
    };
    let rest = rest.split('.').next().unwrap_or(rest);
    let (lang, country) = match rest.split_once('_') {
        Some((l, c)) => (l, Some(c)),
        None => (rest, None),
    };
    if lang.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let (Some(c), Some(m)) = (country, modifier) {
        out.push(format!("{lang}_{c}@{m}"));
    }
    if let Some(c) = country {
        out.push(format!("{lang}_{c}"));
    }
    if let Some(m) = modifier {
        out.push(format!("{lang}@{m}"));
    }
    out.push(lang.to_string());
    out
}

pub fn messages(languages: &[String]) -> &'static Messages {
    for lang in languages {
        match lang.split(['_', '@']).next() {
            Some("fr") => return &FR,
            Some("en") => return &EN,
            _ => {}
        }
    }
    &EN
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_resolution() {
        let cases: &[(Option<&str>, Option<&str>, &[&str])] = &[
            (None, Some("fr_FR.UTF-8"), &["fr_FR", "fr"]),
            (
                None,
                Some("sr_RS@latin"),
                &["sr_RS@latin", "sr_RS", "sr@latin", "sr"],
            ),
            (
                Some("de:fr_CA"),
                Some("en_US.UTF-8"),
                &["de", "fr_CA", "fr", "en_US", "en"],
            ),
            (Some("fr"), Some("C"), &[]),
            (None, Some("C.UTF-8"), &[]),
            (None, None, &[]),
        ];
        for (language, locale, expected) in cases {
            assert_eq!(
                languages(*language, *locale),
                *expected,
                "{language:?} {locale:?}"
            );
        }
    }

    #[test]
    fn message_table() {
        let l = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(messages(&l(&["fr_FR", "fr"])).copy, FR.copy);
        assert_eq!(
            messages(&l(&["de", "fr"])).copy,
            FR.copy,
            "first supported language wins"
        );
        assert_eq!(messages(&l(&["de"])).copy, EN.copy);
        assert_eq!(messages(&[]).copy, EN.copy);
    }
}
