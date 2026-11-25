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
    Show {
        /// An id, an info hash or `selected`.
        target: String,
    },
    /// Session totals: speeds, limits, DHT, port, free space.
    Stats,
    /// Add a magnet link, a URL or a .torrent file.
    Add {
        /// A magnet link, an http(s) URL, or the path of a .torrent file.
        source: String,
        /// Add it without starting it.
        #[arg(long)]
        paused: bool,
        /// Where to save it (default: the download folder).
        #[arg(long)]
        dir: Option<String>,
        /// The category to file it under (shown in the sidebar).
        #[arg(long)]
        category: Option<String>,
        /// Move the .torrent file to the Trash once it is added.
        #[arg(long)]
        trash: bool,
    },
    /// Pause torrents: ids, info hashes, `all` or `selected`.
    Pause {
        /// Ids, info hashes, `all` or `selected`.
        #[arg(required = true)]
        targets: Vec<String>,
    },
    /// Resume torrents.
    #[command(alias = "start")]
    Resume {
        /// Ids, info hashes, `all` or `selected`.
        #[arg(required = true)]
        targets: Vec<String>,
    },
    /// Remove torrents, keeping their files unless told otherwise.
    #[command(alias = "remove")]
    Rm {
        /// Ids, info hashes, `all` or `selected`.
        #[arg(required = true)]
        targets: Vec<String>,
        /// Also delete the downloaded files from disk. This cannot be undone.
        #[arg(long)]
        delete_files: bool,
    },
    /// Choose which files of a torrent to download.
    Files {
        /// An id, an info hash or `selected`. `torrz show` lists the file indices.
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
        /// Download limit a second, such as `12M` or `800k`; `none` removes it.
        #[arg(long)]
        down: Option<String>,
        /// Upload limit a second; `none` removes it.
        #[arg(long)]
        up: Option<String>,
    },
    /// Switch the alternate (slow) limits; with no argument, toggle them.
    AltLimits {
        /// Leave out to toggle.
        #[arg(value_enum)]
        state: Option<OnOff>,
    },
    /// Put torrents in a category, or take them out with --clear.
    Category {
        /// Ids, info hashes, `all` or `selected`.
        #[arg(required = true)]
        targets: Vec<String>,
        /// The category to put them in.
        #[arg(long, conflicts_with = "clear", required_unless_present = "clear")]
        name: Option<String>,
        /// Take them out of their category instead.
        #[arg(long)]
        clear: bool,
    },
    /// Show a torrent's files in the file manager.
    Reveal {
        /// An id, an info hash or `selected`.
        target: String,
    },
    /// Watch a folder and add every .torrent file that appears in it. Alone, says what is watched.
    Watch {
        /// The folder to watch.
        #[arg(conflicts_with = "off")]
        dir: Option<String>,
        /// Stop watching.
        #[arg(long)]
        off: bool,
        /// Move each .torrent file to the Trash once it is added (`on`), or leave it (`off`).
        #[arg(long, value_enum)]
        trash: Option<OnOff>,
    },
    /// Public trackers from newTrackon, ranked by how fast they answer from here.
    Trackers {
        #[command(subcommand)]
        sub: TrackersSub,
    },
    /// Move the window: what it shows, selects and has open.
    Ui {
        #[command(subcommand)]
        ui: UiSub,
    },
}

#[derive(Subcommand, Debug)]
pub enum TrackersSub {
    /// The best public trackers, fastest first (kept for a few hours).
    Best {
        /// Fetch and probe them again now.
        #[arg(long)]
        refresh: bool,
    },
    /// Give torrents more trackers: the best public ones, or the --url ones. Skips private torrents.
    Add {
        /// Ids, info hashes, `all` or `selected`.
        #[arg(required = true)]
        targets: Vec<String>,
        /// A tracker to add instead of the best ones; repeat for several.
        #[arg(long = "url")]
        urls: Vec<String>,
    },
    /// Give every new torrent the best public trackers as it is added; with no argument, toggle.
    Auto(Toggle),
}

#[derive(Args, Debug)]
pub struct Toggle {
    /// Leave out to toggle.
    #[arg(value_enum)]
    state: Option<OnOff>,
}

