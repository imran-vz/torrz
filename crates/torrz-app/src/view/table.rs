//! The toolbar and the torrent table.

use std::borrow::Cow;

use rune::{El, GrowthSpec, ScrollSpec, Tx, col, el, row, text, text_input};
use torrz_core::model::{eta, size, speed};
use torrz_core::{Command, Torrent, TorrentState, Ui};

use super::{TITLEBAR, act, bar_color, sep, state_color, tool, ui};
use crate::state::{Column, Origin, State, click_selection, dispatch, row_key};
use crate::theme::{self as t, Icon, icon};

const ROW: f32 = 28.0;
const GAP: f32 = 10.0;

/// `None` grows (the name column).
fn width(c: Column) -> Option<f32> {
    Some(match c {
        Column::Name => return None,
        Column::Size => 60.0,
        Column::Progress => 124.0,
        Column::Status => 84.0,
        Column::Down => 76.0,
        Column::Up => 72.0,
        Column::Eta => 60.0,
        Column::Ratio => 40.0,
        Column::Peers => 58.0,
    })
}

fn left(c: Column) -> bool {
    matches!(c, Column::Name | Column::Progress | Column::Status)
}

fn sized(c: Column, e: El) -> El {
    match width(c) {
        Some(w) => e.w(w),
        None => e.grow(1.0).min_w(0.0),
    }
}

pub fn toolbar(s: &State, leading_inset: f32) -> El {
    let sel = vec!["selected".to_owned()];
    let any = !s.selected.is_empty();
    let total: u64 = s.rows.iter().map(|&i| s.snap.torrents[i].size).sum();
    let count = s.rows.len();
    let search = row()
        .w(190.0)
        .h(26.0)
        .ml(8.0)
        .px(8.0)
        .gap(6.0)
        .radius(7.0)
        .center_y()
        .fill(t::FIELD)
        .border(1.0, t::RULE_STRONG)
        .child(icon(Icon::Search, 13.0, t::MUTED))
        .child(
            text_input(&s.search, t::INPUT)
                .key("search")
                .single_line()
                .grow(1.0)
                .min_w(0.0)
                .color(t::TEXT)
                .caret(t::BLUE, t::SELECT.alpha(0.6))
                .placeholder("Search", t::FAINT)
                .label("Search torrents")
                .on_edit(|s: &mut State| &mut s.search)
                .on_input_change(|s: &mut State, _: &mut Tx| s.recompute_rows()),
        );
    let mut bar = row().key("toolbar").w_full().h(TITLEBAR).pr(12.0).gap(6.0).center_y().fill(t::BAR).border_sides([0.0, 0.0, 1.0, 0.0], t::EDGE).window_drag();
    if s.sidebar_open {
        bar = bar.pl(16.0);
    } else {
        bar = bar.pl(leading_inset.max(12.0) + 4.0).child(tool(Icon::Sidebar, "Show Sidebar", ui(Ui::Sidebar { open: Some(true) }), true));
    }
    bar.child(text(s.filter.title()).font(t::TITLE).color(t::TEXT).truncate())
        .child(text(format!("{count} {} · {}", if count == 1 { "item" } else { "items" }, size(total))).font(t::SMALL).color(t::MUTED).ml(2.0))
        .child(el().grow(1.0))
        .child(tool(Icon::Plus, "Add Torrent (⌘O)", ui(Ui::Dialog { name: "add".into() }), true))
        .child(sep())
        .child(tool(Icon::Play, "Resume", Command::Resume { targets: sel.clone() }, any))
        .child(tool(Icon::Pause, "Pause", Command::Pause { targets: sel }, any))
        .child(tool(Icon::Trash, "Remove…", ui(Ui::Dialog { name: "remove".into() }), any))
        .child(sep())
        .child(tool(Icon::Sliders, "Speed Limits (⌘,)", ui(Ui::Dialog { name: "limits".into() }), true))
        .child(tool(Icon::Folder, "Watch a Folder", ui(Ui::Dialog { name: "watch".into() }), true))
        .child(tool(Icon::Panel, if s.inspector_open { "Hide Inspector (⌘I)" } else { "Show Inspector (⌘I)" }, ui(Ui::Inspector { open: Some(!s.inspector_open) }), true))
        .child(tool(Icon::Command, "Command Palette (⌘K)", ui(Ui::Palette { open: Some(true), query: None }), true))
        .child(search)
}

fn header(s: &State) -> El {
    let mut h = row().key("table-header").w_full().h(24.0).pl(16.0).pr(14.0).gap(GAP).center_y().fill(t::HEADER).border_sides([0.0, 0.0, 1.0, 0.0], t::RULE).role("row");
    for c in Column::ALL {
        let sorted = s.sort.filter(|(sc, _)| *sc == c);
        let mut cell = row().h_full().gap(4.0).center_y().child(text(c.title()).font(t::TINY_B).color(if sorted.is_some() { t::TEXT } else { t::MUTED }));
        if let Some((_, desc)) = sorted {
            cell = cell.child(text(if desc { "▾" } else { "▴" }).font(t::TINY_B).color(t::BLUE_TEXT));
        }
        if !left(c) {
            cell = cell.justify_end();
        }
        let cell = act(sized(c, cell).label(format!("Sort by {}", c.title())), ui(Ui::Sort { column: c.id().into(), desc: None })).role("columnheader");
        h = h.child(cell);
    }
    h
}

