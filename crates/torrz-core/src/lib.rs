//! Everything Torrz does that isn't drawing: the command set, the engine and the socket the CLI
//! uses to reach the running app.

pub mod backend;
pub mod cli;
pub mod command;
pub mod engine;
pub mod ipc;
pub mod model;

pub use backend::Backend;
pub use command::{Command, Response, Ui};
pub use model::{Detail, FileEntry, Peer, SessionStats, Snapshot, Torrent, TorrentState};
