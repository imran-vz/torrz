//! The `torrz` command line, parsed into the same [`Command`]s the window runs. It lives here,
//! not in the binary, so the app can prove that every control it draws has a line that parses.

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::command::{Command, Ui};

#[derive(Parser, Debug)]
#[command(name = "torrz", version, about = "Drive Torrz from the terminal. Every control in the window has a command here.")]
pub struct Cli {
    /// Print the raw JSON answer.
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub sub: Sub,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum OnOff {
    On,
    Off,
}

impl OnOff {
    fn get(v: Option<OnOff>) -> Option<bool> {
        v.map(|v| v == OnOff::On)
    }
}

#[derive(Subcommand, Debug)]
pub enum Sub {
    /// Start the app (or bring its window forward if it is running).
    Open {
        /// A simulated swarm instead of the network: nothing is downloaded.
        #[arg(long)]
        demo: bool,
    },
    /// Every torrent.
    #[command(alias = "ls")]
    List,
    /// One torrent with its files, peers and pieces.
    Show { target: String },
    /// Session totals: speeds, limits, DHT, port, free space.
    Stats,
    /// Add a magnet link, a URL or a .torrent file.
    Add {
        source: String,
        #[arg(long)]
        paused: bool,
        /// Where to save it (default: the download folder).
        #[arg(long)]
        dir: Option<String>,
        #[arg(long)]
        category: Option<String>,
    },
    /// Pause torrents: ids, info hashes, `all` or `selected`.
    Pause {
        #[arg(required = true)]
        targets: Vec<String>,
    },
    /// Resume torrents.
    #[command(alias = "start")]
    Resume {
        #[arg(required = true)]
        targets: Vec<String>,
    },
    /// Remove torrents, keeping their files unless told otherwise.
    #[command(alias = "remove")]
    Rm {
        #[arg(required = true)]
        targets: Vec<String>,
        #[arg(long)]
        delete_files: bool,
    },
    /// Choose which files of a torrent to download.
    Files {
        target: String,
        /// File indices to download, comma separated.
        #[arg(long, value_delimiter = ',')]
        include: Vec<usize>,
        /// File indices to skip, comma separated.
        #[arg(long, value_delimiter = ',')]
        skip: Vec<usize>,
    },
    /// Speed limits: `12M`, `800k` or `none`.
    Limit {
        #[arg(long)]
        down: Option<String>,
        #[arg(long)]
        up: Option<String>,
    },
    /// Switch the alternate (slow) limits; with no argument, toggle them.
    AltLimits {
        #[arg(value_enum)]
        state: Option<OnOff>,
    },
    /// Put torrents in a category, or take them out with --clear.
    Category {
        #[arg(required = true)]
        targets: Vec<String>,
        #[arg(long, conflicts_with = "clear", required_unless_present = "clear")]
        name: Option<String>,
        #[arg(long)]
        clear: bool,
    },
    /// Show a torrent's files in the file manager.
    Reveal { target: String },
    /// Move the window: what it shows, selects and has open.
    Ui {
        #[command(subcommand)]
        ui: UiSub,
    },
}

#[derive(Args, Debug)]
pub struct Toggle {
    #[arg(value_enum)]
    state: Option<OnOff>,
}

#[derive(Subcommand, Debug)]
pub enum UiSub {
    /// Everything the window shows, as JSON.
    State,
    /// all, downloading, seeding, completed, paused, errors, category:NAME or tracker:HOST.
    Filter { name: String },
    /// Select torrents (none clears the selection).
    Select { targets: Vec<String> },
    /// general, files, peers, trackers, pieces or log.
    Tab { name: String },
    /// name, size, progress, status, down, up, eta, ratio, peers or added.
    Sort {
        column: String,
        #[arg(long, conflicts_with = "asc")]
        desc: bool,
        #[arg(long)]
        asc: bool,
    },
    /// Filter the list by name (an empty string clears it).
    Search { query: String },
    /// Open, close or toggle the command palette, and set what is typed in it.
    Palette {
        #[arg(value_enum)]
        state: Option<OnOff>,
        #[arg(long)]
        query: Option<String>,
    },
    /// Move the palette's highlight by this many rows.
    PaletteMove {
        #[arg(allow_hyphen_values = true)]
        delta: i32,
    },
    /// Run the highlighted palette row.
    PaletteRun,
    /// add, remove, limits or none.
    Dialog { name: String },
    /// Set a field of the open dialog: source, paused, delete-files, down or up.
    Field { name: String, value: String },
    /// Press the open dialog's main button.
    Confirm,
    /// Close the open dialog or palette.
    Cancel,
    Sidebar(Toggle),
    Inspector(Toggle),
    /// show, hide, minimize or quit.
    Window { action: String },
}

/// What a command line asks for.
#[derive(Debug, PartialEq)]
pub enum Parsed {
    Open { demo: bool },
    Run(Command),
}

impl Sub {
    pub fn into_parsed(self) -> Parsed {
        Parsed::Run(match self {
            Sub::Open { demo } => return Parsed::Open { demo },
            Sub::List => Command::List,
            Sub::Show { target } => Command::Show { target },
            Sub::Stats => Command::Stats,
            Sub::Add { source, paused, dir, category } => Command::Add { source, paused, dir, category },
            Sub::Pause { targets } => Command::Pause { targets },
            Sub::Resume { targets } => Command::Resume { targets },
            Sub::Rm { targets, delete_files } => Command::Remove { targets, delete_files },
            Sub::Files { target, include, skip } => Command::Files { target, include, skip },
            Sub::Limit { down, up } => Command::Limit { down, up },
            Sub::AltLimits { state } => Command::AltLimits { on: OnOff::get(state) },
            Sub::Category { targets, name, .. } => Command::Category { targets, name },
            Sub::Reveal { target } => Command::Reveal { target },
            Sub::Ui { ui } => Command::Ui(match ui {
                UiSub::State => Ui::State,
                UiSub::Filter { name } => Ui::Filter { name },
                UiSub::Select { targets } => Ui::Select { targets },
                UiSub::Tab { name } => Ui::Tab { name },
                UiSub::Sort { column, desc, asc } => Ui::Sort { column, desc: if desc { Some(true) } else if asc { Some(false) } else { None } },
                UiSub::Search { query } => Ui::Search { query },
                UiSub::Palette { state, query } => Ui::Palette { open: OnOff::get(state), query },
                UiSub::PaletteMove { delta } => Ui::PaletteMove { delta },
                UiSub::PaletteRun => Ui::PaletteRun,
                UiSub::Dialog { name } => Ui::Dialog { name },
                UiSub::Field { name, value } => Ui::Field { name, value },
                UiSub::Confirm => Ui::Confirm,
                UiSub::Cancel => Ui::Cancel,
                UiSub::Sidebar(t) => Ui::Sidebar { open: OnOff::get(t.state) },
                UiSub::Inspector(t) => Ui::Inspector { open: OnOff::get(t.state) },
                UiSub::Window { action } => Ui::Window { action },
            }),
        })
    }
}

/// Splits a shell line the way [`Command::to_cli`] quotes it: spaces separate, single quotes group.
pub fn split(line: &str) -> Vec<String> {
    let (mut out, mut cur, mut quoted, mut any) = (Vec::new(), String::new(), false, false);
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                quoted = !quoted;
                any = true;
            }
            '\\' if !quoted && chars.peek() == Some(&'\'') => {
                cur.push('\'');
                chars.next();
            }
            c if c.is_whitespace() && !quoted => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Parses a whole line such as `torrz pause 1`.
pub fn parse_line(line: &str) -> Result<Parsed, String> {
    Cli::try_parse_from(split(line)).map(|c| c.sub.into_parsed()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(c: Command) {
        let line = c.to_cli();
        assert_eq!(parse_line(&line), Ok(Parsed::Run(c)), "{line}");
    }

    #[test]
    fn every_command_parses_back_from_its_own_line() {
        let t = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        for c in [
            Command::List,
            Command::Stats,
            Command::Show { target: "3".into() },
            Command::Add { source: "magnet:?xt=urn:btih:abc&dn=it's here".into(), paused: true, dir: Some("/tmp/a b".into()), category: Some("Linux ISOs".into()) },
            Command::Add { source: "/tmp/x.torrent".into(), paused: false, dir: None, category: None },
            Command::Pause { targets: t(&["1", "2"]) },
            Command::Resume { targets: t(&["all"]) },
            Command::Remove { targets: t(&["selected"]), delete_files: true },
            Command::Remove { targets: t(&["4"]), delete_files: false },
            Command::Files { target: "1".into(), include: vec![0, 2], skip: vec![3] },
            Command::Files { target: "1".into(), include: vec![], skip: vec![1] },
            Command::Limit { down: Some("12M".into()), up: Some("none".into()) },
            Command::Limit { down: None, up: Some("800k".into()) },
            Command::AltLimits { on: None },
            Command::AltLimits { on: Some(true) },
            Command::Category { targets: t(&["1"]), name: Some("Films".into()) },
            Command::Category { targets: t(&["1"]), name: None },
            Command::Reveal { target: "2".into() },
        ] {
            round_trip(c);
        }
        for u in [
            Ui::State,
            Ui::Filter { name: "category:Linux ISOs".into() },
            Ui::Select { targets: t(&["1", "5"]) },
            Ui::Select { targets: vec![] },
            Ui::Tab { name: "peers".into() },
            Ui::Sort { column: "size".into(), desc: Some(true) },
            Ui::Sort { column: "name".into(), desc: Some(false) },
            Ui::Sort { column: "name".into(), desc: None },
            Ui::Search { query: "ubuntu iso".into() },
            Ui::Search { query: "".into() },
            Ui::Palette { open: Some(true), query: Some("pause ubu".into()) },
            Ui::Palette { open: None, query: None },
            Ui::PaletteMove { delta: -2 },
            Ui::PaletteRun,
            Ui::Dialog { name: "add".into() },
            Ui::Field { name: "source".into(), value: "magnet:?xt=urn:btih:abc".into() },
            Ui::Confirm,
            Ui::Cancel,
            Ui::Sidebar { open: Some(false) },
            Ui::Inspector { open: None },
            Ui::Window { action: "hide".into() },
        ] {
            round_trip(Command::Ui(u));
        }
    }

    #[test]
    fn open_is_the_cli_s_own() {
        assert_eq!(parse_line("torrz open --demo"), Ok(Parsed::Open { demo: true }));
        assert!(parse_line("torrz pause").is_err(), "pause needs a target");
    }
}
