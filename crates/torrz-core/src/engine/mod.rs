//! The torrent engine behind the app. [`rqbit::RqbitEngine`] is the real one; [`demo::DemoEngine`]
//! is a simulated swarm for tests, screenshots and trying the UI without a network.

pub mod demo;
pub mod rqbit;

use crate::model::Snapshot;

pub trait Engine: Send + Sync + 'static {
    /// Every torrent and the session's totals; `detail` asks for one torrent's files, peers and
    /// pieces as well. Cheap enough to call once a second.
    fn snapshot(&self, detail: Option<u64>) -> Snapshot;
    /// A magnet link, an http(s) URL or a path to a `.torrent` file. Returns the torrent's id.
    fn add(&self, source: &str, paused: bool, dir: Option<&str>) -> anyhow::Result<u64>;
    fn pause(&self, id: u64) -> anyhow::Result<()>;
    fn resume(&self, id: u64) -> anyhow::Result<()>;
    fn remove(&self, id: u64, delete_files: bool) -> anyhow::Result<()>;
    /// The file indices to download; the rest are skipped.
    fn set_files(&self, id: u64, only: &[usize]) -> anyhow::Result<()>;
    /// Bytes a second; `None` is unlimited.
    fn set_limits(&self, down: Option<u64>, up: Option<u64>);
}
