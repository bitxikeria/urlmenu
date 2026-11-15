//! Minimal INI reader shared by desktop entries, `profiles.ini` and `compatibility.ini`.

#[derive(Debug, Default)]
pub struct Ini {
    sections: Vec<(String, Vec<(String, String)>)>,
}

impl Ini {
    pub fn parse(text: &str) -> Self {
        let mut sections: Vec<(String, Vec<(String, String)>)> = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                sections.push((name.to_string(), Vec::new()));
            } else if let (Some((key, value)), Some(section)) =
                (line.split_once('='), sections.last_mut())
            {
                section
                    .1
                    .push((key.trim().to_string(), value.trim().to_string()));
            }
        }
        Self { sections }
    }

    pub fn sections(&self) -> impl Iterator<Item = &str> {
        self.sections.iter().map(|(name, _)| name.as_str())
    }

    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.sections
            .iter()
            .find(|(name, _)| name == section)
            .and_then(|(_, entries)| entries.iter().find(|(k, _)| k == key))
            .map(|(_, v)| v.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_sections_and_ignores_comments() {
        let ini = Ini::parse("# c\n[A]\nk = v\nName[fr]=x\n\n[B]\nk=w=z\n");
        assert_eq!(ini.get("A", "k"), Some("v"));
        assert_eq!(ini.get("A", "Name[fr]"), Some("x"));
        assert_eq!(ini.get("B", "k"), Some("w=z"));
        assert_eq!(ini.get("B", "missing"), None);
        assert_eq!(ini.sections().collect::<Vec<_>>(), ["A", "B"]);
    }
}
