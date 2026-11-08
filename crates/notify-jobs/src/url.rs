//! Paths and URLs as jobs report them: "file:///a/b%20c" -> /a/b c, shown with ~ and without passwords.

use std::path::PathBuf;

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A local path from "file:///..." or a plain absolute path; None for remote URLs and non-paths.
pub fn local_path(s: &str) -> Option<PathBuf> {
    let p = if let Some(rest) = s.strip_prefix("file://") { percent_decode(rest.strip_prefix("localhost").unwrap_or(rest)) } else if s.starts_with('/') { s.to_string() } else { return None };
    let p = p.trim_end_matches("/.").to_string();
    (p.starts_with('/')).then(|| PathBuf::from(if p.len() > 1 { p.trim_end_matches('/').to_string() } else { p }))
}

pub(crate) fn tilde(p: &str) -> String {
    match std::env::var("HOME") {
        Ok(h) if !h.is_empty() && h != "/" && (p == h || p.starts_with(&format!("{h}/"))) => format!("~{}", &p[h.len()..]),
        _ => p.to_string(),
    }
}

/// A URL for display: local paths with ~, remote ones without the user name or password.
pub fn pretty_url(s: &str) -> String {
    if let Some(p) = local_path(s) {
        return tilde(&p.to_string_lossy());
    }
    match s.split_once("://") {
        Some((scheme, rest)) => {
            let (host, path) = rest.split_once('/').map(|(h, p)| (h, format!("/{p}"))).unwrap_or((rest, String::new()));
            let host = host.rsplit_once('@').map(|(_, h)| h).unwrap_or(host);
            format!("{scheme}://{host}{}", percent_decode(&path))
        }
        None => s.to_string(),
    }
}
