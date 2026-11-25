//! The inspector under the table: one torrent's summary, files, peers, trackers and pieces, and
//! the log of every command the window has run.

use std::collections::VecDeque;

use rune::{El, ScrollSpec, Shape, canvas, col, el, row, text};
use torrz_core::model::{eta, size, speed};
use torrz_core::{Command, Detail, Peer, Torrent, TorrentState, Ui};

use super::{act, bar_color, ui};
use crate::state::{HISTORY, Origin, State, Tab};
use crate::theme as t;

pub const HEIGHT: f32 = 244.0;
const GRAPH_W: f32 = 306.0;
const GRAPH_H: f32 = 118.0;

fn tabs(s: &State) -> El {
    let mut strip = row().key("tabs").w_full().h(34.0).px(14.0).gap(2.0).center_y().border_sides([0.0, 0.0, 1.0, 0.0], t::RULE).role("tablist");
    for tab in Tab::ALL {
        let on = s.tab == tab;
        let b = row()
            .h(22.0)
            .px(10.0)
            .radius(5.0)
            .center_y()
            .fill(if on { t::RULE_STRONG } else { t::RULE_STRONG.alpha(0.0) })
            .hover(t::HOVER)
            .child(text(tab.title()).font(if on { t::SMALL_M } else { t::SMALL }).color(if on { t::WHITE } else { t::MUTED }))
            .label(tab.title())
            .checked(on);
        strip = strip.child(act(b, ui(Ui::Tab { name: tab.id().into() })).role("tab"));
    }
    // The inspected torrent's own command line, as a standing hint that the CLI sees the same thing.
    let hint = match (s.tab, s.inspected()) {
        (Tab::Log, _) => "torrz ui state".to_owned(),
        (_, Some(tor)) => Command::Show { target: tor.id.to_string() }.to_cli(),
        _ => "torrz list".to_owned(),
    };
    strip.child(el().grow(1.0)).child(text(hint).font(t::MONO).color(t::FAINT).selectable(true))
}

fn kv(label: &str, value: String) -> El {
    row()
        .grow(1.0)
        .min_w(0.0)
        .pb(5.0)
        .gap(8.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t::RULE)
        .child(text(label).font(t::SMALL).color(t::MUTED))
        .child(el().grow(1.0))
        .child(text(value).font(t::SMALL).color(t::TEXT_2).truncate())
}

fn piece_bar(tor: &Torrent, d: Option<&Detail>, cells: usize, h: f32) -> El {
    let ink = bar_color(tor.state);
    let mut bar = row().w_full().h(h).radius(3.0).clip_rounded(3.0).fill(t::TRACK);
    match d.filter(|d| d.id == tor.id && !d.piece_map.is_empty()) {
        Some(d) => {
            let n = d.piece_map.len();
            for c in 0..cells.min(n) {
                let (lo, hi) = (c * n / cells.min(n), ((c + 1) * n / cells.min(n)).max(c * n / cells.min(n) + 1));
                let avg = d.piece_map[lo..hi.min(n)].iter().map(|v| *v as u32).sum::<u32>() as f32 / (hi.min(n) - lo) as f32 / 255.0;
                bar = bar.child(el().grow(1.0).h_full().fill(ink.alpha(avg)));
            }
        }
        // No piece map yet: a plain progress bar says the same thing more coarsely.
        None => bar = bar.child(el().w_frac(tor.progress()).h_full().fill(ink)),
    }
    bar
}

