//! `torrz`: sends one command to the running app and prints its answer.

use std::process::ExitCode;

use clap::Parser;
use torrz_core::cli::{Cli, Parsed};
use torrz_core::model::{eta, size, speed};
use torrz_core::{Command, Detail, Response, SessionStats, Torrent, Ui, ipc};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let json = cli.json;
    match run(cli.sub.into_parsed(), json) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("torrz: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(parsed: Parsed, json: bool) -> anyhow::Result<bool> {
    let sock = ipc::socket_path();
    let cmd = match parsed {
        Parsed::Open { demo } => {
            if ipc::is_running(&sock) {
                Command::Ui(Ui::Window { action: "show".into() })
            } else {
                return open(demo, &sock);
            }
        }
        Parsed::Run(c) => c,
    };
    let resp = ipc::send(&sock, &cmd)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&resp)?);
    } else {
        print(&cmd, &resp);
    }
    Ok(resp.ok)
}

/// Starts `torrz-app` from beside this binary and waits until it answers.
fn open(demo: bool, sock: &std::path::Path) -> anyhow::Result<bool> {
    let exe = std::env::current_exe()?;
    let app = exe.with_file_name(if cfg!(windows) { "torrz-app.exe" } else { "torrz-app" });
    anyhow::ensure!(app.exists(), "cannot find the app at {}", app.display());
    let mut c = std::process::Command::new(&app);
    if demo {
        c.arg("--demo");
    }
    c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    let mut child = c.spawn()?;
    for _ in 0..100 {
        if ipc::is_running(sock) {
            println!("Torrz is running{}", if demo { " (demo swarm)" } else { "" });
            return Ok(true);
        }
        if let Some(status) = child.try_wait()? {
            anyhow::bail!("the app exited at once ({status}); run {} to see why", app.display());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    anyhow::bail!("the app started but did not answer on {} within 10 s", sock.display())
}

fn print(cmd: &Command, r: &Response) {
    if !r.ok {
        eprintln!("torrz: {}", r.message);
        return;
    }
    match cmd {
        Command::List => {
            let ts: Vec<Torrent> = serde_json::from_value(r.data.clone()).unwrap_or_default();
            if ts.is_empty() {
                println!("No torrents. Add one with `torrz add <magnet|url|file>`.");
                return;
            }
            let w = ts.iter().map(|t| t.name.chars().count()).max().unwrap_or(4).clamp(4, 48);
            println!("{:>3}  {:<w$}  {:>9}  {:>5}  {:<11}  {:>11}  {:>11}  {:>8}  {:>5}", "ID", "NAME", "SIZE", "DONE", "STATUS", "DOWN", "UP", "ETA", "RATIO");
            for t in ts {
                let name: String = if t.name.chars().count() > w { t.name.chars().take(w - 1).chain(['…']).collect() } else { t.name.clone() };
                println!(
                    "{:>3}  {:<w$}  {:>9}  {:>4.0}%  {:<11}  {:>11}  {:>11}  {:>8}  {:>5.2}",
                    t.id,
                    name,
                    size(t.size),
                    t.progress() * 100.0,
                    t.state.label(),
                    speed(t.down),
                    speed(t.up),
                    eta(t.eta_secs),
                    t.ratio()
                );
            }
        }
        Command::Show { .. } => {
            let t: Option<Torrent> = serde_json::from_value(r.data["torrent"].clone()).ok();
            let d: Option<Detail> = serde_json::from_value(r.data["detail"].clone()).ok();
            let Some(t) = t else { return println!("{}", r.data) };
            println!("#{} {}", t.id, t.name);
            println!("  {} · {:.1}% of {} · ratio {:.2}", t.state.label(), t.progress() * 100.0, size(t.size), t.ratio());
            println!("  down {} · up {} · eta {} · peers {}/{}", speed(t.down), speed(t.up), eta(t.eta_secs), t.peers_live, t.peers_seen);
            println!("  saved to {} · hash {}", t.save_path, t.hash);
            if let Some(c) = &t.category {
                println!("  category {c}");
            }
            if let Some(e) = &t.error {
                println!("  error: {e}");
            }
            if let Some(d) = d {
                println!("  pieces {}/{}", d.pieces_have, t.pieces);
                println!("  files:");
                for f in &d.files {
                    println!("    [{}] {:>2}  {:>9}  {}", if f.included { 'x' } else { ' ' }, f.index, size(f.size), f.path);
                }
                if !d.peers.is_empty() {
                    println!("  peers:");
                    for p in d.peers.iter().take(12) {
                        println!("    {:<40} {:<22} down {:>9}  up {:>9}", p.addr, p.client, size(p.downloaded), size(p.uploaded));
                    }
                }
            }
        }
        Command::Stats => {
            let Ok(s) = serde_json::from_value::<SessionStats>(r.data.clone()) else { return println!("{}", r.data) };
            let lim = |l: Option<u64>| l.map(speed).unwrap_or_else(|| "unlimited".into());
            println!("down {} (limit {}) · up {} (limit {})", speed(s.down), lim(s.limit_down), speed(s.up), lim(s.limit_up));
            println!("alternate limits {} · DHT {} nodes · port {}", if s.alt_limits { "on" } else { "off" }, s.dht_nodes, s.port.map(|p| p.to_string()).unwrap_or_else(|| "none".into()));
            println!("saving to {}{}", s.download_dir, s.free_disk.map(|f| format!(" · {} free", size(f))).unwrap_or_default());
        }
        Command::Ui(Ui::State) => println!("{}", serde_json::to_string_pretty(&r.data).unwrap_or_default()),
        Command::Reveal { .. } => println!("{}", r.data["path"].as_str().unwrap_or(&r.message)),
        Command::Ui(Ui::Copy { .. }) => println!("{}", r.data.as_str().unwrap_or(&r.message)),
        Command::BestTrackers { .. } => {
            let best: Vec<torrz_core::model::RankedTracker> = serde_json::from_value(r.data.clone()).unwrap_or_default();
            for t in best {
                println!("{:>5} ms  {}", t.ms, t.url);
            }
        }
        _ => println!("{}", if r.message.is_empty() { "ok" } else { &r.message }),
    }
}
