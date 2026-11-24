//! The source list: library filters, categories, trackers, and the alternate-limits switch.

use rune::{El, ScrollSpec, col, el, row, text};
use torrz_core::{Command, Ui};

use super::{TITLEBAR, act, ui};
use crate::state::{Filter, State};
use crate::theme::{self as t, Icon, icon};

pub const WIDTH: f32 = 196.0;

fn item(s: &State, lead: El, f: Filter) -> El {
    let on = s.filter == f;
    let count = s.snap.torrents.iter().filter(|x| f.matches(x)).count();
    let r = row()
        .w_full()
        .h(26.0)
        .px(8.0)
        .gap(8.0)
        .radius(6.0)
        .center_y()
        .fill(if on { t::SIDE_ON } else { t::SIDE_ON.alpha(0.0) })
        .hover(t::HOVER)
        .child(el().size(14.0, 14.0).center().child(lead))
        .child(text(f.title()).font(t::SIDE).color(if on { t::WHITE } else { t::TEXT_2 }).grow(1.0).truncate())
        .child(text(count.to_string()).font(t::SMALL).color(t::MUTED))
        .label(f.title())
        .checked(on);
    act(r, ui(Ui::Filter { name: f.name() }))
}

fn heading(title: &str) -> El {
    el().pl(8.0).pt(12.0).pb(4.0).child(super::section(title))
}

pub fn sidebar(s: &State, leading_inset: f32) -> El {
    let lib = [
        (Icon::All, Filter::All),
        (Icon::Down, Filter::Downloading),
        (Icon::Up, Filter::Seeding),
        (Icon::Check, Filter::Completed),
        (Icon::Pause, Filter::Paused),
        (Icon::Warn, Filter::Errors),
    ];
    let mut list = col().w_full().px(10.0).pb(8.0).child(el().pl(8.0).pb(4.0).child(super::section("Library")));
    for (i, f) in lib {
        list = list.child(item(s, icon(i, 14.0, t::BLUE), f));
    }

    let mut cats: Vec<&str> = s.snap.torrents.iter().filter_map(|x| x.category.as_deref()).collect();
    cats.sort_unstable();
    cats.dedup();
    if !cats.is_empty() {
        list = list.child(heading("Categories"));
        for (i, c) in cats.iter().enumerate() {
            list = list.child(item(s, el().size(8.0, 8.0).radius(4.0).fill(t::DOTS[i % t::DOTS.len()]), Filter::Category((*c).to_owned())));
        }
    }

    let mut hosts: Vec<&str> = s.snap.torrents.iter().flat_map(|x| x.trackers.iter().map(String::as_str)).collect();
    hosts.sort_unstable();
    hosts.dedup();
    let dht_only = s.snap.torrents.iter().any(|x| x.trackers.is_empty());
    if !hosts.is_empty() || dht_only {
        list = list.child(heading("Trackers"));
        for h in hosts {
            list = list.child(item(s, icon(Icon::Link, 14.0, t::BLUE), Filter::Tracker(h.to_owned())));
        }
        if dht_only {
            list = list.child(item(s, icon(Icon::Link, 14.0, t::BLUE), Filter::Tracker(String::new())));
        }
    }

    let alt = s.snap.session.alt_limits;
    let knob = el().size(12.0, 12.0).radius(6.0).fill(t::WHITE).absolute(if alt { 14.0 } else { 2.0 }, 2.0);
    let switch = el().size(28.0, 16.0).radius(8.0).fill(if alt { t::BLUE } else { t::KNOB_OFF }).child(knob);
    let footer = act(
        row()
            .w_full()
            .h(40.0)
            .px(18.0)
            .gap(8.0)
            .center_y()
            .border_sides([1.0, 0.0, 0.0, 0.0], t::RULE_STRONG.alpha(0.7))
            .child(icon(Icon::Gauge, 14.0, if alt { t::AMBER } else { t::MUTED }))
            .child(text("Alt speed limits").font(t::SMALL).color(t::TEXT_2).grow(1.0))
            .child(switch)
            .label("Alternate speed limits")
            .checked(alt),
        Command::AltLimits { on: Some(!alt) },
    )
    .role("switch");

    col()
        .key("sidebar")
        .w(WIDTH)
        .h_full()
        .gradient(t::SIDE_TOP, t::SIDE_BOTTOM)
        .border_sides([0.0, 1.0, 0.0, 0.0], t::EDGE)
        // The band beside the traffic lights drags the window and holds the sidebar's toggle.
        .child(
            row()
                .w_full()
                .h(TITLEBAR)
                .pl(leading_inset.max(12.0))
                .pr(10.0)
                .center_y()
                .window_drag()
                .child(el().grow(1.0))
                .child(super::tool(Icon::Sidebar, "Hide Sidebar", ui(Ui::Sidebar { open: Some(false) }), true)),
        )
        .child(list.scroll(ScrollSpec::default()).grow(1.0).min_h(0.0))
        .child(footer)
}