fn graph(history: &VecDeque<(u64, u64)>, down: u64, up: u64) -> El {
    let peak = history.iter().map(|(d, u)| (*d).max(*u)).max().unwrap_or(0).max(64 * 1024) as f32 * 1.15;
    let line = |pick: fn(&(u64, u64)) -> u64| -> Vec<[f32; 2]> {
        let off = HISTORY - history.len();
        history.iter().enumerate().map(|(i, v)| [(off + i) as f32 / (HISTORY - 1) as f32 * GRAPH_W, GRAPH_H - 1.0 - pick(v) as f32 / peak * (GRAPH_H - 4.0)]).collect()
    };
    let mut shape = Shape::new();
    for g in 1..4 {
        let y = GRAPH_H * g as f32 / 4.0;
        shape = shape.stroke(vec![[0.0, y], [GRAPH_W, y]], 1.0, t::WHITE.alpha(0.05));
    }
    if history.len() > 1 {
        let d = line(|v| v.0);
        let mut area = d.clone();
        area.push([GRAPH_W, GRAPH_H]);
        area.push([d[0][0], GRAPH_H]);
        shape = shape.fill(area, t::BLUE.alpha(0.14)).stroke(line(|v| v.1), 1.4, t::GREEN).stroke(d, 1.6, t::BLUE);
    }
    col()
        .key("graph")
        .w(GRAPH_W + 24.0)
        .h_full()
        .px(12.0)
        .py(10.0)
        .gap(6.0)
        .radius(8.0)
        .fill(t::BG)
        .border(1.0, t::RULE_STRONG.alpha(0.8))
        .child(
            row()
                .w_full()
                .gap(14.0)
                .child(text(format!("↓ {}", speed(down).replace('—', "0 B/s"))).font(t::SMALL_M).color(t::BLUE_TEXT))
                .child(text(format!("↑ {}", speed(up).replace('—', "0 B/s"))).font(t::SMALL_M).color(t::GREEN_TEXT))
                .child(el().grow(1.0))
                .child(text(format!("peak {}", speed((peak / 1.15) as u64))).font(t::SMALL).color(t::MUTED)),
        )
        .child(canvas(shape).size(GRAPH_W, GRAPH_H))
}

fn general(s: &State) -> El {
    let Some(tor) = s.inspected() else {
        let ss = &s.snap.session;
        let left = col().grow(1.0).min_w(0.0).gap(8.0).child(text("No torrent selected").font(t::HEAD).color(t::TEXT)).child(
            text("Select a torrent to see its pieces, files and peers. The graph shows the whole session.").font(t::BODY).color(t::MUTED).wrap(),
        );
        return row().w_full().grow(1.0).min_h(0.0).px(16.0).py(14.0).gap(22.0).child(left).child(graph(&s.session_history, ss.down, ss.up));
    };
    let d = s.snap.detail.as_ref().filter(|d| d.id == tor.id);
    let have = d.map(|d| d.pieces_have).unwrap_or(0);
    let grid = |cells: [El; 3]| row().w_full().gap(18.0).children(cells);
    let left = col()
        .grow(1.0)
        .min_w(0.0)
        .child(text(tor.name.clone()).font(t::HEAD).color(t::TEXT).truncate().selectable(true))
        .child(el().h(8.0))
        .child(piece_bar(tor, d, 150, 14.0))
        .child(
            row()
                .w_full()
                .pt(4.0)
                .pb(12.0)
                .child(text(format!("{} of {} · {} pieces", size(tor.done), size(tor.size), tor.pieces)).font(t::SMALL).color(t::MUTED))
                .child(el().grow(1.0))
                .child(text(format!("{:.1}%", tor.progress() * 100.0)).font(t::SMALL).color(t::MUTED)),
        )
        .child(
            col()
                .w_full()
                .gap(9.0)
                .child(grid([kv("Status", tor.error.clone().unwrap_or_else(|| tor.state.label().into())), kv("Downloaded", size(tor.done)), kv("Uploaded", size(tor.uploaded))]))
                .child(grid([kv("Ratio", format!("{:.2}", tor.ratio())), kv("Peers", format!("{} of {}", tor.peers_live, tor.peers_seen)), kv("Time left", eta(tor.eta_secs))]))
                .child(grid([kv("Pieces", format!("{have} of {}", tor.pieces)), kv("Save to", tor.save_path.clone()), kv("Hash", format!("{}…{}", &tor.hash[..4.min(tor.hash.len())], &tor.hash[tor.hash.len().saturating_sub(4)..]))])),
        );
    row().w_full().grow(1.0).min_h(0.0).px(16.0).py(14.0).gap(22.0).child(left).child(graph(&s.torrent_history.1, tor.down, tor.up))
}

fn placeholder(msg: &str) -> El {
    col().w_full().grow(1.0).center().child(text(msg).font(t::BODY).color(t::MUTED))
}

fn list_head(cols: &[(&str, Option<f32>, bool)]) -> El {
    let mut h = row().w_full().h(22.0).px(16.0).gap(12.0).center_y().border_sides([0.0, 0.0, 1.0, 0.0], t::RULE);
    for (title, w, right) in cols {
        let c = row().child(text(*title).font(t::TINY_B).color(t::MUTED));
        let c = if *right { c.justify_end() } else { c };
        h = h.child(match w {
            Some(w) => c.w(*w),
            None => c.grow(1.0).min_w(0.0),
        });
    }
    h
}

