//! The one command set. A click in the window, a row in the ⌘K palette and a `torrz` invocation
//! all become a [`Command`]; nothing changes the app any other way, so the three cannot drift.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A torrent target on the wire: an id (`3`), an info hash, `all` or `selected`.
pub type Target = String;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    List,
    Show { target: Target },
    Stats,
    Add {
        source: String,
        #[serde(default)]
        paused: bool,
        #[serde(default)]
        dir: Option<String>,
        #[serde(default)]
        category: Option<String>,
        /// Move the .torrent file to the Trash once it is added.
        #[serde(default)]
        trash: bool,
    },
    Pause { targets: Vec<Target> },
    Resume { targets: Vec<Target> },
    Remove {
        targets: Vec<Target>,
        #[serde(default)]
        delete_files: bool,
    },
    Files {
        target: Target,
        #[serde(default)]
        include: Vec<usize>,
        #[serde(default)]
        skip: Vec<usize>,
    },
    /// Rates as text (`12M`, `800k`, `none`); a missing side is left alone.
    Limit {
        #[serde(default)]
        down: Option<String>,
        #[serde(default)]
        up: Option<String>,
    },
    /// `None` toggles.
    AltLimits {
        #[serde(default)]
        on: Option<bool>,
    },
    Category {
        targets: Vec<Target>,
        #[serde(default)]
        name: Option<String>,
    },
    Reveal { target: Target },
    /// The folder watched for new .torrent files. With nothing set, it reports the current one.
    Watch {
        /// Start watching this folder (replacing the one watched).
        #[serde(default)]
        dir: Option<String>,
        /// Stop watching.
        #[serde(default)]
        off: bool,
        /// Whether a .torrent file goes to the Trash once it is added.
        #[serde(default)]
        trash: Option<bool>,
    },
    /// The best public trackers (newTrackon's stable list, probed from here), fastest first.
    /// `refresh` probes again instead of using the list from the last few hours.
    BestTrackers {
        #[serde(default)]
        refresh: bool,
    },
    /// Gives torrents more trackers: `urls`, or the best public ones when it is empty. Private
    /// torrents are skipped.
    AddTrackers {
        targets: Vec<Target>,
        #[serde(default)]
        urls: Vec<String>,
    },
    /// Whether new torrents get the best public trackers as they are added. `None` toggles.
    AutoTrackers {
        #[serde(default)]
        on: Option<bool>,
    },
    Ui(Ui),
}

/// Commands that move the window itself.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "ui", rename_all = "snake_case")]
pub enum Ui {
    /// Everything the window shows, as JSON.
    State,
    Filter { name: String },
    Select { targets: Vec<Target> },
    Tab { name: String },
    Sort {
        column: String,
        #[serde(default)]
        desc: Option<bool>,
    },
    Search { query: String },
    Palette {
        #[serde(default)]
        open: Option<bool>,
        #[serde(default)]
        query: Option<String>,
    },
    PaletteMove { delta: i32 },
    PaletteRun,
    /// `add`, `remove`, `limits`, `watch` or `none`.
    Dialog { name: String },
    /// Sets a field of the open dialog: `source`, `paused`, `trash`, `delete-files`, `down`,
    /// `up`, `dir`.
    Field { name: String, value: String },
    Confirm,
    Cancel,
    Sidebar {
        #[serde(default)]
        open: Option<bool>,
    },
    Inspector {
        #[serde(default)]
        open: Option<bool>,
    },
    /// `show`, `hide`, `minimize` or `quit`.
    Window { action: String },
    /// The app's icon: `auto` (follows the system's light or dark appearance), `light` or `dark`.
    Icon { style: String },
    /// Asks the system to open magnet links with Torrz (macOS asks the user to confirm).
    DefaultApp,
    /// Copies a torrent's `name`, `hash` or `magnet` link to the clipboard.
    Copy { what: String, target: Target },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub data: Value,
}

impl Response {
    pub fn ok(message: impl Into<String>) -> Self {
        Self { ok: true, message: message.into(), data: Value::Null }
    }

    pub fn data(message: impl Into<String>, data: impl Serialize) -> Self {
        Self { ok: true, message: message.into(), data: serde_json::to_value(data).unwrap_or(Value::Null) }
    }

    pub fn err(message: impl std::fmt::Display) -> Self {
        Self { ok: false, message: message.to_string(), data: Value::Null }
    }
}

fn quote(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:=+,@%".contains(c)) {
        s.to_owned()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

fn join(t: &[Target]) -> String {
    t.iter().map(|s| quote(s)).collect::<Vec<_>>().join(" ")
}

fn on_off(o: Option<bool>) -> &'static str {
    match o {
        Some(true) => " on",
        Some(false) => " off",
        None => "",
    }
}

