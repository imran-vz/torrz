//! The engine plus what Torrz keeps on top of it: categories, speed limits and their alternates.
//! Runs every [`Command`] that is not about the window.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::command::{Command, Response, Target};
use crate::engine::Engine;
use crate::model::{RankedTracker, Snapshot, Torrent, parse_rate};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub limit_down: Option<u64>,
    pub limit_up: Option<u64>,
    pub alt_down: Option<u64>,
    pub alt_up: Option<u64>,
    pub alt_on: bool,
    /// Info hash to category.
    pub categories: HashMap<String, String>,
    /// The app icon the user chose: `auto`, `light` or `dark` (empty means `auto`).
    pub app_icon: String,
    /// The folder watched for new .torrent files.
    pub watch_dir: Option<String>,
    /// Move a watched .torrent file to the Trash once it is added.
    pub watch_trash: bool,
    /// Names of the .torrent files in the watched folder that were already there or already
    /// added, so they are not added again (after a restart, say). Names that leave the folder
    /// leave the list.
    pub watch_seen: Vec<String>,
    /// Give new torrents the best public trackers.
    pub auto_trackers: bool,
    /// The best public trackers when they were last ranked, and when that was (Unix seconds).
    pub best_trackers: Vec<RankedTracker>,
    pub best_trackers_at: u64,
}

/// How long a ranking of the public trackers is used before they are probed again.
const TRACKERS_FRESH_SECS: u64 = 6 * 3600;

/// What the demo is given instead of probing the network.
fn demo_trackers() -> Vec<RankedTracker> {
    [("udp://tracker.opentrackr.org:1337/announce", 18), ("udp://open.stealth.si:80/announce", 31), ("udp://tracker.torrent.eu.org:451/announce", 44), ("udp://exodus.desync.com:6969/announce", 52), ("http://tracker.mywaifu.best:6969/announce", 70)]
        .into_iter()
        .map(|(url, ms)| RankedTracker { url: url.into(), ms })
        .collect()
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Config {
    fn fresh() -> Self {
        Self { alt_down: Some(1024 * 1024), alt_up: Some(256 * 1024), ..Default::default() }
    }
}

pub struct Backend {
    engine: Arc<dyn Engine>,
    config: Mutex<Config>,
    /// `None` keeps the config in memory only (the demo and tests).
    path: Option<PathBuf>,
}

/// `~/Library/Application Support/Torrz` on macOS, `$XDG_DATA_HOME/torrz` elsewhere.
pub fn app_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("TORRZ_HOME") {
        return d.into();
    }
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Torrz")
    } else if let Some(x) = std::env::var_os("XDG_DATA_HOME") {
        PathBuf::from(x).join("torrz")
    } else {
        home.join(".local/share/torrz")
    }
}

pub fn default_download_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Downloads")
}

/// `~/x` as `$HOME/x`; anything else unchanged.
pub fn expand_home(p: &str) -> PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(rest),
        None if p == "~" => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()),
        None => p.into(),
    }
}

/// The local file an `add` source names, or `None` for a magnet link or a URL.
pub fn local_file(source: &str) -> Option<PathBuf> {
    let s = source.trim();
    if s.is_empty() || s.contains("://") || s.get(..7).is_some_and(|p| p.eq_ignore_ascii_case("magnet:")) {
        return None;
    }
    Some(expand_home(s))
}

/// The .torrent files directly in `dir` (not hidden ones), each with whether it has stopped
/// changing: a file written in the last second may still be arriving. `None` when the folder
/// can't be read.
fn torrent_files(dir: &Path) -> Option<Vec<(String, PathBuf, bool)>> {
    let now = std::time::SystemTime::now();
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let path = e.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()).map(str::to_owned) else { continue };
        if name.starts_with('.') || !path.extension().is_some_and(|x| x.eq_ignore_ascii_case("torrent")) {
            continue;
        }
        let Ok(meta) = e.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let settled = meta.len() > 0 && meta.modified().ok().and_then(|m| now.duration_since(m).ok()).is_some_and(|age| age.as_secs_f32() >= 1.0);
        out.push((name, path, settled));
    }
    out.sort();
    Some(out)
}

