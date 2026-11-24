//! The engine plus what Torrz keeps on top of it: categories, speed limits and their alternates.
//! Runs every [`Command`] that is not about the window.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::command::{Command, Response, Target};
use crate::engine::Engine;
use crate::model::{Snapshot, Torrent, parse_rate};

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
            Command::Add { source, paused, dir, category } => {
                let id = self.engine.add(source, *paused, dir.as_deref())?;
                if let Some(cat) = category {
                    if let Some(t) = torrents().into_iter().find(|t| t.id == id) {
                        self.cfg().categories.insert(t.hash, cat.clone());
                        self.save();
                    }
                }
                Response::data(format!("added #{id}"), serde_json::json!({ "id": id }))
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
