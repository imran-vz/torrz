//! The window's state and the one way it changes: [`dispatch`]. Clicks, keys, palette rows and
//! `torrz` on the command line all arrive here as a [`Command`].

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, mpsc};

use rune::{TextEdit, Tx};
use serde_json::json;
use torrz_core::model::speed;
use torrz_core::{Backend, Command, Response, Snapshot, Torrent, TorrentState, Ui};

use crate::theme::Icon;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Ui,
    Cli,
}

/// Work for the engine thread, which may block on the network or the disk.
pub enum Job {
    Run { cmd: Command, selected: Vec<u64>, origin: Origin },
    Refresh,
}

/// Where commands for the engine go.
pub enum Exec {
    /// Run them right here (tests and headless snapshots, where the engine never blocks).
    Inline,
    Worker(mpsc::Sender<Job>),
}

/// What other threads need from the window without asking it.
#[derive(Default)]
pub struct Shared {
    pub selected: Mutex<Vec<u64>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Filter {
    All,
    Downloading,
    Seeding,
    Completed,
    Paused,
    Errors,
    Category(String),
    /// A tracker's host, or the empty string for torrents with no tracker (DHT only).
    Tracker(String),
}

impl Filter {
    pub fn name(&self) -> String {
        match self {
            Filter::All => "all".into(),
            Filter::Downloading => "downloading".into(),
            Filter::Seeding => "seeding".into(),
            Filter::Completed => "completed".into(),
            Filter::Paused => "paused".into(),
            Filter::Errors => "errors".into(),
            Filter::Category(c) => format!("category:{c}"),
            Filter::Tracker(t) if t.is_empty() => "tracker:dht".into(),
            Filter::Tracker(t) => format!("tracker:{t}"),
        }
    }

    pub fn title(&self) -> String {
        match self {
            Filter::All => "All Torrents".into(),
            Filter::Downloading => "Downloading".into(),
            Filter::Seeding => "Seeding".into(),
            Filter::Completed => "Completed".into(),
            Filter::Paused => "Paused".into(),
            Filter::Errors => "Errors".into(),
            Filter::Category(c) => c.clone(),
            Filter::Tracker(t) if t.is_empty() => "DHT only".into(),
            Filter::Tracker(t) => t.clone(),
        }
    }

    pub fn parse(s: &str) -> anyhow::Result<Filter> {
        Ok(match s.to_ascii_lowercase().as_str() {
            "all" => Filter::All,
            "downloading" => Filter::Downloading,
            "seeding" => Filter::Seeding,
            "completed" => Filter::Completed,
            "paused" => Filter::Paused,
            "errors" | "error" => Filter::Errors,
            "tracker:dht" => Filter::Tracker(String::new()),
            _ => match s.split_once(':') {
                Some((k, v)) if k.eq_ignore_ascii_case("category") && !v.is_empty() => Filter::Category(v.into()),
                Some((k, v)) if k.eq_ignore_ascii_case("tracker") && !v.is_empty() => Filter::Tracker(v.into()),
                _ => anyhow::bail!("no filter {s:?}: all, downloading, seeding, completed, paused, errors, category:NAME or tracker:HOST"),
            },
        })
    }

