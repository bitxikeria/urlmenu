//! URL normalisation: unwrap redirectors, then strip tracking query keys.
//! The query is filtered on its raw `key=value` pieces, so kept parameters
//! reach the browser byte for byte.

use url::Url;

const TRACKING_KEYS: &[&str] = &[
    "utm_*",
    "ga_*",
    "_ga",
    "_gl",
    "gclid",
    "gclsrc",
    "dclid",
    "msclkid",
    "twclid",
    "yclid",
    "fbclid",
    "fbadid",
    "fb_action_ids",
    "fb_action_types",
    "fb_ref",
    "fb_source",
    "igshid",
    "mc_cid",
    "mc_eid",
    "mkt_tok",
    "_hsenc",
    "_hsmi",
    "hmb_campaign",
    "hmb_medium",
    "hmb_source",
    "otm_*",
    "stm_*",
    "ref_*",
    "cmpid",
    "os_ehash",
    "gs_l",
    "action_object_map",
    "action_type_map",
    "action_ref_map",
    "oly_anon_id",
    "oly_enc_id",
    "otc",
    "wickedid",
    "soc_src",
    "soc_trk",
    "_openstat",
    "icid",
    "rb_clickid",
    "sref",
];

const MAX_UNWRAP: usize = 3;

pub fn normalize(input: &str, clean: bool, extra_keys: &[String]) -> String {
    let Ok(mut url) = Url::parse(input) else {
        return input.to_string();
    };
    if !is_web(&url) {
        return input.to_string();
    }
    for _ in 0..MAX_UNWRAP {
        match unwrap_redirect(&url) {
            Some(inner) => url = inner,
            None => break,
        }
    }
    if clean {
        strip_tracking(&mut url, extra_keys);
    }
    url.into()
}

fn is_web(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

fn unwrap_redirect(url: &Url) -> Option<Url> {
    let host = url.host_str()?;
    let path = url.path();
    let keys: &[&str] = if host.ends_with(".safelinks.protection.outlook.com") {
        &["url"]
    } else if is_google_host(host) && path == "/url" {
        &["q", "url"]
    } else if host == "l.instagram.com"
        || (matches!(host, "l.facebook.com" | "lm.facebook.com") && path == "/l.php")
    {
        &["u"]
    } else {
        return None;
    };
    keys.iter()
        .find_map(|key| {
            url.query_pairs()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.into_owned())
        })
        .and_then(|target| Url::parse(&target).ok())
        .filter(is_web)
}

fn is_google_host(host: &str) -> bool {
    let rest = host.strip_prefix("www.").unwrap_or(host);
    rest.starts_with("google.")
}

fn strip_tracking(url: &mut Url, extra_keys: &[String]) {
    let Some(query) = url.query() else { return };
    let kept: Vec<&str> = query
        .split('&')
        .filter(|piece| !piece.is_empty())
        .filter(|piece| {
            let raw_key = piece.split_once('=').map_or(*piece, |(k, _)| k);
            let key = percent_decode(raw_key).to_ascii_lowercase();
            !is_tracking(&key, extra_keys)
        })
        .collect();
    if kept.len() == query.split('&').filter(|p| !p.is_empty()).count() {
        return;
    }
    let new_query = kept.join("&");
    url.set_query(if new_query.is_empty() {
        None
    } else {
        Some(&new_query)
    });
}

fn is_tracking(key: &str, extra_keys: &[String]) -> bool {
    TRACKING_KEYS
        .iter()
        .copied()
        .chain(extra_keys.iter().map(String::as_str))
        .any(|pattern| match pattern.strip_suffix('*') {
            Some(prefix) => key.starts_with(&prefix.to_ascii_lowercase()),
            None => key == pattern.to_ascii_lowercase(),
        })
}

fn percent_decode(s: &str) -> String {
    url::form_urlencoded::parse(format!("{s}=").as_bytes())
        .next()
        .map(|(k, _)| k.into_owned())
        .unwrap_or_else(|| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_cases() {
        let cases: &[(&str, &str)] = &[
            ("https://e.com/?utm_source=x&id=1", "https://e.com/?id=1"),
            ("https://e.com/?utm_source=x&utm_medium=y", "https://e.com/"),
            (
                "https://e.com/p?a=%26b&fbclid=z#frag",
                "https://e.com/p?a=%26b#frag",
            ),
            (
                "https://e.com/?UTM_Source=x&q=a+b%2Fc",
                "https://e.com/?q=a+b%2Fc",
            ),
            (
                "https://github.com/o/r/blob/x?ref=main",
                "https://github.com/o/r/blob/x?ref=main",
            ),
            ("https://e.com/?id=1", "https://e.com/?id=1"),
            (
                "https://www.google.com/url?q=https://e.com/a%3Fid%3D1%26utm_term%3Dz&sa=D",
                "https://e.com/a?id=1",
            ),
            (
                "https://eur01.safelinks.protection.outlook.com/?url=https%3A%2F%2Fe.com%2Fx&data=abc",
                "https://e.com/x",
            ),
            (
                "https://l.facebook.com/l.php?u=https%3A%2F%2Fe.com%2F&h=1",
                "https://e.com/",
            ),
            (
                "https://www.google.com/url?q=javascript:alert(1)",
                "https://www.google.com/url?q=javascript:alert(1)",
            ),
            ("mailto:a@b.c?utm_source=x", "mailto:a@b.c?utm_source=x"),
            ("file:///tmp/a.html", "file:///tmp/a.html"),
            ("/tmp/a.html", "/tmp/a.html"),
        ];
        for (input, expected) in cases {
            assert_eq!(normalize(input, true, &[]), *expected, "input: {input}");
        }
    }

    #[test]
    fn clean_disabled_still_unwraps() {
        assert_eq!(
            normalize(
                "https://l.instagram.com/?u=https%3A%2F%2Fe.com%2F%3Futm_source%3Dx",
                false,
                &[]
            ),
            "https://e.com/?utm_source=x"
        );
    }

    #[test]
    fn extra_keys() {
        let extra = vec!["si".to_string(), "pk_*".to_string()];
        assert_eq!(
            normalize("https://youtu.be/x?si=abc&pk_campaign=1&t=3", true, &extra),
            "https://youtu.be/x?t=3"
        );
    }
}