fn files(s: &State) -> El {
    let Some(tor) = s.inspected() else { return placeholder("Select a torrent to see its files.") };
    let Some(d) = s.snap.detail.as_ref().filter(|d| d.id == tor.id && !d.files.is_empty()) else { return placeholder("Waiting for the file list…") };
    let mut list = col().w_full();
    for f in &d.files {
        let (id, index) = (tor.id.to_string(), f.index);
        let cmd = if f.included { Command::Files { target: id, include: vec![], skip: vec![index] } } else { Command::Files { target: id, include: vec![index], skip: vec![] } };
        let check = el()
            .size(13.0, 13.0)
            .radius(3.5)
            .center()
            .fill(if f.included { t::BLUE } else { t::BLUE.alpha(0.0) })
            .border(1.2, if f.included { t::BLUE } else { t::GREY })
            .children(f.included.then(|| t::icon(t::Icon::Check, 9.0, t::WHITE)));
        let pct = if f.size == 0 { 1.0 } else { (f.done as f32 / f.size as f32).min(1.0) };
        let ink = if f.included { t::TEXT_2 } else { t::FAINT };
        let line = row()
            .w_full()
            .h(26.0)
            .px(16.0)
            .gap(12.0)
            .center_y()
            .hover(t::HOVER)
            .child(check)
            .child(text(f.path.clone()).font(t::BODY).color(ink).grow(1.0).truncate())
            .child(el().w(120.0).h(4.0).radius(2.0).fill(t::TRACK).child(el().h(4.0).w_frac(pct).radius(2.0).fill(if f.included { bar_color(tor.state) } else { t::GREY })))
            .child(row().w(44.0).justify_end().child(text(format!("{:.0}%", (pct * 100.0).floor())).font(t::SMALL).color(ink)))
            .child(row().w(72.0).justify_end().child(text(size(f.size)).font(t::SMALL).color(ink)))
            .label(format!("Download {}", f.path))
            .checked(f.included);
        list = list.child(act(line, cmd).role("checkbox"));
    }
    col().w_full().grow(1.0).min_h(0.0).child(list_head(&[("", Some(13.0), false), ("File", None, false), ("Progress", Some(120.0), false), ("", Some(44.0), true), ("Size", Some(72.0), true)])).child(list.scroll(ScrollSpec::default()).grow(1.0).min_h(0.0))
}

/// The shade of the `rank`th biggest source in the "where it comes from" bar.
fn source_shade(rank: usize) -> rune::Color {
    t::BLUE.alpha([1.0, 0.78, 0.62, 0.5, 0.4, 0.32][rank.min(5)])
}

/// Where the download is coming from: one segment a peer, by its share of what arrives now (or,
/// when nothing is arriving, of what it has given), the biggest first.
fn sources(peers: &[Peer]) -> El {
    let now: u64 = peers.iter().map(|p| p.down).sum();
    let by = |p: &Peer| if now > 0 { p.down } else { p.downloaded };
    let total: u64 = peers.iter().map(by).sum();
    let mut bar = row().w(300.0).h(8.0).radius(4.0).clip_rounded(4.0).gap(1.0).fill(t::TRACK);
    if total == 0 {
        return bar;
    }
    let mut ranked: Vec<&Peer> = peers.iter().filter(|p| by(p) > 0).collect();
    ranked.sort_by_key(|p| std::cmp::Reverse(by(p)));
    let shown = ranked.len().min(6);
    for (i, p) in ranked[..shown].iter().enumerate() {
        let share = by(p) as f32 / total as f32;
        let who = if p.client.is_empty() { p.addr.clone() } else { format!("{} · {}", p.client, p.addr) };
        bar = bar.child(el().w_frac(share).h_full().fill(source_shade(i)).help(format!("{who}: {:.0}%", share * 100.0)));
    }
    let rest: u64 = ranked[shown..].iter().map(|p| by(p)).sum();
    if rest > 0 {
        bar = bar.child(el().w_frac(rest as f32 / total as f32).h_full().fill(t::GREY).help(format!("{} more peers", ranked.len() - shown)));
    }
    bar
}

fn state_dot(state: &str) -> rune::Color {
    match state {
        "live" => t::GREEN,
        "connecting" | "queued" => t::AMBER,
        _ => t::GREY,
    }
}