#[cfg(unix)]
fn free_disk(dir: &str) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(Path::new(dir).as_os_str().as_bytes()).ok()?;
    let mut s = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: `c` is a valid NUL-terminated path and `s` is a writable statvfs; it is read only
    // after statvfs reports success, which means it filled the struct.
    let s = unsafe {
        if libc::statvfs(c.as_ptr(), s.as_mut_ptr()) != 0 {
            return None;
        }
        s.assume_init()
    };
    Some(s.f_bavail as u64 * s.f_frsize as u64)
}

#[cfg(not(unix))]
fn free_disk(_: &str) -> Option<u64> {
    None
}

impl Backend {
    pub fn new(engine: Arc<dyn Engine>, config_path: Option<PathBuf>) -> Self {
        let config = config_path
            .as_deref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_else(Config::fresh);
        let b = Self { engine, config: Mutex::new(config), path: config_path };
        b.apply_limits();
        b
    }

    fn cfg(&self) -> std::sync::MutexGuard<'_, Config> {
        self.config.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn save(&self) {
        let Some(p) = &self.path else { return };
        let json = serde_json::to_vec_pretty(&*self.cfg()).unwrap_or_default();
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, p);
        }
    }

    fn apply_limits(&self) {
        let c = self.cfg().clone();
        if c.alt_on { self.engine.set_limits(c.alt_down, c.alt_up) } else { self.engine.set_limits(c.limit_down, c.limit_up) }
    }

    /// The app icon the user chose: `auto`, `light` or `dark`.
    pub fn app_icon(&self) -> String {
        let c = self.cfg();
        if c.app_icon.is_empty() { "auto".into() } else { c.app_icon.clone() }
    }

    pub fn set_app_icon(&self, style: &str) {
        self.cfg().app_icon = style.to_owned();
        self.save();
    }

    /// The .torrent files that arrived in the watched folder since the last call, and whether
    /// to move them to the Trash once added. Each file is handed out once, whatever happens to
    /// it next, so a file that fails to add is not retried every few seconds.
    pub fn take_watched(&self) -> (Vec<PathBuf>, bool) {
        let Some(dir) = self.cfg().watch_dir.clone() else { return (Vec::new(), false) };
        // A folder that can't be read (a drive that went away) keeps what was seen in it.
        let Some(present) = torrent_files(Path::new(&dir)) else { return (Vec::new(), false) };
        let mut c = self.cfg();
        if c.watch_dir.as_deref() != Some(dir.as_str()) {
            return (Vec::new(), false);
        }
        let before = c.watch_seen.len();
        c.watch_seen.retain(|n| present.iter().any(|(name, ..)| name == n));
        let mut changed = c.watch_seen.len() != before;
        let mut new = Vec::new();
        for (name, path, settled) in present {
            if settled && !c.watch_seen.contains(&name) {
                c.watch_seen.push(name);
                new.push(path);
                changed = true;
            }
        }
        let trash = c.watch_trash;
        drop(c);
        if changed {
            self.save();
        }
        (new, trash)
    }

    /// The best public trackers: the ranking from the last few hours, or a new one when that
    /// is stale or `refresh` asks. Blocks on the network while it ranks, so never on the UI thread.
    pub fn best_trackers(&self, refresh: bool) -> anyhow::Result<Vec<RankedTracker>> {
        if self.engine.offline() {
            return Ok(demo_trackers());
        }
        let now = unix_now();
        {
            let c = self.cfg();
            if !refresh && !c.best_trackers.is_empty() && now.saturating_sub(c.best_trackers_at) < TRACKERS_FRESH_SECS {
                return Ok(c.best_trackers.clone());
            }
        }
        let best = crate::trackers::best()?;
        {
            let mut c = self.cfg();
            c.best_trackers = best.clone();
            c.best_trackers_at = now;
        }
        self.save();
        Ok(best)
    }

    pub fn snapshot(&self, detail: Option<u64>) -> Snapshot {
        let mut s = self.engine.snapshot(detail);
        let c = self.cfg().clone();
        for t in &mut s.torrents {
            if let Some(cat) = c.categories.get(&t.hash) {
                t.category = (!cat.is_empty()).then(|| cat.clone());
            }
        }
        s.session.alt_limits = c.alt_on;
        (s.session.limit_down, s.session.limit_up) = if c.alt_on { (c.alt_down, c.alt_up) } else { (c.limit_down, c.limit_up) };
        s.session.watch_dir = c.watch_dir.clone();
        s.session.watch_trash = c.watch_trash;
        s.session.auto_trackers = c.auto_trackers;
        if s.session.free_disk.is_none() {
            s.session.free_disk = free_disk(&s.session.download_dir);
        }
        s
    }

    /// Turns targets (`3`, an info hash, `all`, `selected`) into ids that exist.
    pub fn resolve(torrents: &[Torrent], targets: &[Target], selected: &[u64]) -> anyhow::Result<Vec<u64>> {
        anyhow::ensure!(!targets.is_empty(), "name a torrent: an id, an info hash, `all` or `selected`");
        let mut out = Vec::new();
        for t in targets {
            match t.as_str() {
                "all" => out.extend(torrents.iter().map(|t| t.id)),
                "selected" => {
                    anyhow::ensure!(!selected.is_empty(), "nothing is selected");
                    out.extend_from_slice(selected);
                }
                s => {
                    let found = torrents.iter().find(|x| s.parse::<u64>().is_ok_and(|n| n == x.id) || x.hash.eq_ignore_ascii_case(s));
                    out.push(found.ok_or_else(|| anyhow::anyhow!("no torrent {s:?}"))?.id);
                }
            }
        }
        let mut seen = std::collections::HashSet::new();
        out.retain(|id| seen.insert(*id));
        Ok(out)
    }

    /// Runs a command that isn't about the window. `selected` is what `selected` means right now.
    pub fn exec(&self, cmd: &Command, selected: &[u64]) -> Response {
        match self.try_exec(cmd, selected) {
            Ok(r) => r,
            Err(e) => Response::err(format!("{e:#}")),
        }
    }

    fn try_exec(&self, cmd: &Command, selected: &[u64]) -> anyhow::Result<Response> {
        let torrents = || self.snapshot(None).torrents;
        let ids = |t: &[Target]| Self::resolve(&torrents(), t, selected);
        let each = |ids: Vec<u64>, verb: &str, f: &dyn Fn(u64) -> anyhow::Result<()>| -> anyhow::Result<Response> {
            for id in &ids {
                f(*id)?;
            }
            Ok(Response::data(format!("{verb} {}", plural(ids.len())), &ids))
        };
        Ok(match cmd {
            Command::List => Response::data("", torrents()),
            Command::Stats => Response::data("", self.snapshot(None).session),
            Command::Show { target } => {
                let id = ids(std::slice::from_ref(target))?[0];
                let s = self.snapshot(Some(id));
                let t = s.torrents.into_iter().find(|t| t.id == id);
                Response::data("", serde_json::json!({ "torrent": t, "detail": s.detail }))
            }
            Command::Add { source, paused, dir, category, trash } => {
                let file = match trash {
                    true => Some(local_file(source).filter(|p| p.is_file()).ok_or_else(|| anyhow::anyhow!("only a .torrent file can go to the Trash, not {:?}", source.trim()))?),
                    false => None,
                };
                // A list that can't be had right now only means the torrent keeps its own trackers.
                let extra: Vec<String> = match self.cfg().auto_trackers {
                    true => self.best_trackers(false).map(|b| b.into_iter().map(|t| t.url).collect()).unwrap_or_default(),
                    false => Vec::new(),
                };
                let id = self.engine.add(source, *paused, dir.as_deref(), &extra)?;
                if let Some(cat) = category
                    && let Some(t) = torrents().into_iter().find(|t| t.id == id)
                {
                    self.cfg().categories.insert(t.hash, cat.clone());
                    self.save();
                }
                let mut msg = format!("added #{id}");
                let mut trashed = false;
                if let Some(f) = file {
                    let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    match trash::delete(&f) {
                        Ok(()) => {
                            trashed = true;
                            msg += &format!(" and moved {name} to the Trash");
                        }
                        Err(e) => msg += &format!(", but couldn't move {name} to the Trash: {e}"),
                    }
                }
                Response::data(msg, serde_json::json!({ "id": id, "trashed": trashed }))
            }
            Command::Pause { targets } => each(ids(targets)?, "paused", &|id| self.engine.pause(id))?,
            Command::Resume { targets } => each(ids(targets)?, "resumed", &|id| self.engine.resume(id))?,
            Command::Remove { targets, delete_files } => {
                let ts = torrents();
                let ids = Self::resolve(&ts, targets, selected)?;
                let r = each(ids.clone(), if *delete_files { "removed with files" } else { "removed" }, &|id| self.engine.remove(id, *delete_files))?;
                let mut c = self.cfg();
                for t in ts.iter().filter(|t| ids.contains(&t.id)) {
                    c.categories.remove(&t.hash);
                }
                drop(c);
                self.save();
                r
            }
            Command::Files { target, include, skip } => {
                let id = ids(std::slice::from_ref(target))?[0];
                let files = self.snapshot(Some(id)).detail.map(|d| d.files).unwrap_or_default();
                anyhow::ensure!(!files.is_empty(), "torrent {id} has no file list yet");
                if let Some(bad) = include.iter().chain(skip).find(|i| **i >= files.len()) {
                    anyhow::bail!("torrent {id} has files 0 to {}, not {bad}", files.len() - 1);
                }
                let only: Vec<usize> = files.iter().filter(|f| (f.included || include.contains(&f.index)) && !skip.contains(&f.index)).map(|f| f.index).collect();
                self.engine.set_files(id, &only)?;
                Response::data(format!("downloading {} of {} files", only.len(), files.len()), &only)
            }
            Command::Limit { down, up } => {
                anyhow::ensure!(down.is_some() || up.is_some(), "give --down, --up or both (12M, 800k, none)");
                let (d, u) = (down.as_deref().map(parse_rate).transpose()?, up.as_deref().map(parse_rate).transpose()?);
                let mut guard = self.cfg();
                let c = &mut *guard;
                // Limits set while the alternates are on change the alternates, as the window shows them.
                let (cd, cu) = if c.alt_on { (&mut c.alt_down, &mut c.alt_up) } else { (&mut c.limit_down, &mut c.limit_up) };
                if let Some(d) = d {
                    *cd = d;
                }
                if let Some(u) = u {
                    *cu = u;
                }
                let msg = format!("limits: down {}, up {}", rate(*cd), rate(*cu));
                drop(guard);
                self.apply_limits();
                self.save();
                Response::ok(msg)
            }
            Command::AltLimits { on } => {
                let now = {
                    let mut c = self.cfg();
                    c.alt_on = on.unwrap_or(!c.alt_on);
                    c.alt_on
                };
                self.apply_limits();
                self.save();
                Response::data(format!("alternate limits {}", if now { "on" } else { "off" }), now)
            }
            Command::Category { targets, name } => {
                let ts = torrents();
                let ids = Self::resolve(&ts, targets, selected)?;
                let mut c = self.cfg();
                for t in ts.iter().filter(|t| ids.contains(&t.id)) {
                    // An empty entry overrides a category the engine itself reports.
                    c.categories.insert(t.hash.clone(), name.clone().unwrap_or_default());
                }
                drop(c);
                self.save();
                Response::ok(match name {
                    Some(n) => format!("{} now in {n}", plural(ids.len())),
                    None => format!("cleared the category of {}", plural(ids.len())),
                })
            }
            Command::Reveal { target } => {
                let ts = torrents();
                let id = Self::resolve(&ts, std::slice::from_ref(target), selected)?[0];
                let t = ts.iter().find(|t| t.id == id).ok_or_else(|| anyhow::anyhow!("no torrent {id}"))?;
                Response::data("", serde_json::json!({ "path": Path::new(&t.save_path).join(&t.name) }))
            }
            Command::Watch { dir, off, trash } => {
                let dir = match dir {
                    Some(d) => {
                        let p = expand_home(d.trim());
                        anyhow::ensure!(p.is_dir(), "{} is not a folder", p.display());
                        Some(p.canonicalize().unwrap_or(p))
                    }
                    None => None,
                };
                let mut c = self.cfg();
                let changed = *off || dir.is_some() || trash.is_some();
                if *off {
                    c.watch_dir = None;
                    c.watch_seen.clear();
                }
                if let Some(p) = dir {
                    let p = p.display().to_string();
                    if c.watch_dir.as_deref() != Some(p.as_str()) {
                        // What is in the folder already stays put; only what arrives is added.
                        c.watch_seen = torrent_files(Path::new(&p)).unwrap_or_default().into_iter().map(|(name, ..)| name).collect();
                        c.watch_dir = Some(p);
                    }
                }
                if let Some(t) = trash {
                    c.watch_trash = *t;
                }
                let data = serde_json::json!({ "dir": c.watch_dir, "trash": c.watch_trash });
                let msg = match &c.watch_dir {
                    Some(d) => format!("watching {d} for .torrent files{}", if c.watch_trash { ", then moving them to the Trash" } else { "" }),
                    None => "not watching a folder".into(),
                };
                drop(c);
                if changed {
                    self.save();
                }
                Response::data(msg, data)
            }
            Command::BestTrackers { refresh } => {
                let best = self.best_trackers(*refresh)?;
                let msg = format!("{} trackers answered, the fastest in {} ms", best.len(), best.first().map(|t| t.ms).unwrap_or(0));
                Response::data(msg, best)
            }
            Command::AddTrackers { targets, urls } => {
                let ts = torrents();
                let ids = Self::resolve(&ts, targets, selected)?;
                let urls: Vec<String> = if urls.is_empty() {
                    self.best_trackers(false)?.into_iter().map(|t| t.url).collect()
                } else {
                    if let Some(bad) = urls.iter().find(|u| crate::trackers::scheme(u).is_none() || crate::trackers::host(u).is_none()) {
                        anyhow::bail!("{bad:?} is not a tracker URL (udp://, http:// or https://)");
                    }
                    urls.clone()
                };
                let (mut moved, mut skipped, mut added) = (Vec::new(), Vec::new(), 0);
                for id in ids {
                    if ts.iter().any(|t| t.id == id && t.private) {
                        skipped.push(format!("#{id} is private"));
                        continue;
                    }
                    match self.engine.add_trackers(id, &urls) {
                        Ok((now, n)) => {
                            added += n;
                            moved.push(serde_json::json!({ "id": id, "now": now, "added": n }));
                        }
                        Err(e) => skipped.push(format!("{e:#}")),
                    }
                }
                anyhow::ensure!(!moved.is_empty(), "{}", skipped.join("; "));
                let mut msg = match added {
                    0 => format!("{} already had these trackers", plural(moved.len())),
                    n => format!("added {n} tracker{} to {}", if n == 1 { "" } else { "s" }, plural(moved.len())),
                };
                if !skipped.is_empty() {
                    msg += &format!("; skipped {}", skipped.join("; "));
                }
                Response::data(msg, moved)
            }
            Command::AutoTrackers { on } => {
                let now = {
                    let mut c = self.cfg();
                    c.auto_trackers = on.unwrap_or(!c.auto_trackers);
                    c.auto_trackers
                };
                self.save();
                if !now {
                    return Ok(Response::data("new torrents keep only their own trackers", false));
                }
                // Ranked now, so the next add doesn't wait for it.
                Response::data(
                    match self.best_trackers(false) {
                        Ok(b) => format!("new torrents get the {} best public trackers", b.len()),
                        Err(e) => format!("new torrents get the best public trackers (none to hand yet: {e:#})"),
                    },
                    true,
                )
            }
            Command::Ui(_) => anyhow::bail!("that command needs the Torrz window"),
        })
    }
}

