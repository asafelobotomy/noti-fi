//! Check for a newer noti-fi release on GitHub and install it.
//!
//! Safety rules:
//! - nothing is contacted unless a caller asks (the CLI's `notify upgrade`, or the panel's "Check now");
//! - only HTTPS URLs under this project's own release path are downloaded (a response naming another host is refused);
//! - the archive is checked against the `.sha256` file published beside it (this guards against a damaged download; it does not
//!   prove who published the release, which is GitHub's account security);
//! - files are replaced one at a time by writing a new file and renaming it over the old one, so a running program is never overwritten in place.
//!
//! Network access goes through the [`Fetch`] trait so tests never touch the network.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

pub const REPO: &str = "asafelobotomy/noti-fi";
/// The version of this build (the workspace version).
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

const DOWNLOAD_PREFIX: &str = "https://github.com/asafelobotomy/noti-fi/releases/download/";
const API_LATEST: &str = "https://api.github.com/repos/asafelobotomy/noti-fi/releases/latest";

/// Where bytes come from. The real one is [`Curl`].
pub trait Fetch {
    fn get(&self, url: &str) -> Result<Vec<u8>, String>;
}

/// Downloads with `curl` (HTTPS only, redirects only to HTTPS, 30 s limit for pages and 5 min for the archive).
pub struct Curl;

impl Fetch for Curl {
    fn get(&self, url: &str) -> Result<Vec<u8>, String> {
        if !url.starts_with("https://") {
            return Err(format!("refusing to fetch a non-HTTPS address: {url}"));
        }
        let limit = if url.ends_with(".tar.gz") { "300" } else { "30" };
        let out = Command::new("curl")
            .args(["--fail", "--silent", "--show-error", "--location", "--proto", "=https", "--proto-redir", "=https", "--max-time", limit])
            .args(["--header", "Accept: application/vnd.github+json", "--user-agent", &format!("noti-fi/{CURRENT}")])
            .arg(url)
            .output()
            .map_err(|e| format!("cannot run curl (is it installed?): {e}"))?;
        if out.status.success() {
            Ok(out.stdout)
        } else {
            Err(format!("download failed: {}", String::from_utf8_lossy(&out.stderr).trim()))
        }
    }
}

/// A published release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    pub version: String,
    /// The release page, for reading what changed.
    pub page: String,
    pub notes: String,
    pub archive_name: String,
    pub archive_url: String,
    pub checksum_url: String,
}

/// Compare dotted numbers (`0.10.0` is newer than `0.9.2`); anything after a `-` (a pre-release) is ignored, and a missing part counts as 0.
pub fn is_newer(current: &str, candidate: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> { v.trim_start_matches('v').split('-').next().unwrap_or("").split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    let (a, b) = (parts(current), parts(candidate));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return y > x;
        }
    }
    false
}

/// Read GitHub's "latest release" answer for this machine's architecture.
pub fn parse_release(json: &str, arch: &str) -> Result<Release, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("unreadable answer from GitHub: {e}"))?;
    let tag = v["tag_name"].as_str().ok_or("the answer has no release tag")?.to_string();
    let version = tag.trim_start_matches('v').to_string();
    let want = format!("noti-fi-{version}-{arch}-linux.tar.gz");
    let assets = v["assets"].as_array().ok_or("the release lists no files")?;
    let url_of = |name: &str| assets.iter().find(|a| a["name"] == name).and_then(|a| a["browser_download_url"].as_str()).map(String::from);
    let archive_url = url_of(&want).ok_or_else(|| format!("release {tag} has no build for {arch} (looked for {want})"))?;
    let checksum_url = url_of(&format!("{want}.sha256")).ok_or_else(|| format!("release {tag} has no checksum file for {want}"))?;
    for u in [&archive_url, &checksum_url] {
        if !u.starts_with(DOWNLOAD_PREFIX) {
            return Err(format!("refusing a download address outside this project's releases: {u}"));
        }
    }
    Ok(Release {
        tag,
        version,
        page: v["html_url"].as_str().unwrap_or("").to_string(),
        notes: v["body"].as_str().unwrap_or("").to_string(),
        archive_name: want,
        archive_url,
        checksum_url,
    })
}

