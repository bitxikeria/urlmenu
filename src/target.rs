use crate::detect::{Browser, Profile};
use crate::exec;
use crate::locale::Messages;

/// One pickable line: a browser, optionally a profile, optionally private.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub label: String,
    pub icon: Option<String>,
    pub browser_id: String,
    pub profile: Option<Profile>,
    pub private: bool,
    template: Vec<String>,
    /// Index of the private variant, for the launcher's alternate key.
    pub private_sibling: Option<usize>,
}

impl Target {
    /// Final argv. The URL is always a single argument.
    pub fn command(&self, url: Option<&str>) -> Vec<String> {
        self.argv(url.as_slice())
    }

    /// Commands opening every URL: one command when the `Exec` template takes
    /// a list, one per URL otherwise.
    pub fn commands(&self, urls: &[String]) -> Vec<Vec<String>> {
        let urls: Vec<&str> = urls.iter().map(String::as_str).collect();
        if urls.len() <= 1 || exec::takes_list(&self.template) {
            vec![self.argv(&urls)]
        } else {
            urls.iter().map(|u| self.argv(&[u])).collect()
        }
    }

    fn argv(&self, urls: &[&str]) -> Vec<String> {
        let mut argv = exec::expand(&self.template, urls);
        if let Some(profile) = &self.profile {
            argv.splice(1..1, profile.args.iter().cloned());
        }
        argv
    }

    /// Stable identifier: `<browser>[/<profile-key>][+private]`, each part
    /// percent-encoded so `/`, `+` and control characters stay unambiguous.
    pub fn id(&self) -> String {
        let mut id = encode_id_part(&self.browser_id);
        if let Some(profile) = &self.profile {
            id.push('/');
            id.push_str(&encode_id_part(&profile.key));
        }
        if self.private {
            id.push_str("+private");
        }
        id
    }

    pub fn matches_profile(&self, wanted: &str) -> bool {
        self.profile
            .as_ref()
            .is_some_and(|p| p.name == wanted || p.key == wanted)
    }
}

fn encode_id_part(part: &str) -> String {
    let mut out = String::with_capacity(part.len());
    for byte in part.bytes() {
        if byte.is_ascii_alphanumeric() || b" _.-".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

pub fn find(targets: &[Target], id: &str) -> Option<usize> {
    targets.iter().position(|t| t.id() == id)
}

/// Normal targets first, private variants after, in the same order.
/// Labels: `Name [profile “x”]`, prefixed with `[Private] ` for private variants.
pub fn build(browsers: &[Browser], msg: &Messages) -> Vec<Target> {
    let mut normal = Vec::new();
    let mut private = Vec::new();
    for b in browsers {
        let profiles: Vec<Option<&Profile>> = if b.profiles.is_empty() {
            vec![None]
        } else {
            b.profiles.iter().map(Some).collect()
        };
        for profile in profiles {
            let label = match profile {
                Some(p) => format!("{} [{}{}{}]", b.name, msg.profile.0, p.name, msg.profile.1),
                None => b.name.clone(),
            };
            let base = Target {
                label: label.clone(),
                icon: b.icon.clone(),
                browser_id: b.id.clone(),
                profile: profile.cloned(),
                private: false,
                template: b.exec.clone(),
                private_sibling: None,
            };
            let private_variant = b.private_exec.as_ref().map(|t| Target {
                label: format!("[{}] {label}", msg.private),
                private: true,
                template: t.clone(),
                ..base.clone()
            });
            normal.push((base, private_variant.is_some()));
            private.extend(private_variant);
        }
    }
    let offset = normal.len();
    let mut next_private = offset;
    let mut out: Vec<Target> = normal
        .into_iter()
        .map(|(mut t, has_private)| {
            if has_private {
                t.private_sibling = Some(next_private);
                next_private += 1;
            }
            t
        })
        .collect();
    out.extend(private);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    fn browsers() -> Vec<Browser> {
        vec![
            Browser {
                id: "chromium".into(),
                name: "Chromium".into(),
                icon: Some("chromium".into()),
                exec: s(&["chromium", "%U"]),
                private_exec: Some(s(&["chromium", "--incognito"])),
                profiles: vec![
                    Profile {
                        name: "Travail".into(),
                        key: "Default".into(),
                        args: s(&["--profile-directory=Default"]),
                    },
                    Profile {
                        name: "Chill".into(),
                        key: "Profile 1".into(),
                        args: s(&["--profile-directory=Profile 1"]),
                    },
                ],
            },
            Browser {
                id: "qute".into(),
                name: "qutebrowser".into(),
                icon: None,
                exec: s(&["qutebrowser", "%u"]),
                private_exec: None,
                profiles: Vec::new(),
            },
        ]
    }

    #[test]
    fn labels_and_siblings() {
        let t = build(&browsers(), &crate::locale::EN);
        let labels: Vec<_> = t.iter().map(|x| x.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Chromium [profile “Travail”]",
                "Chromium [profile “Chill”]",
                "qutebrowser",
                "[Private] Chromium [profile “Travail”]",
                "[Private] Chromium [profile “Chill”]",
            ]
        );
        assert_eq!(t[0].private_sibling, Some(3));
        assert_eq!(t[1].private_sibling, Some(4));
        assert_eq!(t[2].private_sibling, None);
    }

    #[test]
    fn ids_are_unique_and_found() {
        let t = build(&browsers(), &crate::locale::EN);
        let ids: Vec<String> = t.iter().map(Target::id).collect();
        assert_eq!(
            ids,
            [
                "chromium/Default",
                "chromium/Profile 1",
                "qute",
                "chromium/Default+private",
                "chromium/Profile 1+private",
            ]
        );
        for (i, id) in ids.iter().enumerate() {
            assert_eq!(find(&t, id), Some(i));
        }
        assert_eq!(find(&t, "nope"), None);
    }

    #[test]
    fn id_parts_are_encoded() {
        assert_eq!(encode_id_part("a/b+c%d\te:é"), "a%2Fb%2Bc%25d%09e%3A%C3%A9");
    }

    #[test]
    fn command_inserts_profile_and_url() {
        let t = build(&browsers(), &crate::locale::EN);
        assert_eq!(
            t[4].command(Some("https://e.com")),
            s(&[
                "chromium",
                "--profile-directory=Profile 1",
                "--incognito",
                "https://e.com"
            ])
        );
        assert_eq!(
            t[0].command(None),
            s(&["chromium", "--profile-directory=Default"])
        );
        assert!(t[1].matches_profile("Chill") && t[1].matches_profile("Profile 1"));
    }

    #[test]
    fn commands_for_several_urls() {
        let t = build(&browsers(), &crate::locale::EN);
        let urls = s(&["https://a.com", "https://b.com"]);
        assert_eq!(
            t[0].commands(&urls),
            [s(&[
                "chromium",
                "--profile-directory=Default",
                "https://a.com",
                "https://b.com"
            ])]
        );
        assert_eq!(
            t[2].commands(&urls),
            [
                s(&["qutebrowser", "https://a.com"]),
                s(&["qutebrowser", "https://b.com"])
            ]
        );
        assert_eq!(t[2].commands(&[]), [s(&["qutebrowser"])]);
    }
}
