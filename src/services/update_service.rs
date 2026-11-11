//! Checks for a newer Umbra release and installs it. An interactive session checks once a day on
//! a background thread and remembers the answer in `~/.umbra/update.json`. When a newer release
//! exists the fullscreen view offers to install it. Installing downloads the release archive for
//! this machine, checks it against the release's `SHA256SUMS`, and replaces the running binary.
//! A binary installed by Homebrew is upgraded with `brew` instead.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::models::config::UpdatesConfig;
use crate::utils::http::{get_text, http_agent};
use crate::utils::paths::umbra_home;

/// Where the releases are listed. `UMBRA_TEST_UPDATE_URL` replaces it in tests.
const RELEASES: &str = "https://api.github.com/repos/Black-Rainbow-Labs/umbra/releases";

/// This build's version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What `~/.umbra/update.json` remembers between sessions.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct UpdateState {
    /// Seconds since 1970 of the last check.
    pub checked_at: u64,
    /// The newest version the last check found.
    pub latest: String,
    /// A version the person chose to skip.
    pub skipped: String,
    /// Seconds since 1970 before which the prompt is not shown again ("remind me tomorrow").
    pub remind_after: u64,
}

/// A release: its version and its files.
#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub version: String,
    /// Each file's name and download address.
    pub assets: Vec<(String, String)>,
}

/// The version the fullscreen view offers to install, once a check found one.
static AVAILABLE: Mutex<Option<String>> = Mutex::new(None);

/// Seconds since 1970.
fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The file the state is kept in.
fn state_file() -> PathBuf {
    umbra_home().join("update.json")
}

/// Reads the state, or the default when there is none.
pub fn load_state() -> UpdateState {
    std::fs::read_to_string(state_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Writes the state.
/// @param state - the state
pub fn save_state(state: &UpdateState) {
    let file = state_file();
    if let Some(folder) = file.parent() {
        let _ = std::fs::create_dir_all(folder);
    }
    if let Ok(text) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(file, text);
    }
}

/// Reads `1.2.3` or `v1.2.3` into numbers. Anything after a `-` or `+` is ignored.
/// @param text - the version
pub fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let core = text.trim().trim_start_matches('v').split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    Some((parts.next()??, parts.next().flatten().unwrap_or(0), parts.next().flatten().unwrap_or(0)))
}

/// True when `candidate` is a later version than `current`.
/// @param candidate - the version found
/// @param current - the version running
pub fn is_newer(candidate: &str, current: &str) -> bool {
    matches!((parse_version(candidate), parse_version(current)), (Some(a), Some(b)) if a > b)
}

/// True when the last check is older than the interval.
/// @param state - the remembered state
/// @param settings - the `[updates]` settings
/// @param now - seconds since 1970
pub fn due(state: &UpdateState, settings: &UpdatesConfig, now: u64) -> bool {
    now.saturating_sub(state.checked_at) >= settings.interval_hours.max(1) * 3600
}

/// The version to offer, when the state names one newer than this build that was not skipped
/// and is not waiting for "remind me tomorrow".
/// @param state - the remembered state
/// @param now - seconds since 1970
pub fn offer(state: &UpdateState, now: u64) -> Option<String> {
    (is_newer(&state.latest, VERSION) && state.latest != state.skipped && now >= state.remind_after).then(|| state.latest.clone())
}

/// The address the releases are read from.
fn releases_url() -> String {
    std::env::var("UMBRA_TEST_UPDATE_URL").unwrap_or_else(|_| RELEASES.to_string())
}