    pub fn matches(&self, t: &Torrent) -> bool {
        use TorrentState::*;
        match self {
            Filter::All => true,
            Filter::Downloading => matches!(t.state, Downloading | Checking | Queued),
            Filter::Seeding => t.state == Seeding,
            Filter::Completed => matches!(t.state, Seeding | Completed),
            Filter::Paused => t.state == Paused,
            Filter::Errors => t.state == Error,
            Filter::Category(c) => t.category.as_deref() == Some(c.as_str()),
            Filter::Tracker(h) if h.is_empty() => t.trackers.is_empty(),
            Filter::Tracker(h) => t.trackers.iter().any(|x| x == h),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    Name,
    Size,
    Progress,
    Status,
    Down,
    Up,
    Eta,
    Ratio,
    Peers,
}

impl Column {
    pub const ALL: [Column; 9] = [Column::Name, Column::Size, Column::Progress, Column::Status, Column::Down, Column::Up, Column::Eta, Column::Ratio, Column::Peers];

    pub fn id(self) -> &'static str {
        match self {
            Column::Name => "name",
            Column::Size => "size",
            Column::Progress => "progress",
            Column::Status => "status",
            Column::Down => "down",
            Column::Up => "up",
            Column::Eta => "eta",
            Column::Ratio => "ratio",
            Column::Peers => "peers",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Column::Name => "Name",
            Column::Size => "Size",
            Column::Progress => "Progress",
            Column::Status => "Status",
            Column::Down => "Down",
            Column::Up => "Up",
            Column::Eta => "ETA",
            Column::Ratio => "Ratio",
            Column::Peers => "Peers",
        }
    }

    fn cmp(self, a: &Torrent, b: &Torrent) -> std::cmp::Ordering {
        let f = |x: f32, y: f32| x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal);
        match self {
            Column::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            Column::Size => a.size.cmp(&b.size),
            Column::Progress => f(a.progress(), b.progress()),
            Column::Status => a.state.label().cmp(b.state.label()),
            Column::Down => a.down.cmp(&b.down),
            Column::Up => a.up.cmp(&b.up),
            Column::Eta => a.eta_secs.unwrap_or(u64::MAX).cmp(&b.eta_secs.unwrap_or(u64::MAX)),
            Column::Ratio => f(a.ratio(), b.ratio()),
            Column::Peers => a.peers_live.cmp(&b.peers_live),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    General,
    Files,
    Peers,
    Trackers,
    Pieces,
    Log,
}

impl Tab {
    pub const ALL: [Tab; 6] = [Tab::General, Tab::Files, Tab::Peers, Tab::Trackers, Tab::Pieces, Tab::Log];

    pub fn id(self) -> &'static str {
        match self {
            Tab::General => "general",
            Tab::Files => "files",
            Tab::Peers => "peers",
            Tab::Trackers => "trackers",
            Tab::Pieces => "pieces",
            Tab::Log => "log",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Tab::General => "General",
            Tab::Files => "Files",
            Tab::Peers => "Peers",
            Tab::Trackers => "Trackers",
            Tab::Pieces => "Pieces",
            Tab::Log => "Log",
        }
    }
}

/// Which app icon to show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconStyle {
    /// Follow the system's light or dark appearance.
    Auto,
    Light,
    Dark,
}

impl IconStyle {
    pub const ALL: [IconStyle; 3] = [IconStyle::Auto, IconStyle::Light, IconStyle::Dark];

    pub fn id(self) -> &'static str {
        match self {
            IconStyle::Auto => "auto",
            IconStyle::Light => "light",
            IconStyle::Dark => "dark",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            IconStyle::Auto => "Automatic",
            IconStyle::Light => "Light",
            IconStyle::Dark => "Dark",
        }
    }

    pub fn parse(s: &str) -> Option<IconStyle> {
        IconStyle::ALL.into_iter().find(|i| i.id().eq_ignore_ascii_case(s))
    }
}

// One dialog exists at a time, so the size its text fields give the enum costs nothing.
#[allow(clippy::large_enum_variant)]
pub enum Dialog {
    None,
    Add { source: TextEdit, paused: bool },
    Remove { ids: Vec<u64>, delete_files: bool },
    Limits { down: TextEdit, up: TextEdit },
}

impl Dialog {
    pub fn name(&self) -> &'static str {
        match self {
            Dialog::None => "none",
            Dialog::Add { .. } => "add",
            Dialog::Remove { .. } => "remove",
            Dialog::Limits { .. } => "limits",
        }
    }
}

pub struct Palette {
    pub query: TextEdit,
    pub index: usize,
}

pub struct PaletteItem {
    pub icon: Icon,
    pub title: String,
    pub section: &'static str,
    pub cmd: Command,
}

pub struct LogLine {
    pub time: String,
    pub origin: Origin,
    pub cli: String,
    pub result: String,
    pub ok: bool,
}

pub struct Toast {
    pub text: String,
    pub ok: bool,
    pub until: f64,
}

pub const HISTORY: usize = 90;