/// The newest release.
pub fn latest(f: &dyn Fetch) -> Result<Release, String> {
    let body = f.get(API_LATEST)?;
    parse_release(&String::from_utf8_lossy(&body), std::env::consts::ARCH)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Check `bytes` against a `sha256sum`-style line (`<hex>  <name>`) that names `name`.
pub fn verify_checksum(bytes: &[u8], checksum_text: &str, name: &str) -> Result<(), String> {
    let want = checksum_text
        .lines()
        .filter_map(|l| l.split_once(char::is_whitespace))
        .find(|(_, n)| n.trim().trim_start_matches('*') == name)
        .map(|(h, _)| h.trim().to_lowercase())
        .ok_or_else(|| format!("the checksum file does not mention {name}"))?;
    let got = sha256_hex(bytes);
    if got == want { Ok(()) } else { Err(format!("checksum mismatch for {name}: expected {want}, got {got}")) }
}

/// Download the release archive and check it. Returns the verified bytes.
pub fn download_verified(f: &dyn Fetch, rel: &Release) -> Result<Vec<u8>, String> {
    let sums = f.get(&rel.checksum_url)?;
    let archive = f.get(&rel.archive_url)?;
    verify_checksum(&archive, &String::from_utf8_lossy(&sums), &rel.archive_name)?;
    Ok(archive)
}

/// Where this program is installed (`<prefix>/bin/<name>`), or `None` when it is not in a `bin` folder (for example a build in `target/`).
pub fn install_prefix(exe: &Path) -> Option<PathBuf> {
    let bin = exe.parent()?;
    if bin.file_name()? != "bin" {
        return None;
    }
    bin.parent().map(Path::to_path_buf)
}

fn replace_file(from: &Path, to: &Path, mode: u32) -> Result<(), String> {
    let dir = to.parent().ok_or("no folder to install into")?;
    fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let tmp = dir.join(format!(".{}.new", to.file_name().and_then(|n| n.to_str()).unwrap_or("file")));
    fs::copy(from, &tmp).map_err(|e| format!("cannot write {}: {e} (is the folder writable by you?)", tmp.display()))?;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(mode)).map_err(|e| e.to_string())?;
    fs::rename(&tmp, to).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("cannot replace {}: {e}", to.display())
    })
}

/// The Qt 6 version installed here (`major.minor.patch`), if it can be told.
pub fn host_qt_version() -> Option<String> {
    let run = |cmd: &str, args: &[&str]| Command::new(cmd).args(args).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|s| !s.is_empty());
    run("pkg-config", &["--modversion", "Qt6Core"]).or_else(|| run("qmake6", &["-query", "QT_VERSION"]))
}

/// Can a panel built against Qt `built` run on Qt `host`? The panel's compiled QML uses Qt's private interfaces, which a newer minor version can lack
/// (and an older one may not have yet), so the major.minor must match and the installed patch must not be older than the one it was built with.
pub fn qt_compatible(built: &str, host: &str) -> bool {
    let nums = |v: &str| -> Vec<u64> { v.trim().split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    let (b, h) = (nums(built), nums(host));
    b.len() >= 2 && h.len() >= 2 && b[0] == h[0] && b[1] == h[1] && h.get(2).copied().unwrap_or(0) >= b.get(2).copied().unwrap_or(0)
}

/// What an installation did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Installed {
    pub files: Vec<PathBuf>,
    /// Why `notify-panel` was left as it was (it was built for a different Qt than the one installed), or `None` when it was replaced.
    pub panel_skipped: Option<String>,
}

/// Unpack a verified archive and put its programs, launcher and docs under `prefix`. `work` is an empty scratch folder.
/// `host_qt` is the Qt installed here: the panel is replaced only when the archive says it was built for a compatible Qt.
pub fn install_archive(archive: &[u8], version: &str, prefix: &Path, work: &Path, host_qt: Option<&str>) -> Result<Installed, String> {
    let tgz = work.join("release.tar.gz");
    fs::write(&tgz, archive).map_err(|e| e.to_string())?;
    let unpacked = work.join("unpacked");
    fs::create_dir_all(&unpacked).map_err(|e| e.to_string())?;
    let st = Command::new("tar").arg("xzf").arg(&tgz).arg("-C").arg(&unpacked).status().map_err(|e| format!("cannot run tar: {e}"))?;
    if !st.success() {
        return Err("the archive could not be unpacked".into());
    }
    let root = fs::read_dir(&unpacked)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.is_dir() && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&format!("noti-fi-{version}-"))))
        .ok_or_else(|| format!("the archive does not contain noti-fi-{version}-*"))?;
    for bin in ["notify", "notify-panel"] {
        if !root.join("bin").join(bin).is_file() {
            return Err(format!("the archive is missing bin/{bin}"));
        }
    }
    let built_qt = fs::read_to_string(root.join("qt-version")).ok().map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    let panel_skipped = match (built_qt.as_deref(), host_qt) {
        (Some(b), Some(h)) if qt_compatible(b, h) => None,
        (Some(b), Some(h)) => Some(format!("it was built for Qt {b} but this machine has Qt {h}, and a panel only runs on the Qt it was built for")),
        (Some(b), None) => Some(format!("it was built for Qt {b} and this machine's Qt version could not be determined")),
        (None, _) => Some("the archive does not say which Qt it was built for".to_string()),
    };
    let mut done = Installed { panel_skipped, ..Default::default() };
    for bin in ["notify", "notify-panel"] {
        if bin == "notify-panel" && done.panel_skipped.is_some() {
            continue;
        }
        let to = prefix.join("bin").join(bin);
        replace_file(&root.join("bin").join(bin), &to, 0o755)?;
        done.files.push(to);
    }
    let desktop = root.join("share/applications/org.notify.Panel.desktop");
    if desktop.is_file() {
        let to = prefix.join("share/applications/org.notify.Panel.desktop");
        replace_file(&desktop, &to, 0o644)?;
        done.files.push(to);
    }
    Ok(done)
}

#[cfg(test)]
mod tests;