/// Reads a release from the GitHub releases API: `latest`, or the tag `v<version>`.
/// @param version - None for the newest release
pub fn fetch_release(version: Option<&str>) -> Result<Release> {
    let path = match version {
        Some(v) => format!("tags/v{}", v.trim_start_matches('v')),
        None => "latest".to_string(),
    };
    let url = format!("{}/{path}", releases_url().trim_end_matches('/'));
    let headers = vec![("User-Agent".to_string(), format!("umbra/{VERSION}")), ("Accept".to_string(), "application/vnd.github+json".to_string())];
    let reply = get_text(&http_agent(10), &url, &headers).map_err(|e| anyhow!("could not reach {url}: {e}"))?;
    if reply.status != 200 {
        bail!("{url} answered HTTP {}", reply.status);
    }
    let json: Value = serde_json::from_str(&reply.body).context("the release list is not JSON")?;
    let tag = json["tag_name"].as_str().ok_or_else(|| anyhow!("the release has no tag_name"))?;
    let assets = json["assets"].as_array().map(|a| a.iter().filter_map(|asset| Some((asset["name"].as_str()?.to_string(), asset["browser_download_url"].as_str()?.to_string()))).collect()).unwrap_or_default();
    Ok(Release { version: tag.trim_start_matches('v').to_string(), assets })
}

/// Checks for a newer release now and remembers the answer. Returns the newest version.
pub fn check_now() -> Result<String> {
    let release = fetch_release(None)?;
    let mut state = load_state();
    state.checked_at = now_secs();
    state.latest = release.version.clone();
    save_state(&state);
    Ok(release.version)
}

/// Starts the daily check for an interactive session: on a background thread when one is due,
/// and offers the version the state already names when it is newer. Does nothing when the
/// `[updates]` settings or `UMBRA_UPDATE_CHECK=0` turn it off.
/// @param settings - the `[updates]` settings
pub fn start_background_check(settings: &UpdatesConfig) {
    if !settings.check || std::env::var("UMBRA_UPDATE_CHECK").is_ok_and(|v| v == "0") {
        return;
    }
    let state = load_state();
    if let Some(version) = offer(&state, now_secs()) {
        set_available(Some(version));
    }
    if !due(&state, settings, now_secs()) {
        return;
    }
    std::thread::spawn(|| {
        if check_now().is_ok() {
            if let Some(version) = offer(&load_state(), now_secs()) {
                set_available(Some(version));
            }
        }
    });
}

/// Sets the version the view offers and asks for a redraw.
/// @param version - the version, or None to offer nothing
pub fn set_available(version: Option<String>) {
    *AVAILABLE.lock().unwrap_or_else(|e| e.into_inner()) = version;
    crate::utils::screen::request_redraw();
}

/// The version the view offers to install, if any.
pub fn available() -> Option<String> {
    AVAILABLE.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// "Skip this version": the version is not offered again.
/// @param version - the version
pub fn skip(version: &str) {
    let mut state = load_state();
    state.skipped = version.to_string();
    save_state(&state);
    set_available(None);
}

/// "Remind me tomorrow": the prompt is not shown again for a day.
pub fn remind_later() {
    let mut state = load_state();
    state.remind_after = now_secs() + 24 * 3600;
    save_state(&state);
    set_available(None);
}

/// The release target this binary was built for, as the release archives name it.
pub fn target_triple() -> Result<&'static str> {
    Ok(match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "x86_64") => "x86_64-unknown-linux-musl",
        ("linux", "aarch64") => "aarch64-unknown-linux-musl",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        (os, arch) => bail!("there is no Umbra release for {os} on {arch}"),
    })
}

/// The archive name for this machine, such as `umbra-aarch64-apple-darwin.tar.gz`.
pub fn archive_name() -> Result<String> {
    let extension = if cfg!(windows) { "zip" } else { "tar.gz" };
    Ok(format!("umbra-{}.{extension}", target_triple()?))
}

/// The binary an install replaces: `UMBRA_UPDATE_TARGET` in tests, else the running binary.
pub fn install_target() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("UMBRA_UPDATE_TARGET") {
        return Ok(PathBuf::from(path));
    }
    let exe = std::env::current_exe().context("the running binary's path is not known")?;
    Ok(std::fs::canonicalize(&exe).unwrap_or(exe))
}

/// True when Homebrew installed a binary.
/// @param exe - the binary
fn is_homebrew(exe: &Path) -> bool {
    let text = exe.to_string_lossy();
    text.contains("/Cellar/") || text.contains("/homebrew/")
}