fn progress(tor: &Torrent, selected: bool) -> El {
    let pct = tor.progress();
    let fill = if selected { t::WHITE } else { bar_color(tor.state) };
    let track = el().grow(1.0).h(4.0).radius(2.0).fill(if selected { t::WHITE.alpha(0.28) } else { t::TRACK }).child(el().h(4.0).w_frac(pct.clamp(0.0, 1.0)).radius(2.0).fill(fill));
    row().gap(8.0).center_y().child(track).child(text(format!("{:.0}%", (pct * 100.0).floor())).font(t::BODY).color(if selected { t::WHITE } else { t::TEXT_2 }).w(32.0).justify_end())
}

fn cell(c: Column, tor: &Torrent, selected: bool) -> El {
    let ink = if selected { t::WHITE } else { t::TEXT_2 };
    let dash = |b: u64| if b == 0 { "—".to_owned() } else { speed(b) };
    let body = match c {
        Column::Name => text(tor.name.clone()).font(t::BODY).color(if selected { t::WHITE } else { t::TEXT }).truncate().help_when_truncated(tor.name.clone()),
        Column::Size => text(size(tor.size)).font(t::BODY).color(ink),
        Column::Progress => return sized(c, progress(tor, selected)),
        Column::Status => text(match (&tor.state, &tor.error) {
            (TorrentState::Error, Some(_)) => "Error".to_owned(),
            (st, _) => st.label().to_owned(),
        })
        .font(t::BODY)
        .color(if selected { t::WHITE } else { state_color(tor.state) })
        .truncate(),
        Column::Down => text(dash(tor.down)).font(t::BODY).color(ink),
        Column::Up => text(dash(tor.up)).font(t::BODY).color(ink),
        Column::Eta => text(match tor.state {
            TorrentState::Seeding => "∞".to_owned(),
            _ => eta(tor.eta_secs),
        })
        .font(t::BODY)
        .color(ink),
        Column::Ratio => text(format!("{:.2}", tor.ratio())).font(t::BODY).color(ink),
        Column::Peers => text(format!("{}/{}", tor.peers_live, tor.peers_seen)).font(t::BODY).color(ink),
    };
    let wrap = row().center_y().child(body);
    sized(c, if left(c) { wrap } else { wrap.justify_end() })
}

fn torrent_row(s: &State, i: usize) -> El {
    let tor = &s.snap.torrents[s.rows[i]];
    let id = tor.id;
    let selected = s.selected.contains(&id);
    let mut r = row()
        .w_full()
        .h(ROW)
        .pl(16.0)
        .pr(14.0)
        .gap(GAP)
        .center_y()
        .fill(if selected {
            t::SELECT
        } else if i % 2 == 1 {
            t::ROW_ALT
        } else {
            t::BG
        })
        .role("row")
        .label(tor.name.clone())
        .checked(selected)
        .on_press_tx(move |s: &mut State, tx: &mut Tx| {
            let m = tx.mods();
            let cmd = click_selection(s, id, m.cmd, m.shift);
            dispatch(s, cmd, Origin::Ui, tx);
        })
        .on_double_press_tx(move |s: &mut State, tx: &mut Tx| {
            dispatch(s, Command::Reveal { target: id.to_string() }, Origin::Ui, tx);
        })
        .on_secondary_press(move |s: &mut State, at, tx: &mut Tx| super::menu::open(s, id, at, tx));
    if !selected {
        r = r.hover(t::HOVER);
    }
    for c in Column::ALL {
        r = r.child(cell(c, tor, selected));
    }
    r
}

fn empty(s: &State) -> El {
    let (title, hint) = if s.snap.torrents.is_empty() {
        ("No torrents yet", "Drop a .torrent file here, or press ⌘O to add a magnet link.")
    } else if !s.search.text.is_empty() {
        ("No matches", "Nothing here matches the search.")
    } else {
        ("Nothing here", "No torrents are in this group right now.")
    };
    col().key("empty").grow(1.0).center().gap(6.0).child(text(title).font(t::HEAD).color(t::TEXT_2)).child(text(hint).font(t::BODY).color(t::MUTED))
}

pub fn table(s: &State) -> El {
    let body = if s.rows.is_empty() {
        empty(s)
    } else {
        col().key("table-body").w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec::default()).child(
            col()
                .key("rows")
                .w_full()
                .growth(GrowthSpec::default())
                .virtual_rows(s.rows.len(), ROW, |s: &State, i| Cow::Owned(row_key(s.snap.torrents[s.rows[i]].id)), torrent_row)
                .virtual_rows_version(s.rows_version),
        )
    };
    col().key("table").w_full().grow(1.0).min_h(0.0).role("grid").label("Torrents").child(header(s)).child(body)
}