pub struct State {
    pub backend: Arc<Backend>,
    pub exec: Exec,
    pub shared: Arc<Shared>,
    pub snap: Snapshot,
    /// Indices into `snap.torrents`, filtered, searched and sorted: the table's rows.
    pub rows: Vec<usize>,
    /// Bumped whenever `rows` may have changed order, so the table re-reads its keys.
    pub rows_version: u64,
    pub filter: Filter,
    /// In the order they were selected; the last is the one the inspector shows.
    pub selected: Vec<u64>,
    pub sort: Option<(Column, bool)>,
    pub search: TextEdit,
    /// Where a text field's edit lands if its dialog closed under it.
    pub scratch: TextEdit,
    pub tab: Tab,
    pub sidebar_open: bool,
    pub inspector_open: bool,
    pub palette: Option<Palette>,
    pub dialog: Dialog,
    pub log: VecDeque<LogLine>,
    pub toast: Option<Toast>,
    /// (down, up) samples, newest last: the session's, and the inspected torrent's.
    pub session_history: VecDeque<(u64, u64)>,
    pub torrent_history: (u64, VecDeque<(u64, u64)>),
    pub demo: bool,
    pub icon: IconStyle,
}

impl State {
    pub fn new(backend: Arc<Backend>, exec: Exec, shared: Arc<Shared>, demo: bool) -> State {
        let mut s = State {
            backend,
            exec,
            shared,
            snap: Snapshot::default(),
            rows: Vec::new(),
            rows_version: 0,
            filter: Filter::All,
            selected: Vec::new(),
            sort: None,
            search: TextEdit::new(""),
            scratch: TextEdit::new(""),
            tab: Tab::General,
            sidebar_open: true,
            inspector_open: true,
            palette: None,
            dialog: Dialog::None,
            log: VecDeque::new(),
            toast: None,
            session_history: VecDeque::new(),
            torrent_history: (0, VecDeque::new()),
            demo,
            icon: IconStyle::Auto,
        };
        s.icon = IconStyle::parse(&s.backend.app_icon()).unwrap_or(IconStyle::Auto);
        let snap = s.backend.snapshot(None);
        s.set_snapshot(snap);
        s
    }

    /// The torrent the inspector shows.
    pub fn inspected(&self) -> Option<&Torrent> {
        let id = *self.selected.last()?;
        self.snap.torrents.iter().find(|t| t.id == id)
    }

    pub fn torrent(&self, id: u64) -> Option<&Torrent> {
        self.snap.torrents.iter().find(|t| t.id == id)
    }

    pub fn set_snapshot(&mut self, snap: Snapshot) {
        self.snap = snap;
        let before = self.selected.len();
        let torrents = &self.snap.torrents;
        self.selected.retain(|id| torrents.iter().any(|t| t.id == *id));
        if self.selected.len() != before {
            self.mirror_selection();
        }
        push(&mut self.session_history, (self.snap.session.down, self.snap.session.up));
        match self.inspected().map(|t| (t.id, t.down, t.up)) {
            Some((id, d, u)) => {
                if self.torrent_history.0 != id {
                    self.torrent_history = (id, VecDeque::new());
                }
                push(&mut self.torrent_history.1, (d, u));
            }
            None => self.torrent_history = (0, VecDeque::new()),
        }
        self.recompute_rows();
    }

    pub fn recompute_rows(&mut self) {
        let words: Vec<String> = self.search.text.to_lowercase().split_whitespace().map(str::to_owned).collect();
        let ts = &self.snap.torrents;
        let mut rows: Vec<usize> = (0..ts.len())
            .filter(|&i| self.filter.matches(&ts[i]))
            .filter(|&i| {
                let name = ts[i].name.to_lowercase();
                words.iter().all(|w| name.contains(w))
            })
            .collect();
        match self.sort {
            Some((col, desc)) => rows.sort_by(|&a, &b| {
                let o = col.cmp(&ts[a], &ts[b]).then(ts[a].id.cmp(&ts[b].id));
                if desc { o.reverse() } else { o }
            }),
            None => rows.sort_by_key(|&i| ts[i].id),
        }
        if rows.len() != self.rows.len() || rows.iter().zip(&self.rows).any(|(a, b)| a != b) {
            self.rows_version += 1;
        }
        self.rows = rows;
    }

    fn mirror_selection(&self) {
        *self.shared.selected.lock().unwrap_or_else(|e| e.into_inner()) = self.selected.clone();
    }

