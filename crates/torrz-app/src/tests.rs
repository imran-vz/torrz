//! The window driven headless: real clicks and keys against the real view, on the demo engine.

use std::sync::Arc;

use rune::{App, Env, Key, KeyEvent, Mods, Runtime};
use torrz_core::cli::{Parsed, parse_line};
use torrz_core::engine::demo::DemoEngine;
use torrz_core::{Backend, Command, TorrentState, Ui};

use crate::state::{Dialog, Exec, Origin, State, Tab, dispatch};

fn runtime() -> Runtime<State> {
    let init = || State::new(Arc::new(Backend::new(Arc::new(DemoEngine::frozen()), None)), Exec::Inline, Arc::default(), true);
    let mut rt = App::new(crate::spec()).init(init).headless_runtime((1180.0, 740.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

/// Commits the change, then lets what it mounted finish arriving: an element still animating in
/// isn't where a click would find it yet.
fn settle(rt: &mut Runtime<State>) {
    rt.frame(0.0);
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }
}

fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key:?}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn key(rt: &mut Runtime<State>, key: Key, cmd: bool) {
    rt.key(KeyEvent { key, mods: Mods { cmd, ..Mods::default() } });
    settle(rt);
}

fn run(rt: &mut Runtime<State>, cmd: Command) {
    rt.update(|s, tx| {
        let r = dispatch(s, cmd, Origin::Cli, tx);
        assert!(r.ok, "{}", r.message);
    });
    settle(rt);
}

fn state_of(rt: &Runtime<State>, id: u64) -> TorrentState {
    rt.state.torrent(id).map(|t| t.state).unwrap_or(TorrentState::Error)
}

#[test]
fn clicking_a_row_selects_it_and_the_toolbar_pauses_it() {
    let mut rt = runtime();
    click(&mut rt, "torrent-2");
    assert_eq!(rt.state.selected, [2]);
    assert_eq!(rt.state.snap.detail.as_ref().map(|d| d.id), Some(2), "the inspector has the torrent's detail");
    click(&mut rt, "torrz pause selected");
    assert_eq!(state_of(&rt, 2), TorrentState::Paused);
    click(&mut rt, "torrz resume selected");
    assert_eq!(state_of(&rt, 2), TorrentState::Downloading);
    // Both clicks are in the log as the lines a terminal would take.
    let log: Vec<&str> = rt.state.log.iter().map(|l| l.cli.as_str()).collect();
    assert_eq!(log, ["torrz ui select 2", "torrz pause selected", "torrz resume selected"]);
}

#[test]
fn the_palette_opens_filters_and_runs_what_is_highlighted() {
    let mut rt = runtime();
    click(&mut rt, "torrent-1");
    key(&mut rt, Key::Command('k'), true);
    assert!(rt.state.palette.is_some());
    key(&mut rt, Key::Text("pause".into()), false);
    assert_eq!(rt.state.palette.as_ref().map(|p| p.query.text.as_str()), Some("pause"));
    let first = crate::state::palette_items(&rt.state, "pause").remove(0);
    assert_eq!(first.cmd.to_cli(), "torrz pause 1");
    key(&mut rt, Key::Enter, false);
    assert!(rt.state.palette.is_none(), "running a row closes the palette");
    assert_eq!(state_of(&rt, 1), TorrentState::Paused);
}

#[test]
fn the_cli_and_the_window_end_in_the_same_place() {
    // The same three steps, once as clicks and once as commands.
    let mut by_hand = runtime();
    click(&mut by_hand, "torrz ui filter downloading");
    click(&mut by_hand, "torrent-3");
    click(&mut by_hand, "torrz ui tab files");

    let mut by_cli = runtime();
    for line in ["torrz ui filter downloading", "torrz ui select 3", "torrz ui tab files"] {
        let Ok(Parsed::Run(cmd)) = parse_line(line) else { panic!("{line} should parse") };
        run(&mut by_cli, cmd);
    }
    let strip = |mut v: serde_json::Value| {
        // Times and who asked differ; what the window shows must not.
        v["log"] = serde_json::Value::Null;
        v["toast"] = serde_json::Value::Null;
        v
    };
    assert_eq!(strip(by_hand.state.ui_json()), strip(by_cli.state.ui_json()));
    assert_eq!(by_hand.state.rows.len(), 6);
}

#[test]
fn dialogs_work_from_fields_to_confirm() {
    let mut rt = runtime();
    click(&mut rt, "torrz ui dialog add");
    assert!(matches!(rt.state.dialog, Dialog::Add { .. }));
    key(&mut rt, Key::Text("magnet:?xt=urn:btih:abc&dn=Example+Film".into()), false);
    click(&mut rt, "torrz ui field paused on");
    click(&mut rt, "torrz ui confirm");
    assert!(matches!(rt.state.dialog, Dialog::None));
    let added = rt.state.snap.torrents.last().expect("a torrent");
    assert_eq!((added.name.as_str(), added.state), ("Example Film", TorrentState::Paused));

    // Removing needs a selection, and says so instead of opening an empty dialog.
    rt.update(|s, tx| assert!(!dispatch(s, Command::Ui(Ui::Dialog { name: "remove".into() }), Origin::Cli, tx).ok));
    click(&mut rt, "torrent-13");
    key(&mut rt, Key::Backspace, false);
    assert!(matches!(rt.state.dialog, Dialog::Remove { .. }));
    click(&mut rt, "torrz ui confirm");
    assert_eq!(rt.state.snap.torrents.len(), 12);
    assert!(rt.state.selected.is_empty(), "a removed torrent leaves the selection");
}

#[test]
fn sorting_and_search_change_the_rows() {
    let mut rt = runtime();
    click(&mut rt, "torrz ui sort size");
    let ids = |rt: &Runtime<State>| rt.state.rows.iter().map(|&i| rt.state.snap.torrents[i].id).collect::<Vec<_>>();
    assert_eq!(ids(&rt)[0], 11, "smallest first");
    click(&mut rt, "torrz ui sort size");
    assert_eq!(ids(&rt)[0], 4, "a second click reverses");
    run(&mut rt, Command::Ui(Ui::Search { query: "iso".into() }));
    assert_eq!(ids(&rt).len(), 4);
    key(&mut rt, Key::Escape, false);
    assert_eq!(ids(&rt).len(), 12, "Escape clears the search");
}

/// The promise the app is built on: nothing in the window can be pressed that the command line
/// cannot also do. Every control's key is its own `torrz …` line, and it must parse.
#[test]
fn every_control_in_every_screen_has_a_command_line() {
    // Controls whose keys are names rather than lines, and the line that does the same.
    let named = |key: &str| -> Option<&'static str> {
        Some(match key {
            k if k.starts_with("torrent-") => "torrz ui select 1",
            k if k.starts_with("palette-item-") => "torrz ui palette-run",
            "search" => "torrz ui search x",
            "palette-input" => "torrz ui palette on --query x",
            "dialog-source" | "choose-file" => "torrz ui field source x",
            "dialog-down" => "torrz ui field down 1M",
            "dialog-up" => "torrz ui field up 1M",
            _ => return None,
        })
    };
    let interactive = ["button", "tab", "checkbox", "switch", "columnheader", "option", "textbox", "searchbox", "textfield"];
    let screens: Vec<(&str, Vec<Command>)> = vec![
        ("empty selection", vec![]),
        ("selected", vec![Command::Ui(Ui::Select { targets: vec!["1".into()] })]),
        ("sidebar hidden", vec![Command::Ui(Ui::Sidebar { open: Some(false) })]),
        ("palette", vec![Command::Ui(Ui::Select { targets: vec!["1".into()] }), Command::Ui(Ui::Palette { open: Some(true), query: None })]),
        ("add dialog", vec![Command::Ui(Ui::Dialog { name: "add".into() })]),
        ("limits dialog", vec![Command::Ui(Ui::Dialog { name: "limits".into() })]),
        ("remove dialog", vec![Command::Ui(Ui::Select { targets: vec!["1".into()] }), Command::Ui(Ui::Dialog { name: "remove".into() })]),
    ];
    let mut checked = 0;
    let mut check = |name: &str, rt: &Runtime<State>| {
        for e in rt.elements() {
            let pressable = interactive.contains(&e.role) || (e.role == "row" && e.key.as_deref().is_some_and(|k| k.starts_with("torrent-")));
            if !pressable || e.inert {
                continue;
            }
            let key = e.key.clone().unwrap_or_else(|| panic!("{name}: a {} labelled {:?} has no key, so no command", e.role, e.label));
            let line = named(&key).map(str::to_owned).unwrap_or(key.clone());
            assert!(matches!(parse_line(&line), Ok(Parsed::Run(_))), "{name}: the {} {key:?} is not a command the CLI accepts", e.role);
            checked += 1;
        }
    };
    for (name, steps) in screens {
        let mut rt = runtime();
        for c in steps {
            run(&mut rt, c);
        }
        check(name, &rt);
    }
    for tab in Tab::ALL {
        let mut rt = runtime();
        run(&mut rt, Command::Ui(Ui::Select { targets: vec!["1".into()] }));
        run(&mut rt, Command::Ui(Ui::Tab { name: tab.id().into() }));
        check(tab.id(), &rt);
    }
    assert!(checked > 300, "only {checked} controls were found: the roles this test looks for have changed");
}