/// Installs a release over the running binary, or with `brew upgrade` for a Homebrew install.
/// Returns the version installed. A failure leaves the old binary as it was.
/// @param version - the version to install, or None for the newest
/// @param progress - called with a line for each step
pub fn install(version: Option<&str>, progress: &mut dyn FnMut(&str)) -> Result<String> {
    let exe = install_target()?;
    if is_homebrew(&exe) && std::env::var("UMBRA_UPDATE_TARGET").is_err() {
        progress("upgrading with Homebrew");
        let status = Command::new("brew").args(["upgrade", "black-rainbow-labs/umbra/umbra"]).status().context("could not run brew")?;
        if !status.success() {
            bail!("brew upgrade failed with {status}");
        }
        return Ok(version.unwrap_or("the newest version").to_string());
    }
    let release = fetch_release(version)?;
    let name = archive_name()?;
    let url = asset_url(&release, &name)?;
    let sums_url = asset_url(&release, "SHA256SUMS")?;
    progress(&format!("downloading {name} for {}", release.version));
    let archive = download(&url)?;
    let sums = String::from_utf8(download(&sums_url)?).context("SHA256SUMS is not text")?;
    check_sum(&archive, &sums, &name)?;
    progress("checksum matches SHA256SUMS");
    let work = exe.parent().unwrap_or(Path::new(".")).join(format!(".umbra-update-{}", std::process::id()));
    let result = unpack_and_replace(&archive, &name, &work, &exe, &release.version);
    let _ = std::fs::remove_dir_all(&work);
    result?;
    progress(&format!("installed {} at {}", release.version, exe.display()));
    Ok(release.version)
}

/// The download address of a release file.
/// @param release - the release
/// @param name - the file name
fn asset_url(release: &Release, name: &str) -> Result<String> {
    release.assets.iter().find(|(n, _)| n == name).map(|(_, u)| u.clone()).ok_or_else(|| anyhow!("release {} has no {name}", release.version))
}

/// Downloads a file into memory, at most 256 MB.
/// @param url - the address
fn download(url: &str) -> Result<Vec<u8>> {
    let mut response = http_agent(120).get(url).header("User-Agent", format!("umbra/{VERSION}")).call().map_err(|e| anyhow!("could not download {url}: {e}"))?;
    if response.status().as_u16() != 200 {
        bail!("{url} answered HTTP {}", response.status().as_u16());
    }
    response.body_mut().with_config().limit(256 * 1024 * 1024).read_to_vec().map_err(|e| anyhow!("could not read {url}: {e}"))
}

/// Checks an archive's SHA-256 against its line in `SHA256SUMS`.
/// @param archive - the archive's bytes
/// @param sums - the text of `SHA256SUMS`
/// @param name - the archive's name
pub fn check_sum(archive: &[u8], sums: &str, name: &str) -> Result<()> {
    let expected = sums.lines().find_map(|line| {
        let (hash, file) = line.split_once(char::is_whitespace)?;
        (file.trim().trim_start_matches('*') == name).then(|| hash.trim().to_lowercase())
    }).ok_or_else(|| anyhow!("SHA256SUMS has no line for {name}"))?;
    let actual: String = Sha256::digest(archive).iter().map(|b| format!("{b:02x}")).collect();
    if actual != expected {
        bail!("{name} does not match SHA256SUMS (expected {expected}, got {actual})");
    }
    Ok(())
}