fn plural(n: usize) -> String {
    if n == 1 { "1 torrent".into() } else { format!("{n} torrents") }
}

fn rate(r: Option<u64>) -> String {
    r.map(crate::model::speed).unwrap_or_else(|| "unlimited".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::demo::DemoEngine;
    use crate::model::TorrentState;

    fn backend() -> Backend {
        Backend::new(Arc::new(DemoEngine::frozen()), None)
    }

    #[test]
    fn targets_resolve_and_bad_ones_say_so() {
        let b = backend();
        let ts = b.snapshot(None).torrents;
        assert_eq!(Backend::resolve(&ts, &["2".into(), "selected".into(), "2".into()], &[5]).unwrap(), [2, 5]);
        assert_eq!(Backend::resolve(&ts, &["all".into()], &[]).unwrap().len(), 12);
        assert!(Backend::resolve(&ts, &["selected".into()], &[]).is_err());
        assert!(Backend::resolve(&ts, &["77".into()], &[]).is_err());
    }

    #[test]
    fn pause_all_then_show() {
        let b = backend();
        let r = b.exec(&Command::Pause { targets: vec!["all".into()] }, &[]);
        assert!(r.ok, "{}", r.message);
        assert!(b.snapshot(None).torrents.iter().all(|t| matches!(t.state, TorrentState::Paused | TorrentState::Completed)));
    }

    #[test]
    fn limits_and_alternates() {
        let b = backend();
        assert!(b.exec(&Command::Limit { down: Some("10M".into()), up: None }, &[]).ok);
        assert_eq!(b.snapshot(None).session.limit_down, Some(10 << 20));
        assert!(b.exec(&Command::AltLimits { on: None }, &[]).ok);
        let s = b.snapshot(None).session;
        assert!(s.alt_limits);
        assert_eq!(s.limit_down, Some(1 << 20));
        assert!(!b.exec(&Command::Limit { down: Some("quick".into()), up: None }, &[]).ok);
    }

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("torrz-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.canonicalize().unwrap()
    }

    /// Writes a file whose modification time is far enough back to count as settled.
    fn settled_file(p: &Path) {
        std::fs::write(p, b"d4:infod4:name1:xee").unwrap();
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(5);
        std::fs::File::options().write(true).open(p).unwrap().set_modified(old).unwrap();
    }

    #[test]
    fn trash_needs_a_file() {
        let b = backend();
        let r = b.exec(&Command::Add { source: "magnet:?xt=urn:btih:abc".into(), paused: false, dir: None, category: None, trash: true }, &[]);
        assert!(!r.ok, "a magnet link has nothing to trash");
        assert_eq!(b.snapshot(None).torrents.len(), 12, "and nothing was added");
        assert_eq!(local_file("magnet:?xt=urn:btih:abc"), None);
        assert_eq!(local_file("https://example.com/a.torrent"), None);
        assert_eq!(local_file("/tmp/a.torrent"), Some(PathBuf::from("/tmp/a.torrent")));
    }

    #[test]
    fn the_watched_folder_hands_out_new_files_once() {
        let d = temp_dir("watch");
        settled_file(&d.join("old.torrent"));
        let b = backend();
        assert!(!b.exec(&Command::Watch { dir: Some(d.join("missing").display().to_string()), off: false, trash: None }, &[]).ok);
        let r = b.exec(&Command::Watch { dir: Some(d.display().to_string()), off: false, trash: Some(true) }, &[]);
        assert!(r.ok, "{}", r.message);
        assert_eq!(r.data["trash"], true);
        assert_eq!(b.take_watched(), (vec![], true), "what was there before is left alone");

        settled_file(&d.join("new.torrent"));
        std::fs::write(d.join("arriving.torrent"), b"d4:info").unwrap();
        std::fs::write(d.join("notes.txt"), b"x").unwrap();
        settled_file(&d.join(".hidden.torrent"));
        let (files, trash) = b.take_watched();
        assert_eq!(files, [d.join("new.torrent")]);
        assert!(trash);
        assert!(b.take_watched().0.is_empty(), "each file once");

        // Leaving the folder and coming back counts as arriving again.
        std::fs::remove_file(d.join("new.torrent")).unwrap();
        assert!(b.take_watched().0.is_empty());
        settled_file(&d.join("new.torrent"));
        assert_eq!(b.take_watched().0, [d.join("new.torrent")]);

        assert!(b.exec(&Command::Watch { dir: None, off: true, trash: None }, &[]).ok);
        assert_eq!(b.snapshot(None).session.watch_dir, None);
        settled_file(&d.join("later.torrent"));
        assert!(b.take_watched().0.is_empty(), "nothing is watched");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn trackers_go_to_public_torrents_and_new_ones() {
        let b = backend();
        let r = b.exec(&Command::BestTrackers { refresh: false }, &[]);
        assert!(r.ok, "{}", r.message);
        assert_eq!(r.data.as_array().map(Vec::len), Some(5), "the demo's made-up list, with no network");

        let r = b.exec(&Command::AddTrackers { targets: vec!["1".into(), "2".into()], urls: vec![] }, &[]);
        assert!(r.ok, "{}", r.message);
        let t = &b.snapshot(None).torrents;
        assert!(t[0].trackers.contains(&"tracker.opentrackr.org".to_owned()));
        assert_eq!(t[1].trackers.len(), 5);
        let again = b.exec(&Command::AddTrackers { targets: vec!["2".into()], urls: vec![] }, &[]);
        assert!(again.ok && again.message.contains("already had"), "{}", again.message);
        assert!(!b.exec(&Command::AddTrackers { targets: vec!["2".into()], urls: vec!["wss://x.example".into()] }, &[]).ok);

        assert!(b.exec(&Command::AutoTrackers { on: Some(true) }, &[]).ok);
        assert!(b.snapshot(None).session.auto_trackers);
        let r = b.exec(&Command::Add { source: "magnet:?xt=urn:btih:abc&dn=New".into(), paused: false, dir: None, category: None, trash: false }, &[]);
        assert!(r.ok, "{}", r.message);
        assert_eq!(b.snapshot(None).torrents.last().unwrap().trackers.len(), 5, "a new torrent gets them too");
    }

    #[test]
    fn files_skip_and_category() {
        let b = backend();
        let r = b.exec(&Command::Files { target: "1".into(), include: vec![3], skip: vec![1] }, &[]);
        assert!(r.ok, "{}", r.message);
        let d = b.snapshot(Some(1)).detail.unwrap();
        assert_eq!(d.files.iter().map(|f| f.included).collect::<Vec<_>>(), [true, false, true, true]);
        assert!(!b.exec(&Command::Files { target: "1".into(), include: vec![40], skip: vec![] }, &[]).ok);
        assert!(b.exec(&Command::Category { targets: vec!["1".into()], name: Some("Keep".into()) }, &[]).ok);
        assert_eq!(b.snapshot(None).torrents[0].category.as_deref(), Some("Keep"));
        assert!(b.exec(&Command::Category { targets: vec!["1".into()], name: None }, &[]).ok);
        assert_eq!(b.snapshot(None).torrents[0].category, None);
    }
}