    fn set_selection(&mut self, ids: Vec<u64>) {
        self.selected = ids;
        self.mirror_selection();
        self.refresh();
    }

    /// Asks for a fresh snapshot (with the inspected torrent's detail).
    pub fn refresh(&mut self) {
        match &self.exec {
            Exec::Inline => {
                let snap = self.backend.snapshot(self.selected.last().copied());
                self.set_snapshot(snap);
            }
            Exec::Worker(tx) => {
                let _ = tx.send(Job::Refresh);
            }
        }
    }

    /// Records a finished command and shows what needs showing. Runs on the UI thread.
    pub fn finish(&mut self, cmd: &Command, resp: &Response, origin: Origin, tx: &mut Tx) {
        if !cmd.is_query() {
            self.log.push_back(LogLine { time: clock_hms(), origin, cli: cmd.to_cli(), result: if resp.message.is_empty() { "ok".into() } else { resp.message.clone() }, ok: resp.ok });
            while self.log.len() > 300 {
                self.log.pop_front();
            }
        }
        if !resp.ok {
            self.say(&resp.message, false, tx);
        } else if matches!(cmd, Command::Add { .. } | Command::Remove { .. } | Command::Limit { .. } | Command::AltLimits { .. } | Command::Category { .. }) {
            self.say(&resp.message, true, tx);
        }
        if let (Command::Reveal { .. }, Some(path)) = (cmd, resp.data.get("path").and_then(|p| p.as_str()))
            && resp.ok
            && tx.reveal_path(expand_home(path)).is_err()
        {
            self.say("Couldn't show that in the file manager", false, tx);
        }
    }

    pub fn say(&mut self, text: &str, ok: bool, tx: &mut Tx) {
        self.toast = Some(Toast { text: text.to_owned(), ok, until: tx.clock() + 3.5 });
        tx.wake_after(std::time::Duration::from_millis(3600));
    }

    /// Everything the window shows, for `torrz ui state`.
    pub fn ui_json(&self) -> serde_json::Value {
        let items = self.palette.as_ref().map(|p| palette_items(self, &p.query.text));
        json!({
            "filter": self.filter.name(),
            "title": self.filter.title(),
            "selected": self.selected,
            "rows": self.rows.iter().map(|&i| self.snap.torrents[i].id).collect::<Vec<_>>(),
            "sort": self.sort.map(|(c, desc)| json!({ "column": c.id(), "desc": desc })),
            "search": self.search.text,
            "tab": self.tab.id(),
            "sidebar": self.sidebar_open,
            "inspector": self.inspector_open,
            "dialog": match &self.dialog {
                Dialog::None => json!(null),
                Dialog::Add { source, paused } => json!({ "name": "add", "source": source.text, "paused": paused }),
                Dialog::Remove { ids, delete_files } => json!({ "name": "remove", "ids": ids, "delete-files": delete_files }),
                Dialog::Limits { down, up } => json!({ "name": "limits", "down": down.text, "up": up.text }),
            },
            "palette": self.palette.as_ref().map(|p| json!({
                "query": p.query.text,
                "index": p.index,
                "items": items.iter().flatten().map(|i| json!({ "title": i.title, "command": i.cmd.to_cli() })).collect::<Vec<_>>(),
            })),
            "toast": self.toast.as_ref().map(|t| t.text.clone()),
            "log": self.log.iter().rev().take(20).map(|l| json!({ "time": l.time, "from": if l.origin == Origin::Ui { "ui" } else { "cli" }, "command": l.cli, "result": l.result, "ok": l.ok })).collect::<Vec<_>>(),
            "demo": self.demo,
            "icon": self.icon.id(),
        })
    }
}

fn push(h: &mut VecDeque<(u64, u64)>, v: (u64, u64)) {
    h.push_back(v);
    while h.len() > HISTORY {
        h.pop_front();
    }
}

pub fn expand_home(p: &str) -> std::path::PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(rest),
        None => p.into(),
    }
}

