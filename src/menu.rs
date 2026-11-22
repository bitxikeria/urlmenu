//! What the launcher offers, independent of how it is drawn.

use serde::Serialize;

use crate::target::Target;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Item {
    pub id: String,
    pub label: String,
    pub icon: Option<String>,
    pub private: Option<Variant>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Variant {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Menu {
    /// First URL; the picked target opens it with the `more` that follow.
    pub url: Option<String>,
    #[serde(skip)]
    pub more: usize,
    /// Normal targets; each carries its private variant.
    pub items: Vec<Item>,
    /// Offer "Copy to clipboard".
    pub copy: bool,
    /// Item to preselect.
    pub suggested: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Choice {
    Open(String),
    Copy,
    Cancel,
}

impl Menu {
    /// Items keep the target order; the first `preferred` id present becomes
    /// the suggestion.
    pub fn build(
        targets: &[Target],
        url: Option<String>,
        copy: bool,
        preferred: &[String],
    ) -> Self {
        let items: Vec<Item> = targets
            .iter()
            .filter(|t| !t.private)
            .map(|t| Item {
                id: t.id(),
                label: t.label.clone(),
                icon: t.icon.clone(),
                private: t.private_sibling.map(|i| Variant {
                    id: targets[i].id(),
                    label: targets[i].label.clone(),
                }),
            })
            .collect();
        let suggested = preferred
            .iter()
            .find(|id| items.iter().any(|i| &i.id == *id))
            .cloned();
        let copy = copy && url.is_some();
        Self {
            url,
            more: 0,
            items,
            copy,
            suggested,
        }
    }

    /// URL as shown in the launcher: `https://a.com/ (+2)` for three links.
    pub fn shown_url(&self) -> Option<String> {
        self.url.as_ref().map(|u| match self.more {
            0 => u.clone(),
            n => format!("{u} (+{n})"),
        })
    }

    pub fn has_private(&self) -> bool {
        self.items.iter().any(|i| i.private.is_some())
    }

    /// Id of the private variant of `id`, or `id` itself when it has none.
    pub fn private_of(&self, id: &str) -> String {
        self.items
            .iter()
            .find(|i| i.id == id)
            .and_then(|i| i.private.as_ref())
            .map_or_else(|| id.to_string(), |v| v.id.clone())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn sample() -> Menu {
        Menu {
            url: Some("https://e.com/?a=1&b=2".into()),
            more: 0,
            items: vec![
                Item {
                    id: "ff".into(),
                    label: "Firefox".into(),
                    icon: Some("firefox".into()),
                    private: Some(Variant {
                        id: "ff+private".into(),
                        label: "[Private] Firefox".into(),
                    }),
                },
                Item {
                    id: "qute".into(),
                    label: "qutebrowser".into(),
                    icon: None,
                    private: None,
                },
            ],
            copy: true,
            suggested: None,
        }
    }

    #[test]
    fn suggestion_from_preferred() {
        use crate::detect::Browser;
        let browser = |id: &str| Browser {
            id: id.into(),
            name: id.into(),
            icon: None,
            exec: vec![id.into()],
            private_exec: None,
            profiles: Vec::new(),
        };
        let targets = crate::target::build(
            &[browser("a"), browser("b"), browser("c")],
            &crate::locale::EN,
        );
        let ids = |m: &Menu| m.items.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
        let pref = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();

        let m = Menu::build(&targets, None, false, &pref(&["c", "b"]));
        assert_eq!(ids(&m), ["a", "b", "c"], "order unchanged");
        assert_eq!(m.suggested.as_deref(), Some("c"));

        let m = Menu::build(&targets, None, false, &pref(&["gone", "b"]));
        assert_eq!(ids(&m), ["a", "b", "c"]);
        assert_eq!(m.suggested.as_deref(), Some("b"));

        let m = Menu::build(&targets, None, false, &[]);
        assert_eq!(ids(&m), ["a", "b", "c"]);
        assert_eq!(m.suggested, None);
    }

    #[test]
    fn private_lookup() {
        let m = sample();
        assert!(m.has_private());
        assert_eq!(m.private_of("ff"), "ff+private");
        assert_eq!(m.private_of("qute"), "qute");
    }

    #[test]
    fn shown_url_counts_the_other_links() {
        let mut m = sample();
        assert_eq!(m.shown_url().as_deref(), Some("https://e.com/?a=1&b=2"));
        m.more = 2;
        assert_eq!(
            m.shown_url().as_deref(),
            Some("https://e.com/?a=1&b=2 (+2)")
        );
    }
}