fn peers(s: &State) -> El {
    let Some(tor) = s.inspected() else { return placeholder("Select a torrent to see its peers.") };
    let d = s.snap.detail.as_ref().filter(|d| d.id == tor.id);
    let Some(d) = d.filter(|d| !d.peers.is_empty()) else { return placeholder(if tor.peers_seen == 0 { "No peers yet." } else { "Not connected to any peers right now." }) };
    let mut ps: Vec<&Peer> = d.peers.iter().collect();
    ps.sort_by(|a, b| b.down.cmp(&a.down).then(b.downloaded.cmp(&a.downloaded)).then_with(|| a.addr.cmp(&b.addr)));
    let fastest = ps.iter().map(|p| p.down).max().unwrap_or(0).max(1);
    let live = ps.iter().filter(|p| p.state == "live").count();
    let kinds = |k: &str| ps.iter().filter(|p| p.kind == k).count();
    let summary = [format!("{live} connected"), format!("{} seen", tor.peers_seen.max(ps.len() as u32)), format!("{} TCP", kinds("tcp")), format!("{} uTP", kinds("utp"))].join(" · ");
    let head = row()
        .key("peer-summary")
        .w_full()
        .h(34.0)
        .px(16.0)
        .gap(12.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], t::RULE)
        .child(text(summary).font(t::SMALL).color(t::MUTED))
        .child(el().grow(1.0))
        .child(text(if ps.iter().any(|p| p.down > 0) { "Arriving from" } else { "Came from" }).font(t::SMALL).color(t::FAINT))
        .child(sources(&d.peers));

    let dim = |x: String, c| text(x).font(t::SMALL).color(c);
    let mut list = col().w_full();
    for p in ps {
        let badge = match p.kind.as_str() {
            "" => el().w(34.0),
            k => row().w(34.0).child(row().h(16.0).px(5.0).radius(4.0).center().fill(t::WHITE.alpha(0.08)).child(text(if k == "utp" { "uTP".to_owned() } else { k.to_uppercase() }).font(t::TINY_B).color(t::MUTED))),
        };
        let down = row()
            .w(130.0)
            .gap(8.0)
            .center_y()
            .child(el().grow(1.0).h(4.0).radius(2.0).fill(t::TRACK).child(el().h(4.0).w_frac(p.down as f32 / fastest as f32).radius(2.0).fill(t::BLUE)))
            .child(row().w(68.0).justify_end().child(dim(if p.down == 0 { "—".into() } else { speed(p.down) }, t::BLUE_TEXT)));
        let per_piece = if p.pieces == 0 { "—".to_owned() } else { format!("{:.1} s", p.piece_ms as f32 / 1000.0) };
        list = list.child(
            row()
                .w_full()
                .h(24.0)
                .px(16.0)
                .gap(12.0)
                .center_y()
                .hover(t::HOVER)
                .child(el().size(7.0, 7.0).radius(3.5).fill(state_dot(&p.state)).help(p.state.clone()))
                .child(text(p.addr.clone()).font(t::MONO).color(t::TEXT_2).w(220.0).truncate().selectable(true))
                .child(text(if p.client.is_empty() { "—".to_owned() } else { p.client.clone() }).font(t::BODY).color(t::TEXT_2).grow(1.0).min_w(0.0).truncate())
                .child(badge)
                .child(down)
                .child(row().w(68.0).justify_end().child(dim(if p.up == 0 { "—".into() } else { speed(p.up) }, t::GREEN_TEXT)))
                .child(row().w(70.0).justify_end().child(dim(size(p.downloaded), t::TEXT_2)))
                .child(row().w(56.0).justify_end().child(dim(per_piece, t::MUTED)).help("Average time a piece from this peer took")),
        );
    }
    col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .child(head)
        .child(list_head(&[("", Some(7.0), false), ("Address", Some(220.0), false), ("Client", None, false), ("", Some(34.0), false), ("Down", Some(130.0), true), ("Up", Some(68.0), true), ("Got", Some(70.0), true), ("Piece", Some(56.0), true)]))
        .child(list.scroll(ScrollSpec::default()).grow(1.0).min_h(0.0))
}

