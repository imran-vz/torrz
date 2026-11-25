//! The Graphite window: sidebar, toolbar, table, inspector, status bar, and what floats over them.

mod inspector;
pub mod menu;
mod overlays;
pub mod pieces;
mod sidebar;
mod table;

use rune::{Cx, El, KeyEvent, Tx, col, el, row, text};
use torrz_core::model::{size, speed};
use torrz_core::{Command, TorrentState, Ui};

use crate::state::{Dialog, Origin, State, dispatch};
use crate::theme::{self as t, Icon, icon};

pub const TITLEBAR: f32 = 48.0;

/// Makes `e` a control that runs `cmd`. Its key is the command's own shell line, which is how
/// the tests hold every control to having one.
pub fn act(e: El, cmd: Command) -> El {
    e.key(cmd.to_cli()).role("button").on_press_tx(move |s: &mut State, tx: &mut Tx| {
        dispatch(s, cmd.clone(), Origin::Ui, tx);
    })
}

pub fn ui(u: Ui) -> Command {
    Command::Ui(u)
}

/// A square toolbar button. Disabled, it is dimmed and inert but still names its command.
pub fn tool(i: Icon, label: &str, cmd: Command, enabled: bool) -> El {
    let b = row().size(30.0, 26.0).radius(6.0).center().child(icon(i, 15.0, if enabled { t::TEXT_2 } else { t::FAINT.alpha(0.6) })).label(label).help(label);
    if enabled { act(b.hover(t::HOVER).press(t::PRESS), cmd) } else { b.key(cmd.to_cli()).role("button").disabled(true) }
}

pub fn sep() -> El {
    el().size(1.0, 18.0).fill(t::RULE_STRONG).mx(4.0)
}

pub fn state_color(s: TorrentState) -> rune::Color {
    match s {
        TorrentState::Downloading => t::BLUE_TEXT,
        TorrentState::Seeding => t::GREEN_TEXT,
        TorrentState::Checking => t::AMBER,
        TorrentState::Error => t::RED,
        _ => t::MUTED,
    }
}

pub fn bar_color(s: TorrentState) -> rune::Color {
    match s {
        TorrentState::Downloading => t::BLUE,
        TorrentState::Seeding | TorrentState::Completed => t::GREEN,
        TorrentState::Checking => t::AMBER,
        TorrentState::Error => t::RED,
        _ => t::GREY,
    }
}

fn status_bar(s: &State) -> El {
    let ss = &s.snap.session;
    let dim = |x: String| text(x).font(t::SMALL).color(t::MUTED);
    let strong = |x: String| text(x).font(t::SMALL_M).color(t::TEXT_2);
    let limit = |l: Option<u64>| l.map(speed).unwrap_or_else(|| "∞".into());
    let mut bar = row()
        .key("status-bar")
        .w_full()
        .h(24.0)
        .px(14.0)
        .gap(16.0)
        .center_y()
        .fill(t::BAR)
        .border_sides([1.0, 0.0, 0.0, 0.0], t::EDGE)
        .child(strong(format!("↓ {}", speed(ss.down).replace('—', "0 B/s"))))
        .child(strong(format!("↑ {}", speed(ss.up).replace('—', "0 B/s"))));
    bar = bar.child(match ss.port {
        Some(p) => row().gap(5.0).center_y().child(el().size(7.0, 7.0).radius(3.5).fill(t::GREEN)).child(dim(format!("Port {p}"))),
        None => row().gap(5.0).center_y().child(el().size(7.0, 7.0).radius(3.5).fill(t::AMBER)).child(dim("Not listening".into())),
    });
    bar = bar.child(dim(format!("DHT {} nodes", ss.dht_nodes)));
    if ss.limit_down.is_some() || ss.limit_up.is_some() {
        bar = bar.child(dim(format!("{} ↓ {}  ↑ {}", if ss.alt_limits { "Alt limits" } else { "Limits" }, limit(ss.limit_down), limit(ss.limit_up))).color(if ss.alt_limits { t::AMBER } else { t::MUTED }));
    }
    if let Some(d) = &ss.watch_dir {
        let name = std::path::Path::new(d).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| d.clone());
        bar = bar.child(act(
            row().gap(5.0).center_y().child(icon(Icon::Folder, 12.0, t::MUTED)).child(dim(format!("Watching {name}"))).label(format!("Watching {d} for .torrent files")).help(d.clone()),
            ui(Ui::Dialog { name: "watch".into() }),
        ));
    }
    bar = bar.child(el().grow(1.0));
    if s.demo {
        bar = bar.child(text("Demo swarm").font(t::SMALL_M).color(t::AMBER));
    }
    if let Some(f) = ss.free_disk {
        bar = bar.child(dim(format!("Free {}", size(f))));
    }
    bar.child(text("⌘K").font(t::MONO).color(t::FAINT))
}

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    cx.caption_color(t::WHITE);
    crate::icon::sync(s.icon, cx.system_dark());
    let insets = cx.chrome_insets();
    let mut main = col().key("main").grow(1.0).min_w(0.0).h_full().child(table::toolbar(s, if s.sidebar_open { 0.0 } else { insets.leading })).child(table::table(s));
    if s.inspector_open {
        main = main.child(inspector::inspector(s));
    }
    main = main.child(status_bar(s));

    let mut body = row().key("body").w_full().h_full();
    if s.sidebar_open {
        body = body.child(sidebar::sidebar(s, insets.leading));
    }
    body = body.child(main);

    let mut root = el()
        .key("root")
        .w_full()
        .h_full()
        .fill(t::BG)
        .accepts_extensions(&["torrent"])
        .on_drop_files(|s: &mut State, paths: &[std::path::PathBuf], tx: &mut Tx| {
            for p in paths {
                dispatch(s, Command::Add { source: p.display().to_string(), paused: false, dir: None, category: None, trash: false }, Origin::Ui, tx);
            }
        })
        .child(body);
    if let Some(toast) = &s.toast {
        root = root.child(overlays::toast(toast));
    }
    if !matches!(s.dialog, Dialog::None) {
        root = root.child(overlays::dialog(s));
    }
    if let Some(p) = &s.palette {
        root = root.child(overlays::palette(s, p));
    }
    root
}

