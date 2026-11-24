//! What floats over the window: the ⌘K palette, the dialogs and the toast.

use rune::{El, FileDialogOptions, Key, KeyEvent, ScrollSpec, Tx, col, el, row, text, text_input};
use torrz_core::{Command, Ui};

use super::{act, ui};
use crate::state::{Dialog, Origin, Palette, State, Toast, dispatch, palette_items};
use crate::theme::{self as t, Icon, icon};

fn run(s: &mut State, u: Ui, tx: &mut Tx) -> bool {
    dispatch(s, Command::Ui(u), Origin::Ui, tx);
    true
}

fn scrim(key: &str, dim: f32) -> El {
    el().key(key).pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).fill(rune::Color { r: 0.0, g: 0.0, b: 0.0, a: dim })
}

fn keycap(k: &str, what: &str) -> El {
    row()
        .gap(5.0)
        .center_y()
        .child(row().h(16.0).px(4.0).radius(3.0).center_y().fill(t::WHITE.alpha(0.12)).child(text(k).font(t::SMALL).color(t::TEXT_2)))
        .child(text(what).font(t::SMALL).color(t::MUTED))
}

pub fn palette(s: &State, p: &Palette) -> El {
    let items = palette_items(s, &p.query.text);
    let index = p.index.min(items.len().saturating_sub(1));
    let input = row().w_full().h(50.0).px(16.0).gap(10.0).center_y().border_sides([0.0, 0.0, 1.0, 0.0], t::WHITE.alpha(0.1)).child(icon(Icon::Search, 16.0, t::MUTED)).child(
        text_input(&p.query, t::PALETTE_INPUT)
            .key("palette-input")
            .single_line()
            .grow(1.0)
            .min_w(0.0)
            .color(t::TEXT)
            .caret(t::BLUE, t::SELECT.alpha(0.6))
            .placeholder("Type a command or a torrent's name", t::FAINT)
            .label("Command palette")
            .autofocus()
            .on_edit(|s: &mut State| match &mut s.palette {
                Some(p) => &mut p.query,
                None => &mut s.scratch,
            })
            .on_input_change(|s: &mut State, _: &mut Tx| {
                if let Some(p) = &mut s.palette {
                    p.index = 0;
                }
            })
            .on_input_key(|s: &mut State, ev: &KeyEvent, tx: &mut Tx| match &ev.key {
                Key::Down => run(s, Ui::PaletteMove { delta: 1 }, tx),
                Key::Up => run(s, Ui::PaletteMove { delta: -1 }, tx),
                Key::Enter => run(s, Ui::PaletteRun, tx),
                Key::Escape => run(s, Ui::Cancel, tx),
                // ⌘C with nothing selected in the field copies the highlighted row's command line.
                Key::Command('c') if s.palette.as_ref().is_some_and(|p| p.query.anchor == p.query.focus) => {
                    let line = s.palette.as_ref().and_then(|p| {
                        let mut items = palette_items(s, &p.query.text);
                        (!items.is_empty()).then(|| items.swap_remove(p.index.min(items.len() - 1)).cmd.to_cli())
                    });
                    if let Some(line) = line {
                        tx.copy(line.clone());
                        s.say(&format!("Copied: {line}"), true, tx);
                    }
                    true
                }
                _ => false,
            }),
    )
    .child(text("⌘K").font(t::SMALL).color(t::FAINT));

    let mut list = col().w_full().padding(6.0);
    let mut section = "";
    for (i, it) in items.iter().enumerate() {
        if it.section != section {
            section = it.section;
            list = list.child(el().px(10.0).pt(6.0).pb(4.0).child(text(section.to_uppercase()).font(t::TINY_B).color(t::MUTED)));
        }
        let on = i == index;
        let line = it.cmd.to_cli();
        let r = row()
            .key(format!("palette-item-{i}"))
            .w_full()
            .h(32.0)
            .px(10.0)
            .gap(10.0)
            .radius(7.0)
            .center_y()
            .fill(if on { t::SELECT } else { t::SELECT.alpha(0.0) })
            .child(icon(it.icon, 14.0, if on { t::WHITE } else { t::MUTED }))
            .child(text(it.title.clone()).font(t::BODY).color(if on { t::WHITE } else { t::TEXT }).grow(1.0).truncate())
            .child(text(line).font(t::MONO).color(if on { t::WHITE.alpha(0.8) } else { t::FAINT }).truncate())
            .role("option")
            .label(it.title.clone())
            .checked(on)
            .on_press_tx(move |s: &mut State, tx: &mut Tx| {
                if let Some(p) = &mut s.palette {
                    p.index = i;
                }
                run(s, Ui::PaletteRun, tx);
            });
        list = list.child(if on { r } else { r.hover(t::HOVER) });
    }
    if items.is_empty() {
        list = list.child(el().w_full().h(64.0).center().child(text(format!("Nothing matches “{}”", p.query.text)).font(t::BODY).color(t::MUTED)));
    }

    let footer = row()
        .w_full()
        .h(30.0)
        .px(14.0)
        .gap(14.0)
        .center_y()
        .border_sides([1.0, 0.0, 0.0, 0.0], t::WHITE.alpha(0.1))
        .child(keycap("↩", "Run"))
        .child(keycap("⌘C", "Copy as CLI"))
        .child(keycap("esc", "Close"))
        .child(el().grow(1.0))
        .child(text("Same commands as the terminal").font(t::SMALL).color(t::FAINT));

    let sheet = col()
        .key("palette")
        .w(560.0)
        .max_h(420.0)
        .radius(12.0)
        .clip_rounded(12.0)
        .fill(t::PALETTE)
        .border(1.0, t::WHITE.alpha(0.12))
        .shadow(18.0, 50.0, rune::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.55 })
        .role("dialog")
        .label("Command palette")
        .modal(true, |s: &mut State| s.palette = None)
        .child(input)
        .child(col().key("palette-list").w_full().shrink().min_h(0.0).scroll(ScrollSpec::default()).role("listbox").child(list))
        .child(footer);
    scrim("palette-scrim", 0.28).child(col().w_full().h_full().pt(96.0).items_center().child(sheet))
}

