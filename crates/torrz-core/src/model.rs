//! What the engine reports: plain data the UI draws and the CLI prints.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TorrentState {
    Downloading,
    Seeding,
    Paused,
    Checking,
    Queued,
    Completed,
    Error,
}

impl TorrentState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Downloading => "Downloading",
            Self::Seeding => "Seeding",
            Self::Paused => "Paused",
            Self::Checking => "Checking",
            Self::Queued => "Queued",
            Self::Completed => "Completed",
            Self::Error => "Error",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Torrent {
    pub id: u64,
    pub hash: String,
    pub name: String,
    pub state: TorrentState,
    pub error: Option<String>,
    pub size: u64,
    pub done: u64,
    pub uploaded: u64,
    /// Bytes a second.
    pub down: u64,
    pub up: u64,
    pub eta_secs: Option<u64>,
    pub peers_live: u32,
    pub peers_seen: u32,
    pub pieces: u32,
    pub save_path: String,
    pub category: Option<String>,
    /// Tracker hosts.
    pub trackers: Vec<String>,
    /// A private torrent talks only to its own tracker, so Torrz never adds public ones.
    #[serde(default)]
    pub private: bool,
}

impl Torrent {
    /// 0.0 to 1.0.
    pub fn progress(&self) -> f32 {
        if self.size == 0 { 0.0 } else { (self.done as f64 / self.size as f64).min(1.0) as f32 }
    }

    pub fn ratio(&self) -> f32 {
        if self.done == 0 { 0.0 } else { (self.uploaded as f64 / self.done as f64) as f32 }
    }

    pub fn finished(&self) -> bool {
        self.size > 0 && self.done >= self.size
    }

    /// A magnet link for it: its info hash and name (peers come from the DHT).
    pub fn magnet(&self) -> String {
        let name: String = url::form_urlencoded::byte_serialize(self.name.as_bytes()).collect();
        format!("magnet:?xt=urn:btih:{}&dn={name}", self.hash)
    }
}

/// A public tracker and how long it took to answer from here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankedTracker {
    pub url: String,
    pub ms: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileEntry {
    pub index: usize,
    pub path: String,
    pub size: u64,
    pub done: u64,
    pub included: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Peer {
    pub addr: String,
    pub client: String,
    pub state: String,
    pub downloaded: u64,
    pub uploaded: u64,
}

/// The selected torrent's parts, fetched only for the torrent the inspector shows.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Detail {
    pub id: u64,
    pub files: Vec<FileEntry>,
    pub peers: Vec<Peer>,
    /// Piece availability squeezed into at most `PIECE_BUCKETS` cells, each 0 (none) to 255 (all).
    pub piece_map: Vec<u8>,
    pub pieces_have: u32,
}

pub const PIECE_BUCKETS: usize = 600;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SessionStats {
    pub down: u64,
    pub up: u64,
    pub dht_nodes: u32,
    pub port: Option<u16>,
    /// Bytes a second; `None` is unlimited.
    pub limit_down: Option<u64>,
    pub limit_up: Option<u64>,
    pub alt_limits: bool,
    pub free_disk: Option<u64>,
    pub download_dir: String,
    /// The folder watched for .torrent files, if any.
    #[serde(default)]
    pub watch_dir: Option<String>,
    /// Whether a watched .torrent file goes to the Trash once it is added.
    #[serde(default)]
    pub watch_trash: bool,
    /// Whether new torrents get the best public trackers as they are added.
    #[serde(default)]
    pub auto_trackers: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub torrents: Vec<Torrent>,
    pub session: SessionStats,
    pub detail: Option<Detail>,
}

/// Squeezes a have-bitfield of `total` pieces into at most [`PIECE_BUCKETS`] cells.
pub fn bucket_pieces(have: impl Fn(usize) -> bool, total: usize) -> Vec<u8> {
    if total == 0 {
        return Vec::new();
    }
    let n = total.min(PIECE_BUCKETS);
    (0..n)
        .map(|b| {
            let (lo, hi) = (b * total / n, ((b + 1) * total / n).max(b * total / n + 1));
            let got = (lo..hi).filter(|&i| have(i)).count();
            (got * 255 / (hi - lo)) as u8
        })
        .collect()
}

pub fn size(b: u64) -> String {
    const U: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1000.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    match i {
        0 => format!("{b} B"),
        _ if v >= 100.0 => format!("{v:.0} {}", U[i]),
        _ => format!("{v:.1} {}", U[i]),
    }
}

pub fn speed(bps: u64) -> String {
    if bps == 0 { "—".into() } else { format!("{}/s", size(bps)) }
}

pub fn eta(secs: Option<u64>) -> String {
    match secs {
        None => "—".into(),
        Some(s) if s >= 86_400 => format!("{}d {}h", s / 86_400, s % 86_400 / 3600),
        Some(s) if s >= 3600 => format!("{}h {:02}m", s / 3600, s % 3600 / 60),
        Some(s) if s >= 60 => format!("{}m {:02}s", s / 60, s % 60),
        Some(s) => format!("{s}s"),
    }
}

/// `12M`, `800k`, `1.5m`, `0`/`none`/`off` (unlimited) to bytes a second.
pub fn parse_rate(s: &str) -> anyhow::Result<Option<u64>> {
    let t = s.trim().to_ascii_lowercase();
    if matches!(t.as_str(), "0" | "none" | "off" | "unlimited") {
        return Ok(None);
    }
    let t = t.trim_end_matches("/s").trim_end_matches('b');
    let (num, mul) = match t.chars().last() {
        Some('k') => (&t[..t.len() - 1], 1024.0),
        Some('m') => (&t[..t.len() - 1], 1024.0 * 1024.0),
        Some('g') => (&t[..t.len() - 1], 1024.0 * 1024.0 * 1024.0),
        _ => (t, 1.0),
    };
    let v: f64 = num.trim().parse().map_err(|_| anyhow::anyhow!("not a rate: {s:?} (try 12M, 800k or none)"))?;
    anyhow::ensure!(v > 0.0, "a rate must be above zero (use none for unlimited)");
    Ok(Some((v * mul) as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_parse() {
        assert_eq!(parse_rate("12M").unwrap(), Some(12 * 1024 * 1024));
        assert_eq!(parse_rate("800k").unwrap(), Some(800 * 1024));
        assert_eq!(parse_rate("none").unwrap(), None);
        assert!(parse_rate("fast").is_err());
    }

    #[test]
    fn sizes_read_well() {
        assert_eq!(size(312 * 1024 * 1024), "312 MB");
        assert_eq!(size(6_549_825_126), "6.1 GB");
        assert_eq!(eta(Some(121)), "2m 01s");
    }

    #[test]
    fn pieces_bucket() {
        let m = bucket_pieces(|i| i < 1200, 2400);
        assert_eq!(m.len(), PIECE_BUCKETS);
        assert_eq!((m[0], m[PIECE_BUCKETS - 1]), (255, 0));
    }
}