/// Keys nothing focused wanted.
pub fn on_key(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    use rune::Key;
    let mut run = |s: &mut State, c: Command| {
        dispatch(s, c, Origin::Ui, tx);
        true
    };
    let selected = || vec!["selected".to_owned()];
    let typing = s.palette.is_some() || !matches!(s.dialog, Dialog::None);
    match &ev.key {
        Key::Command('k') => run(s, ui(Ui::Palette { open: None, query: None })),
        Key::Command('o') | Key::Command('n') => run(s, ui(Ui::Dialog { name: "add".into() })),
        Key::Command('f') => {
            tx.focus("search", false);
            true
        }
        Key::Command('i') => run(s, ui(Ui::Inspector { open: None })),
        Key::Command('s') if ev.mods.alt => run(s, ui(Ui::Sidebar { open: None })),
        Key::Command(',') => run(s, ui(Ui::Dialog { name: "limits".into() })),
        Key::Command('a') if !typing => run(s, ui(Ui::Select { targets: s.rows.iter().map(|&i| s.snap.torrents[i].id.to_string()).collect() })),
        Key::Command(c @ '1'..='6') => run(s, ui(Ui::Tab { name: crate::state::Tab::ALL[*c as usize - '1' as usize].id().into() })),
        Key::Escape if typing => run(s, ui(Ui::Cancel)),
        Key::Escape if !s.search.text.is_empty() => run(s, ui(Ui::Search { query: String::new() })),
        Key::Escape if !s.selected.is_empty() => run(s, ui(Ui::Select { targets: Vec::new() })),
        Key::Enter if !matches!(s.dialog, Dialog::None) => run(s, ui(Ui::Confirm)),
        _ if typing => false,
        Key::Text(sp) if sp == " " && !s.selected.is_empty() => {
            let running = s.inspected().is_some_and(|t| matches!(t.state, TorrentState::Downloading | TorrentState::Seeding | TorrentState::Checking | TorrentState::Queued));
            run(s, if running { Command::Pause { targets: selected() } } else { Command::Resume { targets: selected() } })
        }
        Key::Backspace | Key::Delete if !s.selected.is_empty() => run(s, ui(Ui::Dialog { name: "remove".into() })),
        Key::Up | Key::Down if !s.rows.is_empty() => {
            let order: Vec<u64> = s.rows.iter().map(|&i| s.snap.torrents[i].id).collect();
            let at = s.selected.last().and_then(|id| order.iter().position(|x| x == id));
            let next = match (at, &ev.key) {
                (None, Key::Down) => 0,
                (None, _) => order.len() - 1,
                (Some(i), Key::Down) => (i + 1).min(order.len() - 1),
                (Some(i), _) => i.saturating_sub(1),
            };
            let cmd = crate::state::click_selection(s, order[next], false, ev.mods.shift);
            run(s, cmd)
        }
        _ => false,
    }
}

/// Called at the start of each frame: lets the toast expire.
pub fn poll(s: &mut State, now: f64, _: &mut Tx) -> bool {
    if s.toast.as_ref().is_some_and(|t| now >= t.until) {
        s.toast = None;
        return true;
    }
    false
}

pub fn section(title: &str) -> El {
    text(title).font(t::TINY_B).color(t::MUTED)
}
