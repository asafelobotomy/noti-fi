//! Every source file stays within 400 lines.
//!
//! A ratchet: files that were already longer when this rule was added are listed in `tests/loc-baseline.txt` with the length they may not exceed.
//! A listed file may shrink (lower its number, or delete the line once it is within the limit) but never grow; any other file must be within the limit.
//! Frozen reference code (`legacy/`), generated output, data files (JSON) and docs are not counted.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const LIMIT: usize = 400;
const SOURCE_EXTENSIONS: [&str; 4] = ["rs", "qml", "py", "sh"];
const SKIP_DIRS: [&str; 6] = ["target", ".git", "legacy", "shots", "node_modules", ".cache"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if p.is_dir() {
            if !SKIP_DIRS.contains(&name.as_str()) {
                walk(&p, out);
            }
        } else if p.extension().and_then(|x| x.to_str()).is_some_and(|x| SOURCE_EXTENSIONS.contains(&x)) {
            out.push(p);
        }
    }
}

fn baseline(root: &Path) -> BTreeMap<String, usize> {
    let text = fs::read_to_string(root.join("tests/loc-baseline.txt")).unwrap_or_default();
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let (path, n) = l.rsplit_once(' ')?;
            Some((path.trim().to_string(), n.trim().parse().ok()?))
        })
        .collect()
}

#[test]
fn no_source_file_exceeds_400_lines_unless_it_is_baselined_and_not_growing() {
    let root = root();
    let base = baseline(&root);
    let mut files = Vec::new();
    walk(&root, &mut files);
    let mut problems = Vec::new();
    let mut seen = Vec::new();
    for f in &files {
        let rel = f.strip_prefix(&root).unwrap().to_string_lossy().into_owned();
        let lines = fs::read_to_string(f).map(|t| t.lines().count()).unwrap_or(0);
        seen.push(rel.clone());
        match (lines > LIMIT, base.get(&rel)) {
            (true, None) => problems.push(format!("{rel}: {lines} lines (limit {LIMIT}); split it into smaller files")),
            (true, Some(max)) if lines > *max => problems.push(format!("{rel}: grew to {lines} lines, but it may not exceed {max} (and should shrink toward {LIMIT})")),
            (false, Some(_)) => problems.push(format!("{rel}: now {lines} lines, within the limit; remove it from tests/loc-baseline.txt")),
            _ => {}
        }
    }
    for rel in base.keys() {
        if !seen.contains(rel) {
            problems.push(format!("{rel}: listed in tests/loc-baseline.txt but the file no longer exists; remove the line"));
        }
    }
    assert!(problems.is_empty(), "per-file line limit ({LIMIT}):\n  {}", problems.join("\n  "));
}
