//! Torrz, the window. `torrz-app` opens it on the real engine; `--demo` uses a simulated swarm.
//! The `torrz` command line reaches it over the socket set up here.

mod handler;
mod icon;
mod state;
mod theme;
mod view;

#[cfg(test)]
mod tests;

use std::sync::{Arc, OnceLock, mpsc};
use std::time::Duration;

use rune::{App, AppHandle, AppSpec, Titlebar};
use torrz_core::engine::Engine;
use torrz_core::engine::demo::DemoEngine;
use torrz_core::engine::rqbit::{RqbitEngine, RqbitOptions};
use torrz_core::{Backend, Command, Response, Ui, backend, ipc};

use state::{Exec, Job, Origin, Shared, State, dispatch};

/// The running window's handle, for answers that arrive from outside a handler (macOS
/// confirming the magnet-link handler). Unset in headless runs and tests.
pub static HANDLE: OnceLock<AppHandle<State>> = OnceLock::new();

/// How often the watched folder is looked at.
const WATCH_EVERY: Duration = Duration::from_secs(2);

pub fn spec() -> AppSpec<State> {
    AppSpec {
        min_size: Some((900.0, 560.0)),
        on_key: Some(view::on_key),
        poll: Some(view::poll),
        // Named states for headless snapshots (`torrz-app --snap out.png --scene palette`).
        scenes: &[
            ("selected", |s| scene_select(s)),
            ("palette", |s| {
                scene_select(s);
                s.palette = Some(state::Palette { query: rune::TextEdit::new("pause"), index: 0 });
            }),
            ("add", |s| s.dialog = state::Dialog::Add { source: rune::TextEdit::new(""), paused: false, trash: false }),
            ("watch", |s| s.dialog = state::Dialog::Watch { dir: rune::TextEdit::new("~/Downloads"), trash: true }),
            ("files", |s| {
                scene_select(s);
                s.tab = state::Tab::Files;
            }),
            ("peers", |s| {
                scene_select(s);
                s.tab = state::Tab::Peers;
            }),
            ("trackers", |s| {
                scene_select(s);
                s.tab = state::Tab::Trackers;
            }),
            ("pieces", |s| {
                scene_select(s);
                s.tab = state::Tab::Pieces;
            }),
        ],
        // Scenes need an init fn; the real state always comes from `App::init`.
        ..AppSpec::new("Torrz", (1180.0, 740.0), || State::new(Arc::new(Backend::new(Arc::new(DemoEngine::frozen()), None)), Exec::Inline, Arc::default(), true), view::view)
    }
}

fn scene_select(s: &mut State) {
    s.selected = vec![1];
    s.refresh();
}

/// Runs an engine command off the UI thread, then hands the window its result and a fresh
/// snapshot. Both the engine thread (clicks) and the socket threads (the CLI) come through here.
fn run_engine(backend: &Backend, shared: &Shared, handle: &AppHandle<State>, cmd: Command, selected: Vec<u64>, origin: Origin) -> Response {
    let resp = backend.exec(&cmd, &selected);
    let detail = shared.selected.lock().unwrap_or_else(|e| e.into_inner()).last().copied();
    let snap = backend.snapshot(detail);
    let (c, r) = (cmd, resp.clone());
    handle.update(move |s: &mut State, tx| {
        s.finish(&c, &r, origin, tx);
        s.set_snapshot(snap);
    });
    resp
}