fn clock_hms() -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    #[cfg(unix)]
    {
        let t = now as libc::time_t;
        let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
        // SAFETY: `t` and `tm` are valid for the call; `tm` is read only when localtime_r
        // returns non-null, which means it was filled in.
        let tm = unsafe {
            if libc::localtime_r(&t, tm.as_mut_ptr()).is_null() {
                return String::new();
            }
            tm.assume_init()
        };
        format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
    }
    #[cfg(not(unix))]
    {
        format!("{:02}:{:02}:{:02}", now / 3600 % 24, now / 60 % 60, now % 60)
    }
}

/// The palette's rows for what is typed, best first: every word typed must appear in the row's
/// title or its command line.
pub fn palette_items(s: &State, query: &str) -> Vec<PaletteItem> {
    let ui = |u: Ui| Command::Ui(u);
    let item = |icon, section, title: String, cmd| PaletteItem { icon, title, section, cmd };
    let mut v = Vec::new();
    let sel: Vec<String> = s.selected.iter().map(|i| i.to_string()).collect();
    if let Some(t) = s.inspected() {
        let what = if sel.len() == 1 { format!("“{}”", t.name) } else { format!("{} torrents", sel.len()) };
        let running = matches!(t.state, TorrentState::Downloading | TorrentState::Seeding | TorrentState::Checking | TorrentState::Queued);
        if running || sel.len() > 1 {
            v.push(item(Icon::Pause, "Selection", format!("Pause {what}"), Command::Pause { targets: sel.clone() }));
        }
        if !running || sel.len() > 1 {
            v.push(item(Icon::Play, "Selection", format!("Resume {what}"), Command::Resume { targets: sel.clone() }));
        }
        v.push(item(Icon::Folder, "Selection", "Reveal in Finder".into(), Command::Reveal { target: t.id.to_string() }));
        v.push(item(Icon::Trash, "Selection", format!("Remove {what}…"), ui(Ui::Dialog { name: "remove".into() })));
    }
    v.push(item(Icon::Plus, "Actions", "Add Torrent…".into(), ui(Ui::Dialog { name: "add".into() })));
    v.push(item(Icon::Pause, "Actions", "Pause All".into(), Command::Pause { targets: vec!["all".into()] }));
    v.push(item(Icon::Play, "Actions", "Resume All".into(), Command::Resume { targets: vec!["all".into()] }));
    v.push(item(Icon::Sliders, "Actions", "Set Speed Limits…".into(), ui(Ui::Dialog { name: "limits".into() })));
    v.push(item(Icon::Gauge, "Actions", format!("Turn Alternate Speed Limits {}", if s.snap.session.alt_limits { "Off" } else { "On" }), Command::AltLimits { on: Some(!s.snap.session.alt_limits) }));
    for f in [Filter::All, Filter::Downloading, Filter::Seeding, Filter::Completed, Filter::Paused, Filter::Errors] {
        v.push(item(Icon::All, "View", format!("Show {}", f.title()), ui(Ui::Filter { name: f.name() })));
    }
    for t in Tab::ALL {
        v.push(item(Icon::Panel, "View", format!("Inspector: {}", t.title()), ui(Ui::Tab { name: t.id().into() })));
    }
    for c in Column::ALL {
        v.push(item(Icon::Sort, "View", format!("Sort by {}", c.title()), ui(Ui::Sort { column: c.id().into(), desc: None })));
    }
    v.push(item(Icon::Sidebar, "View", format!("{} Sidebar", if s.sidebar_open { "Hide" } else { "Show" }), ui(Ui::Sidebar { open: Some(!s.sidebar_open) })));
    v.push(item(Icon::Panel, "View", format!("{} Inspector", if s.inspector_open { "Hide" } else { "Show" }), ui(Ui::Inspector { open: Some(!s.inspector_open) })));
    for i in IconStyle::ALL {
        let mark = if s.icon == i { " ✓" } else { "" };
        v.push(item(Icon::Panel, "App Icon", format!("App Icon: {}{mark}", i.title()), ui(Ui::Icon { style: i.id().into() })));
    }
    v.push(item(Icon::Power, "View", "Quit Torrz".into(), ui(Ui::Window { action: "quit".into() })));
    for t in &s.snap.torrents {
        v.push(item(
            match t.state {
                TorrentState::Seeding => Icon::Up,
                TorrentState::Downloading => Icon::Down,
                TorrentState::Paused => Icon::Pause,
                _ => Icon::Check,
            },
            "Torrents",
            t.name.clone(),
            ui(Ui::Select { targets: vec![t.id.to_string()] }),
        ));
    }
    let words: Vec<String> = query.to_lowercase().split_whitespace().map(str::to_owned).collect();
    if words.is_empty() {
        // Nothing typed: the actions, not the whole library.
        v.retain(|i| i.section != "Torrents");
        return v;
    }
    let mut scored: Vec<(i32, PaletteItem)> = v
        .into_iter()
        .filter_map(|i| {
            let title = i.title.to_lowercase();
            let cli = i.cmd.to_cli().to_lowercase();
            let mut score = 0;
            for w in &words {
                if title.starts_with(w.as_str()) {
                    score += 3;
                } else if title.split(|c: char| !c.is_alphanumeric()).any(|p| p.starts_with(w.as_str())) {
                    score += 2;
                } else if title.contains(w.as_str()) {
                    score += 1;
                } else if !cli.contains(w.as_str()) {
                    return None;
                }
            }
            Some((score, i))
        })
        .collect();
    scored.sort_by_key(|(score, _)| -score);
    scored.into_iter().map(|(_, i)| i).collect()
}

