//! The torrent engine behind the app. [`rqbit::RqbitEngine`] is the real one; [`demo::DemoEngine`]
//! is a simulated swarm for tests, screenshots and trying the UI without a network.

pub mod demo;
pub mod rqbit;

use crate::model::Snapshot;

pub trait Engine: Send + Sync + 'static {
    /// Every torrent and the session's totals; `detail` asks for one torrent's files, peers and
    /// pieces as well. Cheap enough to call once a second.
    fn snapshot(&self, detail: Option<u64>) -> Snapshot;
    /// A magnet link, an http(s) URL or a path to a `.torrent` file, announced to `trackers` as
    /// well as its own (unless it is private). Returns the torrent's id.
    fn add(&self, source: &str, paused: bool, dir: Option<&str>, trackers: &[String]) -> anyhow::Result<u64>;
    /// Gives a torrent more trackers; refuses a private one. Returns its id afterwards (the real
    /// engine adds it again, which may give it another id) and how many trackers it gained.
    fn add_trackers(&self, id: u64, trackers: &[String]) -> anyhow::Result<(u64, usize)>;
    fn pause(&self, id: u64) -> anyhow::Result<()>;
    fn resume(&self, id: u64) -> anyhow::Result<()>;
    fn remove(&self, id: u64, delete_files: bool) -> anyhow::Result<()>;
    /// The file indices to download; the rest are skipped.
    fn set_files(&self, id: u64, only: &[usize]) -> anyhow::Result<()>;
    /// Bytes a second; `None` is unlimited.
    fn set_limits(&self, down: Option<u64>, up: Option<u64>);
    /// A simulated engine never touches the network, so it is handed made-up trackers.
    fn offline(&self) -> bool {
        false
    }
}