/// Unpacks the archive with the system `tar`, checks the new binary runs and reports the
/// expected version, and puts it in place of the old one.
/// @param archive - the archive's bytes
/// @param name - the archive's name
/// @param work - a folder for the download, next to the binary so a rename stays on one disk
/// @param exe - the binary to replace
/// @param version - the version the new binary must report
fn unpack_and_replace(archive: &[u8], name: &str, work: &Path, exe: &Path, version: &str) -> Result<()> {
    std::fs::create_dir_all(work).with_context(|| format!("could not make {}", work.display()))?;
    let file = work.join(name);
    std::fs::write(&file, archive)?;
    let status = Command::new("tar").arg("-xf").arg(&file).arg("-C").arg(work).status().context("could not run tar")?;
    if !status.success() {
        bail!("tar could not unpack {name}");
    }
    let binary_name = if cfg!(windows) { "umbra.exe" } else { "umbra" };
    let new = find_file(work, binary_name).ok_or_else(|| anyhow!("{name} has no {binary_name}"))?;
    make_executable(&new)?;
    let output = Command::new(&new).arg("--version").output().context("the new binary does not start")?;
    let reported = String::from_utf8_lossy(&output.stdout).to_string();
    if !reported.contains(version) {
        bail!("the new binary reports {:?}, not {version}", reported.trim());
    }
    replace_binary(&new, exe)
}

/// Finds a file by name in a folder or one level below it.
/// @param dir - the folder
/// @param name - the file name
fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    let direct = dir.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path().join(name)).find(|p| p.is_file())
}

/// Sets the executable bits on Unix.
/// @param file - the binary
#[cfg(unix)]
fn make_executable(file: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o755)).context("could not make the new binary executable")
}

/// Windows needs no executable bit.
/// @param _file - the binary
#[cfg(not(unix))]
fn make_executable(_file: &Path) -> Result<()> {
    Ok(())
}

/// Puts the new binary where the old one is. On Unix a rename replaces a running binary safely.
/// On Windows the running binary is renamed to `umbra.exe.old` first, and removed at the next
/// start by `remove_old_binary`.
/// @param new - the new binary
/// @param exe - the old binary's path
fn replace_binary(new: &Path, exe: &Path) -> Result<()> {
    if cfg!(windows) {
        let old = exe.with_extension("exe.old");
        let _ = std::fs::remove_file(&old);
        std::fs::rename(exe, &old).with_context(|| format!("could not move {} aside", exe.display()))?;
        if let Err(error) = std::fs::rename(new, exe) {
            let _ = std::fs::rename(&old, exe);
            return Err(error).with_context(|| format!("could not put the new binary at {}", exe.display()));
        }
        return Ok(());
    }
    std::fs::rename(new, exe).with_context(|| format!("could not put the new binary at {}", exe.display()))
}

/// Removes the `umbra.exe.old` a Windows update left behind.
pub fn remove_old_binary() {
    if cfg!(windows) {
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::fs::remove_file(exe.with_extension("exe.old"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(is_newer("v0.10.0", "0.9.9"));
        assert!(is_newer("1.0.0", "0.6.2"));
        assert!(!is_newer("0.6.2", "0.6.2"));
        assert!(!is_newer("0.6.1", "0.6.2"));
        assert!(!is_newer("garbage", "0.6.2"));
        assert_eq!(parse_version("0.7.0-beta.1"), Some((0, 7, 0)));
    }

    #[test]
    fn a_check_is_due_once_a_day_and_an_offer_honors_skip_and_remind() {
        let settings = UpdatesConfig::default();
        let state = UpdateState { checked_at: 1000, latest: "99.0.0".into(), ..Default::default() };
        assert!(!due(&state, &settings, 1000 + 3600));
        assert!(due(&state, &settings, 1000 + 24 * 3600));
        assert_eq!(offer(&state, 2000), Some("99.0.0".into()));
        assert_eq!(offer(&UpdateState { skipped: "99.0.0".into(), ..state.clone() }, 2000), None);
        assert_eq!(offer(&UpdateState { remind_after: 5000, ..state.clone() }, 2000), None);
        assert_eq!(offer(&UpdateState { latest: VERSION.into(), ..state }, 2000), None);
    }

    #[test]
    fn the_checksum_must_match_its_line() {
        let data = b"hello";
        let hash = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        assert!(check_sum(data, &format!("{hash}  umbra-x.tar.gz\nffff  other\n"), "umbra-x.tar.gz").is_ok());
        assert!(check_sum(b"changed", &format!("{hash}  umbra-x.tar.gz\n"), "umbra-x.tar.gz").is_err());
        assert!(check_sum(data, "ffff  other\n", "umbra-x.tar.gz").is_err());
    }
}