impl Command {
    /// The shell line that does the same thing. The CLI's parser reads these back unchanged
    /// (`torrz-cli`'s round-trip test holds both to that).
    pub fn to_cli(&self) -> String {
        let body = match self {
            Command::List => "list".into(),
            Command::Show { target } => format!("show {}", quote(target)),
            Command::Stats => "stats".into(),
            Command::Add { source, paused, dir, category, trash } => {
                let mut s = format!("add {}", quote(source));
                if *paused {
                    s += " --paused";
                }
                if let Some(d) = dir {
                    s += &format!(" --dir {}", quote(d));
                }
                if let Some(c) = category {
                    s += &format!(" --category {}", quote(c));
                }
                if *trash {
                    s += " --trash";
                }
                s
            }
            Command::Pause { targets } => format!("pause {}", join(targets)),
            Command::Resume { targets } => format!("resume {}", join(targets)),
            Command::Remove { targets, delete_files } => {
                format!("rm {}{}", join(targets), if *delete_files { " --delete-files" } else { "" })
            }
            Command::Files { target, include, skip } => {
                let list = |v: &[usize]| v.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
                let mut s = format!("files {}", quote(target));
                if !include.is_empty() {
                    s += &format!(" --include {}", list(include));
                }
                if !skip.is_empty() {
                    s += &format!(" --skip {}", list(skip));
                }
                s
            }
            Command::Limit { down, up } => {
                let mut s = String::from("limit");
                if let Some(d) = down {
                    s += &format!(" --down {}", quote(d));
                }
                if let Some(u) = up {
                    s += &format!(" --up {}", quote(u));
                }
                s
            }
            Command::AltLimits { on } => format!("alt-limits{}", on_off(*on)),
            Command::Category { targets, name } => match name {
                Some(n) => format!("category {} --name {}", join(targets), quote(n)),
                None => format!("category {} --clear", join(targets)),
            },
            Command::Reveal { target } => format!("reveal {}", quote(target)),
            Command::Watch { dir, off, trash } => {
                let mut s = String::from("watch");
                if let Some(d) = dir {
                    s += &format!(" {}", quote(d));
                }
                if *off {
                    s += " --off";
                }
                if let Some(t) = trash {
                    s += &format!(" --trash{}", on_off(Some(*t)));
                }
                s
            }
            Command::BestTrackers { refresh } => format!("trackers best{}", if *refresh { " --refresh" } else { "" }),
            Command::AddTrackers { targets, urls } => {
                let mut s = format!("trackers add {}", join(targets));
                for u in urls {
                    s += &format!(" --url {}", quote(u));
                }
                s
            }
            Command::AutoTrackers { on } => format!("trackers auto{}", on_off(*on)),
            Command::Ui(ui) => format!("ui {}", ui.to_cli()),
        };
        format!("torrz {body}")
    }

    /// Whether running it changes nothing (so it is kept out of the command log).
    pub fn is_query(&self) -> bool {
        matches!(self, Command::List | Command::Show { .. } | Command::Stats | Command::Watch { dir: None, off: false, trash: None } | Command::BestTrackers { refresh: false } | Command::Ui(Ui::State))
    }
}

impl Ui {
    fn to_cli(&self) -> String {
        match self {
            Ui::State => "state".into(),
            Ui::Filter { name } => format!("filter {}", quote(name)),
            Ui::Select { targets } => format!("select {}", join(targets)).trim_end().to_owned(),
            Ui::Tab { name } => format!("tab {}", quote(name)),
            Ui::Sort { column, desc } => format!(
                "sort {}{}",
                quote(column),
                match desc {
                    Some(true) => " --desc",
                    Some(false) => " --asc",
                    None => "",
                }
            ),
            Ui::Search { query } => format!("search {}", quote(query)),
            Ui::Palette { open, query } => {
                let mut s = format!("palette{}", on_off(*open));
                if let Some(q) = query {
                    s += &format!(" --query {}", quote(q));
                }
                s
            }
            Ui::PaletteMove { delta } => format!("palette-move {delta}"),
            Ui::PaletteRun => "palette-run".into(),
            Ui::Dialog { name } => format!("dialog {}", quote(name)),
            Ui::Field { name, value } => format!("field {} {}", quote(name), quote(value)),
            Ui::Confirm => "confirm".into(),
            Ui::Cancel => "cancel".into(),
            Ui::Sidebar { open } => format!("sidebar{}", on_off(*open)),
            Ui::Inspector { open } => format!("inspector{}", on_off(*open)),
            Ui::Window { action } => format!("window {}", quote(action)),
            Ui::Icon { style } => format!("icon {}", quote(style)),
            Ui::DefaultApp => "default-app".into(),
            Ui::Copy { what, target } => format!("copy {} {}", quote(what), quote(target)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_render_as_shell_lines() {
        let c = Command::Add { source: "magnet:?xt=urn:btih:abc&dn=a b".into(), paused: true, dir: None, category: Some("linux".into()), trash: false };
        assert_eq!(c.to_cli(), "torrz add 'magnet:?xt=urn:btih:abc&dn=a b' --paused --category linux");
        assert_eq!(Command::Pause { targets: vec!["selected".into()] }.to_cli(), "torrz pause selected");
        assert_eq!(Command::Ui(Ui::Sort { column: "size".into(), desc: Some(true) }).to_cli(), "torrz ui sort size --desc");
        assert_eq!(Command::Watch { dir: Some("/tmp/in".into()), off: false, trash: Some(true) }.to_cli(), "torrz watch /tmp/in --trash on");
        assert!(Command::Watch { dir: None, off: false, trash: None }.is_query());
    }

    #[test]
    fn commands_survive_the_wire() {
        let c = Command::Ui(Ui::Palette { open: Some(true), query: Some("pause".into()) });
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(serde_json::from_str::<Command>(&json).unwrap(), c);
    }
}