fn refresh(backend: &Backend, shared: &Shared, handle: &AppHandle<State>) {
    let detail = shared.selected.lock().unwrap_or_else(|e| e.into_inner()).last().copied();
    let snap = backend.snapshot(detail);
    handle.update(move |s: &mut State, _| s.set_snapshot(snap));
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let demo = args.iter().any(|a| a == "--demo");
    // Rune's own flags (--snap, --tree, --script …) render headless: no socket, no engine thread.
    let headless = args.iter().any(|a| matches!(a.as_str(), "--snap" | "--tree" | "--script" | "--video" | "--out" | "--trace" | "--list-scenes" | "--html" | "--strip"));
    let rune_args: Vec<String> = args.iter().filter(|a| *a != "--demo").cloned().collect();

    let dir = backend::app_dir();
    let engine: Arc<dyn Engine> = if headless {
        Arc::new(DemoEngine::frozen())
    } else if demo {
        Arc::new(DemoEngine::new())
    } else {
        match RqbitEngine::start(RqbitOptions { download_dir: backend::default_download_dir(), state_dir: dir.clone(), listen_port: 51413, dht: true }) {
            Ok(e) => Arc::new(e),
            Err(e) => {
                eprintln!("torrz: {e:#}");
                std::process::exit(1);
            }
        }
    };
    // The demo keeps its settings in memory, so trying it never touches the real config.
    let backend = Arc::new(Backend::new(engine, (!demo && !headless).then(|| dir.join("config.json"))));
    let shared = Arc::new(Shared::default());

    if headless {
        let (b, sh) = (backend.clone(), shared.clone());
        App::new(spec()).init(move || State::new(b, Exec::Inline, sh, true)).titlebar(Titlebar::new(view::TITLEBAR)).args(rune_args).run();
        return;
    }

    let sock = ipc::socket_path();
    if ipc::is_running(&sock) {
        // One Torrz at a time: bring the running one forward instead.
        let _ = ipc::send(&sock, &Command::Ui(Ui::Window { action: "show".into() }));
        return;
    }

    let (jobs_tx, jobs_rx) = mpsc::channel::<Job>();
    let (b, sh) = (backend.clone(), shared.clone());
    let app = App::new(spec())
        .init(move || State::new(b, Exec::Worker(jobs_tx), sh, demo))
        .titlebar(Titlebar::new(view::TITLEBAR))
        .args(rune_args)
        // Closing the window keeps the torrents running; Quit ends them.
        .keep_running()
        // A magnet link opened from a browser or Finder: the add dialog, filled in, in front.
        .on_open_url(|s: &mut State, url: &str, tx| {
            let url = url.trim();
            if !url.get(..7).is_some_and(|p| p.eq_ignore_ascii_case("magnet:")) {
                return;
            }
            // Closing the window keeps Torrz running, so the window may need bringing back.
            if let Some(h) = HANDLE.get() {
                h.open();
            }
            dispatch(s, Command::Ui(Ui::Dialog { name: "add".into() }), Origin::Ui, tx);
            dispatch(s, Command::Ui(Ui::Field { name: "source".into(), value: url.to_owned() }), Origin::Ui, tx);
        });
    let handle = app.handle();
    let _ = HANDLE.set(handle.clone());

    // The engine thread: commands from clicks, and a snapshot every second.
    {
        let (backend, shared, handle) = (backend.clone(), shared.clone(), handle.clone());
        std::thread::Builder::new()
            .name("torrz-engine-jobs".into())
            .spawn(move || {
                loop {
                    match jobs_rx.recv_timeout(Duration::from_millis(1000)) {
                        Ok(Job::Run { cmd, selected, origin }) => {
                            run_engine(&backend, &shared, &handle, cmd, selected, origin);
                        }
                        Ok(Job::Refresh) | Err(mpsc::RecvTimeoutError::Timeout) => refresh(&backend, &shared, &handle),
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })
            .expect("spawn the engine thread");
    }

    // The watched folder: each .torrent file that arrives is added as `torrz add` would add it.
    {
        let (backend, shared, handle) = (backend.clone(), shared.clone(), handle.clone());
        std::thread::Builder::new()
            .name("torrz-watch".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(WATCH_EVERY);
                    let (files, trash) = backend.take_watched();
                    for f in files {
                        let cmd = Command::Add { source: f.display().to_string(), paused: false, dir: None, category: None, trash };
                        let selected = shared.selected.lock().unwrap_or_else(|e| e.into_inner()).clone();
                        run_engine(&backend, &shared, &handle, cmd, selected, Origin::Watch);
                    }
                }
            })
            .expect("spawn the watch thread");
    }

    // The socket: window commands run on the UI thread and answer back; engine commands run here.
    {
        let (backend, shared, handle) = (backend.clone(), shared.clone(), handle.clone());
        let served = ipc::serve(&sock, move |cmd| match cmd {
            Command::Ui(u) => {
                if matches!(&u, Ui::Window { action } if action.eq_ignore_ascii_case("show")) {
                    handle.open();
                }
                let (tx, rx) = mpsc::channel();
                handle.update(move |s: &mut State, t| {
                    let _ = tx.send(dispatch(s, Command::Ui(u), Origin::Cli, t));
                });
                rx.recv_timeout(Duration::from_secs(5)).unwrap_or_else(|_| Response::err("the window did not answer"))
            }
            cmd => {
                let selected = shared.selected.lock().unwrap_or_else(|e| e.into_inner()).clone();
                run_engine(&backend, &shared, &handle, cmd, selected, Origin::Cli)
            }
        });
        if let Err(e) = served {
            eprintln!("torrz: the command line can't reach this window: {e:#}");
        }
    }

    app.run();
    let _ = std::fs::remove_file(&sock);
}