fn field(key: &'static str, edit: &rune::TextEdit, hint: &str, label: &str, get: fn(&mut State) -> &mut rune::TextEdit) -> El {
    row().w_full().h(30.0).px(9.0).radius(7.0).center_y().fill(t::FIELD).border(1.0, t::RULE_STRONG).child(
        text_input(edit, t::INPUT)
            .key(key)
            .single_line()
            .grow(1.0)
            .min_w(0.0)
            .color(t::TEXT)
            .caret(t::BLUE, t::SELECT.alpha(0.6))
            .placeholder(hint, t::FAINT)
            .label(label)
            .on_edit(get)
            .on_submit(|s: &mut State, tx: &mut Tx| {
                run(s, Ui::Confirm, tx);
            }),
    )
}

fn button(label: &str, cmd: Command, fill: rune::Color, ink: rune::Color) -> El {
    act(row().h(28.0).px(14.0).radius(7.0).center().fill(fill).hover(t::HOVER).press(t::PRESS).child(text(label).font(t::BODY_M).color(ink)).label(label), cmd)
}

fn checkbox(label: &str, on: bool, cmd: Command) -> El {
    let box_ = el().size(14.0, 14.0).radius(4.0).center().fill(if on { t::BLUE } else { t::BLUE.alpha(0.0) }).border(1.2, if on { t::BLUE } else { t::GREY }).children(on.then(|| icon(Icon::Check, 10.0, t::WHITE)));
    act(row().gap(8.0).center_y().child(box_).child(text(label).font(t::BODY).color(t::TEXT_2)).label(label).checked(on), cmd).role("checkbox")
}