fn trackers(s: &State) -> El {
    let Some(tor) = s.inspected() else { return placeholder("Select a torrent to see its trackers.") };
    let mut list = col().w_full().px(16.0).py(10.0).gap(8.0);
    let line = |dot, name: String, note: &str| row().gap(8.0).center_y().child(el().size(7.0, 7.0).radius(3.5).fill(dot)).child(text(name).font(t::BODY).color(t::TEXT_2)).child(text(note).font(t::SMALL).color(t::MUTED));
    for h in &tor.trackers {
        list = list.child(line(t::GREEN, h.clone(), "tracker"));
    }
    list = list.child(line(if s.snap.session.dht_nodes > 0 { t::GREEN } else { t::AMBER }, "DHT".into(), &format!("{} nodes · peer exchange and local discovery on", s.snap.session.dht_nodes)));
    if tor.trackers.is_empty() {
        list = list.child(text("This torrent has no trackers; its peers come from the DHT.").font(t::SMALL).color(t::MUTED));
    }
    let auto = s.snap.session.auto_trackers;
    let mut tools = row().w_full().pt(4.0).gap(14.0).center_y();
    tools = if tor.private {
        tools.child(text("Private torrent: it only talks to its own tracker, so Torrz adds no public ones.").font(t::SMALL).color(t::MUTED))
    } else {
        tools.child(act(
            row().h(24.0).px(10.0).gap(6.0).radius(6.0).center_y().fill(t::WHITE.alpha(0.09)).hover(t::HOVER).press(t::PRESS).child(text("Add Best Trackers").font(t::SMALL_M).color(t::TEXT)).label("Add the best public trackers").help("The fastest public trackers from newTrackon, as they answer from here"),
            Command::AddTrackers { targets: vec![tor.id.to_string()], urls: Vec::new() },
        ))
    };
    list = list.child(tools.child(super::overlays::checkbox("Add them to new torrents too", auto, Command::AutoTrackers { on: Some(!auto) })));
    list.grow(1.0)
}

fn pieces(s: &State) -> El {
    let Some(tor) = s.inspected() else { return placeholder("Select a torrent to see its pieces.") };
    let Some(d) = s.snap.detail.as_ref().filter(|d| d.id == tor.id && !d.piece_map.is_empty()) else { return placeholder("No piece map yet.") };
    // Only a torrent that is downloading has pieces in flight; a paused one holds still.
    let grid = super::pieces::piece_map(&d.piece_map, &s.pieces_before, bar_color(tor.state), s.pieces_generation, tor.state == TorrentState::Downloading);
    col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .px(16.0)
        .py(12.0)
        .gap(10.0)
        .child(text(format!("{} of {} pieces · each cell is about {} pieces", d.pieces_have, tor.pieces, (tor.pieces as usize).div_ceil(d.piece_map.len().max(1)))).font(t::SMALL).color(t::MUTED))
        .child(col().w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec::default()).child(grid))
}

fn log(s: &State) -> El {
    if s.log.is_empty() {
        return placeholder("Every click and every `torrz` command shows up here as its command line.");
    }
    let mut list = col().w_full().px(16.0).py(8.0).gap(3.0);
    for l in &s.log {
        list = list.child(
            row()
                .w_full()
                .gap(10.0)
                .child(text(l.time.clone()).font(t::MONO).color(t::FAINT))
                .child(text(format!("{:<5}", l.origin.id())).font(t::MONO).color(match l.origin {
                    Origin::Ui => t::BLUE_TEXT,
                    Origin::Cli => t::AMBER,
                    Origin::Watch => t::GREEN_TEXT,
                }))
                .child(text(l.cli.clone()).font(t::MONO).color(t::TEXT_2).truncate().selectable(true))
                .child(text(format!("→ {}", l.result)).font(t::MONO).color(if l.ok { t::GREEN_TEXT } else { t::RED }).truncate().grow(1.0)),
        );
    }
    col().key("log").w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec::stick_bottom()).child(list)
}

pub fn inspector(s: &State) -> El {
    let body = match s.tab {
        Tab::General => general(s),
        Tab::Files => files(s),
        Tab::Peers => peers(s),
        Tab::Trackers => trackers(s),
        Tab::Pieces => pieces(s),
        Tab::Log => log(s),
    };
    col().key("inspector").w_full().h(HEIGHT).fill(t::PANEL).border_sides([1.0, 0.0, 0.0, 0.0], t::EDGE).child(tabs(s)).child(body.key(format!("tab-{}", s.tab.id())))
}