#[derive(Subcommand, Debug)]
pub enum UiSub {
    /// Everything the window shows, as JSON.
    State,
    /// all, downloading, seeding, completed, paused, errors, category:NAME or tracker:HOST.
    Filter {
        /// The group to show, as the sidebar names them.
        name: String,
    },
    /// Select torrents (none clears the selection).
    Select {
        /// Ids, info hashes or `all`. The last one is the torrent the inspector shows.
        targets: Vec<String>,
    },
    /// general, files, peers, trackers, pieces or log.
    Tab {
        /// The inspector tab to show.
        name: String,
    },
    /// name, size, progress, status, down, up, eta, ratio, peers or added.
    Sort {
        /// The column to sort by; `added` restores the order torrents were added in.
        column: String,
        /// Largest first. With neither flag, sorting a column again reverses it.
        #[arg(long, conflicts_with = "asc")]
        desc: bool,
        /// Smallest first.
        #[arg(long)]
        asc: bool,
    },
    /// Filter the list by name (an empty string clears it).
    Search {
        /// Words that must all appear in a torrent's name.
        query: String,
    },
    /// Open, close or toggle the command palette, and set what is typed in it.
    Palette {
        /// Leave out to toggle.
        #[arg(value_enum)]
        state: Option<OnOff>,
        /// What to type into it (opens it if it is closed).
        #[arg(long)]
        query: Option<String>,
    },
    /// Move the palette's highlight by this many rows.
    PaletteMove {
        /// Rows to move: positive is down, negative is up, and it wraps.
        #[arg(allow_hyphen_values = true)]
        delta: i32,
    },
    /// Run the highlighted palette row.
    PaletteRun,
    /// add, remove, limits, watch or none.
    Dialog {
        /// The dialog to open; `none` closes whichever is open.
        name: String,
    },
    /// Set a field of the open dialog: source, paused, trash, delete-files, down, up or dir.
    Field {
        /// The field's name.
        name: String,
        /// Text for source, down, up and dir; `on` or `off` for paused, trash and delete-files.
        value: String,
    },
    /// Press the open dialog's main button.
    Confirm,
    /// Close the open dialog or palette.
    Cancel,
    /// Show, hide or toggle the sidebar.
    Sidebar(Toggle),
    /// Show, hide or toggle the inspector under the table.
    Inspector(Toggle),
    /// show, hide, minimize or quit.
    Window {
        /// Hiding closes the window and keeps the torrents running; quit stops them.
        action: String,
    },
    /// The app's icon: auto (follows the system), light or dark.
    Icon {
        /// Which version of the icon the Dock shows while Torrz runs.
        style: String,
    },
    /// Make Torrz the app that opens magnet links (macOS asks you to confirm).
    DefaultApp,
    /// Copy a torrent's name, hash or magnet link to the clipboard (and print it).
    Copy {
        /// name, hash or magnet.
        what: String,
        /// An id, an info hash or `selected`.
        target: String,
    },
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
            Sub::Add { source, paused, dir, category, trash } => Command::Add { source, paused, dir, category, trash },
            Sub::Pause { targets } => Command::Pause { targets },
            Sub::Resume { targets } => Command::Resume { targets },
            Sub::Rm { targets, delete_files } => Command::Remove { targets, delete_files },
            Sub::Files { target, include, skip } => Command::Files { target, include, skip },
            Sub::Limit { down, up } => Command::Limit { down, up },
            Sub::AltLimits { state } => Command::AltLimits { on: OnOff::get(state) },
            Sub::Category { targets, name, .. } => Command::Category { targets, name },
            Sub::Reveal { target } => Command::Reveal { target },
            Sub::Watch { dir, off, trash } => Command::Watch { dir, off, trash: OnOff::get(trash) },
            Sub::Trackers { sub } => match sub {
                TrackersSub::Best { refresh } => Command::BestTrackers { refresh },
                TrackersSub::Add { targets, urls } => Command::AddTrackers { targets, urls },
                TrackersSub::Auto(t) => Command::AutoTrackers { on: OnOff::get(t.state) },
            },
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
                UiSub::Icon { style } => Ui::Icon { style },
                UiSub::DefaultApp => Ui::DefaultApp,
                UiSub::Copy { what, target } => Ui::Copy { what, target },
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
            Command::Add { source: "magnet:?xt=urn:btih:abc&dn=it's here".into(), paused: true, dir: Some("/tmp/a b".into()), category: Some("Linux ISOs".into()), trash: false },
            Command::Add { source: "/tmp/x.torrent".into(), paused: false, dir: None, category: None, trash: true },
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
            Command::Watch { dir: None, off: false, trash: None },
            Command::Watch { dir: Some("/tmp/a b".into()), off: false, trash: Some(true) },
            Command::Watch { dir: None, off: true, trash: None },
            Command::Watch { dir: None, off: false, trash: Some(false) },
            Command::BestTrackers { refresh: false },
            Command::BestTrackers { refresh: true },
            Command::AddTrackers { targets: t(&["1", "3"]), urls: vec![] },
            Command::AddTrackers { targets: t(&["selected"]), urls: t(&["udp://a.example:80/announce", "https://b.example/announce?x=1&y=2"]) },
            Command::AutoTrackers { on: None },
            Command::AutoTrackers { on: Some(false) },
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
            Ui::Icon { style: "dark".into() },
            Ui::DefaultApp,
            Ui::Copy { what: "magnet".into(), target: "3".into() },
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
