//! The torrent rows' right-click menu, drawn by the system. Each item's id is the `torrz` line it
//! runs, and a pick is parsed back through the CLI's own parser before it runs, so the menu can
//! do nothing the command line can't (the tests hold every item to a line that parses).

use rune::{PointerPress, PopupItem, PopupMenu, Tx};
use torrz_core::cli::{Parsed, parse_line};
use torrz_core::{Command, TorrentState, Ui};

use super::ui;
use crate::state::{Origin, State, dispatch};

fn item(cmd: Command, title: impl Into<String>) -> PopupItem {
    PopupItem::new(cmd.to_cli(), title)
}

fn running(st: TorrentState) -> bool {
    matches!(st, TorrentState::Downloading | TorrentState::Seeding | TorrentState::Checking | TorrentState::Queued)
}

/// The menu for the selection; the torrent the inspector shows answers the one-torrent items.
pub fn torrent_menu(s: &State) -> PopupMenu {
    let Some(one) = s.inspected() else { return PopupMenu::new() };
    let ids: Vec<String> = s.selected.iter().map(|i| i.to_string()).collect();
    let picked: Vec<_> = s.selected.iter().filter_map(|id| s.torrent(*id)).collect();
    let target = one.id.to_string();

    let mut menu = PopupMenu::new();
    if picked.iter().any(|t| !running(t.state)) {
        menu = menu.item(item(Command::Resume { targets: ids.clone() }, "Resume").symbol("play.fill"));
    }
    if picked.iter().any(|t| running(t.state)) {
        menu = menu.item(item(Command::Pause { targets: ids.clone() }, "Pause").symbol("pause.fill"));
    }
    menu = menu
        .separator()
        .item(item(ui(Ui::Tab { name: "files".into() }), "Show Files").symbol("list.bullet"))
        .item(item(Command::Reveal { target: target.clone() }, "Reveal in Finder").symbol("folder"))
        .submenu(
            "Copy",
            PopupMenu::new()
                .item(item(ui(Ui::Copy { what: "name".into(), target: target.clone() }), "Name"))
                .item(item(ui(Ui::Copy { what: "hash".into(), target: target.clone() }), "Info Hash"))
                .item(item(ui(Ui::Copy { what: "magnet".into(), target }), "Magnet Link")),
        )
        .separator();

    // Every category in use, with a check by the one the whole selection shares.
    let mut categories: Vec<String> = s.snap.torrents.iter().filter_map(|t| t.category.clone()).collect();
    categories.sort_by_key(|c| c.to_lowercase());
    categories.dedup();
    let shared = |c: Option<&str>| picked.iter().all(|t| t.category.as_deref() == c);
    let mut cats = PopupMenu::new().item(item(Command::Category { targets: ids.clone(), name: None }, "None").checked(shared(None))).separator();
    for c in &categories {
        cats = cats.item(item(Command::Category { targets: ids.clone(), name: Some(c.clone()) }, c.clone()).checked(shared(Some(c))));
    }
    menu = menu.submenu("Category", cats);

    let public = picked.iter().any(|t| !t.private);
    menu.item(item(Command::AddTrackers { targets: ids, urls: Vec::new() }, "Add Best Trackers").enabled(public).symbol("antenna.radiowaves.left.and.right"))
        .separator()
        .item(item(ui(Ui::Dialog { name: "remove".into() }), if picked.len() > 1 { format!("Remove {} Torrents…", picked.len()) } else { "Remove…".into() }).symbol("trash"))
}

/// A right-click on row `id`: as in Finder, a row outside the selection becomes the selection
/// first; then the menu opens where the pointer is.
pub fn open(s: &mut State, id: u64, at: PointerPress, tx: &mut Tx) {
    if !s.selected.contains(&id) {
        dispatch(s, ui(Ui::Select { targets: vec![id.to_string()] }), Origin::Ui, tx);
    }
    tx.popup_menu(torrent_menu(s), at, |s: &mut State, picked, tx| {
        let Some(line) = picked else { return };
        match parse_line(line) {
            Ok(Parsed::Run(cmd)) => {
                dispatch(s, cmd, Origin::Ui, tx);
            }
            _ => s.say(&format!("Can't run {line}"), false, tx),
        }
    });
}