/// Runs a command. Window commands apply at once; engine commands go to the engine thread (or
/// run here when [`Exec::Inline`]) and their result arrives through [`State::finish`].
pub fn dispatch(s: &mut State, cmd: Command, origin: Origin, tx: &mut Tx) -> Response {
    match cmd {
        Command::Ui(u) => {
            let logged = Command::Ui(u.clone());
            let r = match apply_ui(s, u, origin, tx) {
                Ok(r) => r,
                Err(e) => Response::err(format!("{e:#}")),
            };
            // The palette's own steps (typing, moving the highlight) would drown the log.
            if !matches!(logged, Command::Ui(Ui::PaletteMove { .. } | Ui::PaletteRun | Ui::Palette { .. } | Ui::Field { .. })) || !r.ok {
                s.finish(&logged, &r, origin, tx);
            }
            r
        }
        cmd => match &s.exec {
            Exec::Inline => {
                let r = s.backend.exec(&cmd, &s.selected);
                s.finish(&cmd, &r, origin, tx);
                s.refresh();
                r
            }
            Exec::Worker(w) => {
                let line = cmd.to_cli();
                let _ = w.send(Job::Run { cmd, selected: s.selected.clone(), origin });
                // The engine thread answers through `State::finish`; the log has the outcome.
                Response::ok(format!("running {line}"))
            }
        },
    }
}

fn toggle(cur: bool, want: Option<bool>) -> bool {
    want.unwrap_or(!cur)
}

