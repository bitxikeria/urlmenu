use globset::{GlobBuilder, GlobMatcher};
use regex::Regex;

use crate::config::RuleConfig;
use crate::target::Target;

#[derive(Debug)]
pub struct Rule {
    pub index: usize,
    pub config: RuleConfig,
    host: Option<GlobMatcher>,
    url: Option<Regex>,
}

impl Rule {
    /// All set conditions must hold.
    pub fn matches(&self, url: &str) -> bool {
        let host_ok = self.host.as_ref().is_none_or(|glob| {
            url::Url::parse(url)
                .ok()
                .and_then(|u| u.host_str().map(|h| glob.is_match(h)))
                .unwrap_or(false)
        });
        let url_ok = self.url.as_ref().is_none_or(|re| re.is_match(url));
        host_ok && url_ok
    }

    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if let Some(h) = &self.config.host {
            parts.push(format!("host = {h:?}"));
        }
        if let Some(u) = &self.config.url {
            parts.push(format!("url = {u:?}"));
        }
        format!("rule #{} ({})", self.index + 1, parts.join(", "))
    }

    /// The target this rule opens. Without a `profile`, the browser's first
    /// profile is used.
    pub fn resolve(&self, targets: &[Target]) -> Result<usize, String> {
        let c = &self.config;
        targets
            .iter()
            .position(|t| {
                t.browser_id == c.browser
                    && t.private == c.private
                    && c.profile.as_deref().is_none_or(|p| t.matches_profile(p))
            })
            .ok_or_else(|| {
                let profile = c
                    .profile
                    .as_deref()
                    .map(|p| format!(", profile {p:?}"))
                    .unwrap_or_default();
                let private = if c.private { ", private" } else { "" };
                format!(
                    "{}: no target for browser {:?}{profile}{private}",
                    self.describe(),
                    c.browser
                )
            })
    }
}

pub fn compile(configs: &[RuleConfig]) -> Result<Vec<Rule>, String> {
    configs
        .iter()
        .enumerate()
        .map(|(index, c)| {
            let ctx = |e: String| format!("rule #{}: {e}", index + 1);
            if c.host.is_none() && c.url.is_none() {
                return Err(ctx("needs `host` or `url`".into()));
            }
            let host = c
                .host
                .as_deref()
                .map(|h| {
                    GlobBuilder::new(h)
                        .case_insensitive(true)
                        .build()
                        .map(|g| g.compile_matcher())
                        .map_err(|e| ctx(e.to_string()))
                })
                .transpose()?;
            let url = c
                .url
                .as_deref()
                .map(|u| Regex::new(u).map_err(|e| ctx(e.to_string())))
                .transpose()?;
            Ok(Rule {
                index,
                config: c.clone(),
                host,
                url,
            })
        })
        .collect()
}

pub fn find<'a>(rules: &'a [Rule], url: &str) -> Option<&'a Rule> {
    rules.iter().find(|r| r.matches(url))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::{Browser, Profile};
    use crate::target;

    fn rule(
        host: Option<&str>,
        url: Option<&str>,
        browser: &str,
        profile: Option<&str>,
        private: bool,
    ) -> RuleConfig {
        RuleConfig {
            host: host.map(String::from),
            url: url.map(String::from),
            browser: browser.into(),
            profile: profile.map(String::from),
            private,
        }
    }

    #[test]
    fn matching() {
        let rules = compile(&[
            rule(Some("*.mobicoop.io"), None, "ff", None, false),
            rule(
                None,
                Some(r"^https://meet\.google\.com/"),
                "chromium",
                None,
                false,
            ),
            rule(Some("github.com"), Some("/pulls"), "ff", None, true),
        ])
        .unwrap();
        let cases: &[(&str, Option<usize>)] = &[
            ("https://openproject.MOBICOOP.io/wp/1", Some(0)),
            ("https://mobicoop.io/", None),
            ("https://meet.google.com/abc", Some(1)),
            ("https://github.com/pulls", Some(2)),
            ("https://github.com/o/r", None),
            ("not a url", None),
        ];
        for (url, expected) in cases {
            assert_eq!(find(&rules, url).map(|r| r.index), *expected, "url: {url}");
        }
    }

    #[test]
    fn compile_errors() {
        assert!(compile(&[rule(None, None, "ff", None, false)]).is_err());
        assert!(compile(&[rule(None, Some("("), "ff", None, false)]).is_err());
    }

    #[test]
    fn resolve_targets() {
        let targets = target::build(
            &[Browser {
                id: "ff".into(),
                name: "Firefox".into(),
                icon: None,
                exec: vec!["firefox".into(), "%u".into()],
                private_exec: Some(vec![
                    "firefox".into(),
                    "--private-window".into(),
                    "%u".into(),
                ]),
                profiles: ["perso", "work"]
                    .map(|n| Profile {
                        name: n.into(),
                        key: n.into(),
                        args: vec!["-P".into(), n.into()],
                    })
                    .to_vec(),
            }],
            &crate::locale::EN,
        );
        let resolve = |c| compile(&[c]).unwrap()[0].resolve(&targets);
        assert_eq!(resolve(rule(Some("x"), None, "ff", None, false)), Ok(0));
        assert_eq!(
            resolve(rule(Some("x"), None, "ff", Some("work"), false)),
            Ok(1)
        );
        assert_eq!(
            resolve(rule(Some("x"), None, "ff", Some("work"), true)),
            Ok(3)
        );
        assert!(resolve(rule(Some("x"), None, "ff", Some("nope"), false)).is_err());
        assert!(resolve(rule(Some("x"), None, "brave", None, false)).is_err());
    }
}