pub fn dialog(s: &State) -> El {
    let cancel = || button("Cancel", ui(Ui::Cancel), t::WHITE.alpha(0.09), t::TEXT);
    let onoff = |v: bool| if v { "on" } else { "off" }.to_owned();
    let (title, body, confirm): (String, El, El) = match &s.dialog {
        Dialog::None => return el(),
        Dialog::Add { source, paused } => (
            "Add Torrent".into(),
            col()
                .w_full()
                .gap(12.0)
                .child(text("Paste a magnet link or a URL, or choose a .torrent file.").font(t::BODY).color(t::MUTED))
                .child(
                    row()
                        .w_full()
                        .gap(8.0)
                        .child(field("dialog-source", source, "magnet:?xt=urn:btih:…", "Magnet link, URL or file", |s| match &mut s.dialog {
                            Dialog::Add { source, .. } => source,
                            _ => &mut s.scratch,
                        }).grow(1.0).min_w(0.0))
                        .child(
                            row()
                                .key("choose-file")
                                .h(30.0)
                                .px(12.0)
                                .radius(7.0)
                                .center()
                                .fill(t::WHITE.alpha(0.09))
                                .hover(t::HOVER)
                                .press(t::PRESS)
                                .role("button")
                                .label("Choose File…")
                                .child(text("Choose File…").font(t::BODY).color(t::TEXT))
                                .on_press_tx(|_: &mut State, tx: &mut Tx| {
                                    let options = FileDialogOptions { title: "Choose a torrent".into(), ..Default::default() }.filter("Torrents", &["torrent"]);
                                    tx.open_file(options, |s: &mut State, result, tx| match result {
                                        // The picked path lands in the field exactly as `torrz ui field source` would put it.
                                        Ok(Some(path)) => {
                                            run(s, Ui::Field { name: "source".into(), value: path.display().to_string() }, tx);
                                        }
                                        Ok(None) => {}
                                        Err(e) => s.say(&format!("Couldn't open the file picker: {e}"), false, tx),
                                    });
                                }),
                        ),
                )
                .child(checkbox("Add paused", *paused, ui(Ui::Field { name: "paused".into(), value: onoff(!*paused) })))
                .child(text(format!("Saves to {}", s.snap.session.download_dir)).font(t::SMALL).color(t::FAINT)),
            button("Add", ui(Ui::Confirm), t::BLUE, t::WHITE),
        ),
        Dialog::Remove { ids, delete_files } => {
            let what = match ids.as_slice() {
                [one] => format!("“{}”", s.torrent(*one).map(|x| x.name.as_str()).unwrap_or("this torrent")),
                many => format!("{} torrents", many.len()),
            };
            (
                format!("Remove {what}?"),
                col()
                    .w_full()
                    .gap(12.0)
                    .child(text(if *delete_files { "The downloaded files will be deleted from disk. This can't be undone." } else { "The downloaded files stay where they are." }).font(t::BODY).color(if *delete_files { t::RED } else { t::MUTED }).wrap())
                    .child(checkbox("Also delete the downloaded files", *delete_files, ui(Ui::Field { name: "delete-files".into(), value: onoff(!*delete_files) }))),
                button(if *delete_files { "Remove and Delete" } else { "Remove" }, ui(Ui::Confirm), t::RED, t::WHITE),
            )
        }
        Dialog::Limits { down, up } => {
            let labelled = |label: &str, f: El| row().w_full().gap(10.0).center_y().child(text(label).font(t::BODY).color(t::TEXT_2).w(74.0)).child(f.grow(1.0).min_w(0.0));
            (
                if s.snap.session.alt_limits { "Alternate Speed Limits".into() } else { "Speed Limits".into() },
                col()
                    .w_full()
                    .gap(10.0)
                    .child(text("Per second: 12M, 800k. Leave a box empty for no limit.").font(t::BODY).color(t::MUTED))
                    .child(labelled("Download", field("dialog-down", down, "Unlimited", "Download limit", |s| match &mut s.dialog {
                        Dialog::Limits { down, .. } => down,
                        _ => &mut s.scratch,
                    })))
                    .child(labelled("Upload", field("dialog-up", up, "Unlimited", "Upload limit", |s| match &mut s.dialog {
                        Dialog::Limits { up, .. } => up,
                        _ => &mut s.scratch,
                    }))),
                button("Apply", ui(Ui::Confirm), t::BLUE, t::WHITE),
            )
        }
    };
    let sheet = col()
        .key("dialog")
        .w(460.0)
        .padding(20.0)
        .gap(14.0)
        .radius(12.0)
        .fill(t::SHEET)
        .border(1.0, t::WHITE.alpha(0.1))
        .shadow(18.0, 50.0, rune::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.55 })
        .role("dialog")
        .label(title.clone())
        .modal(true, |s: &mut State| s.dialog = Dialog::None)
        .child(text(title).font(t::HEAD).color(t::TEXT).wrap())
        .child(body)
        .child(row().w_full().pt(4.0).gap(8.0).justify_end().child(cancel()).child(confirm));
    scrim("dialog-scrim", 0.42).child(col().w_full().h_full().center().child(sheet))
}

pub fn toast(toast: &Toast) -> El {
    el().key("toast").pin(Some(0.0), None, Some(0.0), Some(40.0)).items_center().justify_center().inert().child(
        row()
            .h(32.0)
            .px(14.0)
            .gap(8.0)
            .radius(16.0)
            .center_y()
            .max_w(640.0)
            .fill(t::SHEET)
            .border(1.0, t::WHITE.alpha(0.12))
            .shadow(8.0, 24.0, rune::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.45 })
            .child(el().size(7.0, 7.0).radius(3.5).fill(if toast.ok { t::GREEN } else { t::RED }))
            .child(text(toast.text.clone()).font(t::BODY).color(t::TEXT).truncate()),
    )
}