fn apply_ui(s: &mut State, u: Ui, origin: Origin, tx: &mut Tx) -> anyhow::Result<Response> {
    Ok(match u {
        Ui::State => Response::data("", s.ui_json()),
        Ui::Filter { name } => {
            s.filter = Filter::parse(&name)?;
            s.recompute_rows();
            Response::ok(format!("showing {}", s.filter.title()))
        }
        Ui::Select { targets } => {
            let ids = if targets.is_empty() { Vec::new() } else { Backend::resolve(&s.snap.torrents, &targets, &s.selected)? };
            if let Some(last) = ids.last() {
                tx.scroll_into_view(row_key(*last));
            }
            let n = ids.len();
            s.set_selection(ids);
            Response::ok(match n {
                0 => "selection cleared".into(),
                1 => "selected 1 torrent".to_owned(),
                n => format!("selected {n} torrents"),
            })
        }
        Ui::Tab { name } => {
            s.tab = Tab::ALL.into_iter().find(|t| t.id().eq_ignore_ascii_case(&name)).ok_or_else(|| anyhow::anyhow!("no tab {name:?}: general, files, peers, trackers, pieces or log"))?;
            s.inspector_open = true;
            Response::ok(format!("inspector shows {}", s.tab.title()))
        }
        Ui::Sort { column, desc } => {
            if column.eq_ignore_ascii_case("added") {
                s.sort = None;
            } else {
                let col = Column::ALL.into_iter().find(|c| c.id().eq_ignore_ascii_case(&column)).ok_or_else(|| anyhow::anyhow!("no column {column:?}: name, size, progress, status, down, up, eta, ratio, peers or added"))?;
                let desc = desc.unwrap_or(matches!(s.sort, Some((c, false)) if c == col));
                s.sort = Some((col, desc));
            }
            s.recompute_rows();
            Response::ok(match s.sort {
                Some((c, d)) => format!("sorted by {} {}", c.id(), if d { "descending" } else { "ascending" }),
                None => "sorted by date added".into(),
            })
        }
        Ui::Search { query } => {
            s.search.replace_text(query);
            let end = s.search.text.len();
            s.search.set_caret(end);
            s.recompute_rows();
            Response::ok(format!("{} shown", s.rows.len()))
        }
        Ui::Palette { open, query } => {
            let open = toggle(s.palette.is_some(), open) || (open.is_none() && query.is_some());
            if open {
                let p = s.palette.get_or_insert_with(|| Palette { query: TextEdit::new(""), index: 0 });
                if let Some(q) = query {
                    p.query.replace_text(q);
                    let end = p.query.text.len();
                    p.query.set_caret(end);
                    p.index = 0;
                }
                s.dialog = Dialog::None;
                tx.focus_when_shown("palette-input", false);
                Response::ok("palette open")
            } else {
                s.palette = None;
                Response::ok("palette closed")
            }
        }
        Ui::PaletteMove { delta } => {
            let n = s.palette.as_ref().map(|p| palette_items(s, &p.query.text).len()).ok_or_else(|| anyhow::anyhow!("the palette is closed"))?;
            let p = s.palette.as_mut().ok_or_else(|| anyhow::anyhow!("the palette is closed"))?;
            if n > 0 {
                p.index = (p.index as i64 + delta as i64).rem_euclid(n as i64) as usize;
                tx.scroll_into_view(format!("palette-item-{}", p.index));
            }
            Response::data("", p.index)
        }
        Ui::PaletteRun => {
            let p = s.palette.as_ref().ok_or_else(|| anyhow::anyhow!("the palette is closed"))?;
            let mut items = palette_items(s, &p.query.text);
            anyhow::ensure!(!items.is_empty(), "nothing matches {:?}", p.query.text);
            let item = items.swap_remove(p.index.min(items.len() - 1));
            s.palette = None;
            let line = item.cmd.to_cli();
            let r = dispatch(s, item.cmd, origin, tx);
            Response { message: if r.ok { format!("ran {line}") } else { r.message }, ..r }
        }
        Ui::Dialog { name } => {
            s.palette = None;
            s.dialog = match name.to_ascii_lowercase().as_str() {
                "none" => Dialog::None,
                "add" => {
                    tx.focus_when_shown("dialog-source", false);
                    Dialog::Add { source: TextEdit::new(""), paused: false }
                }
                "remove" => {
                    anyhow::ensure!(!s.selected.is_empty(), "select a torrent to remove first");
                    Dialog::Remove { ids: s.selected.clone(), delete_files: false }
                }
                "limits" => {
                    let show = |l: Option<u64>| l.map(|b| speed(b).trim_end_matches("/s").replace(' ', "").replace("kB", "k").replace("MB", "M").replace("GB", "G")).unwrap_or_default();
                    tx.focus_when_shown("dialog-down", false);
                    Dialog::Limits { down: TextEdit::new(show(s.snap.session.limit_down)), up: TextEdit::new(show(s.snap.session.limit_up)) }
                }
                _ => anyhow::bail!("no dialog {name:?}: add, remove, limits or none"),
            };
            Response::ok(format!("dialog: {}", s.dialog.name()))
        }
        Ui::Field { name, value } => {
            let truthy = || match value.to_ascii_lowercase().as_str() {
                "on" | "true" | "yes" | "1" => Ok(true),
                "off" | "false" | "no" | "0" => Ok(false),
                _ => Err(anyhow::anyhow!("{name} is on or off, not {value:?}")),
            };
            match (&mut s.dialog, name.as_str()) {
                (Dialog::Add { source, .. }, "source") => source.replace_text(value.clone()),
                (Dialog::Add { paused, .. }, "paused") => *paused = truthy()?,
                (Dialog::Remove { delete_files, .. }, "delete-files") => *delete_files = truthy()?,
                (Dialog::Limits { down, .. }, "down") => down.replace_text(value.clone()),
                (Dialog::Limits { up, .. }, "up") => up.replace_text(value.clone()),
                (Dialog::None, _) => anyhow::bail!("no dialog is open"),
                (d, _) => anyhow::bail!("the {} dialog has no field {name:?}", d.name()),
            }
            Response::ok(format!("{name} set"))
        }
        Ui::Confirm => {
            let cmd = match &s.dialog {
                Dialog::None => anyhow::bail!("no dialog is open"),
                Dialog::Add { source, paused } => {
                    anyhow::ensure!(!source.text.trim().is_empty(), "paste a magnet link or choose a .torrent file first");
                    Command::Add { source: source.text.trim().to_owned(), paused: *paused, dir: None, category: None }
                }
                Dialog::Remove { ids, delete_files } => Command::Remove { targets: ids.iter().map(|i| i.to_string()).collect(), delete_files: *delete_files },
                Dialog::Limits { down, up } => {
                    let rate = |t: &TextEdit| if t.text.trim().is_empty() { "none".to_owned() } else { t.text.trim().to_owned() };
                    let (d, u) = (rate(down), rate(up));
                    // Refuse here so the dialog stays open on a typo.
                    torrz_core::model::parse_rate(&d)?;
                    torrz_core::model::parse_rate(&u)?;
                    Command::Limit { down: Some(d), up: Some(u) }
                }
            };
            s.dialog = Dialog::None;
            dispatch(s, cmd, origin, tx)
        }
        Ui::Cancel => {
            if s.palette.take().is_some() {
                Response::ok("palette closed")
            } else {
                s.dialog = Dialog::None;
                Response::ok("closed")
            }
        }
        Ui::Sidebar { open } => {
            s.sidebar_open = toggle(s.sidebar_open, open);
            Response::ok(if s.sidebar_open { "sidebar shown" } else { "sidebar hidden" })
        }
        Ui::Inspector { open } => {
            s.inspector_open = toggle(s.inspector_open, open);
            Response::ok(if s.inspector_open { "inspector shown" } else { "inspector hidden" })
        }
        Ui::Window { action } => {
            match action.to_ascii_lowercase().as_str() {
                // Showing is the app handle's job (main.rs does it before this runs).
                "show" => {}
                "hide" => tx.close_window(),
                "minimize" => tx.minimize(),
                "quit" => tx.quit(),
                _ => anyhow::bail!("no window action {action:?}: show, hide, minimize or quit"),
            }
            Response::ok(format!("window: {action}"))
        }
        Ui::Icon { style } => {
            s.icon = IconStyle::parse(&style).ok_or_else(|| anyhow::anyhow!("no icon style {style:?}: auto, light or dark"))?;
            s.backend.set_app_icon(s.icon.id());
            Response::ok(format!("app icon: {}", s.icon.id()))
        }
    })
}

pub fn row_key(id: u64) -> String {
    format!("torrent-{id}")
}

/// What a click on a row selects, as a command: plain replaces, ⌘ toggles, ⇧ extends from the
/// last one selected.
pub fn click_selection(s: &State, id: u64, cmd: bool, shift: bool) -> Command {
    let ids: Vec<u64> = if shift && !s.selected.is_empty() {
        let order: Vec<u64> = s.rows.iter().map(|&i| s.snap.torrents[i].id).collect();
        let anchor = *s.selected.last().unwrap_or(&id);
        match (order.iter().position(|x| *x == anchor), order.iter().position(|x| *x == id)) {
            (Some(a), Some(b)) => {
                let mut v: Vec<u64> = order[a.min(b)..=a.max(b)].to_vec();
                // Keep the clicked row last: it is the one the inspector shows.
                v.retain(|x| *x != id);
                v.push(id);
                v
            }
            _ => vec![id],
        }
    } else if cmd {
        let mut v = s.selected.clone();
        if v.contains(&id) {
            v.retain(|x| *x != id);
        } else {
            v.push(id);
        }
        v
    } else {
        vec![id]
    };
    Command::Ui(Ui::Select { targets: ids.iter().map(|i| i.to_string()).collect() })
}
