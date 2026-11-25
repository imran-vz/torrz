//! A simulated swarm: twelve open-content torrents that download, seed and pause like real ones.
//! Nothing touches the network or the disk.

use std::sync::Mutex;
use std::time::Instant;

use super::Engine;
use crate::model::{Detail, FileEntry, Peer, SessionStats, Snapshot, Torrent, TorrentState, bucket_pieces};

const GB: u64 = 1 << 30;
const MB: u64 = 1 << 20;
const KB: u64 = 1 << 10;

struct Sim {
    t: Torrent,
    /// The rate it downloads or seeds at when running.
    rate_down: u64,
    rate_up: u64,
    files: Vec<(String, u64, bool)>,
}

struct Inner {
    sims: Vec<Sim>,
    next_id: u64,
    last: Instant,
    limit_down: Option<u64>,
    limit_up: Option<u64>,
    frozen: bool,
}

pub struct DemoEngine(Mutex<Inner>);

#[allow(clippy::too_many_arguments)]
fn sim(id: u64, name: &str, size: u64, pct: u64, state: TorrentState, down: u64, up: u64, peers: (u32, u32), ratio: f64, cat: &str, tracker: &str) -> Sim {
    let done = size / 100 * pct;
    let files = if name.ends_with(".iso") {
        vec![(name.to_owned(), size - 50 * KB, true), ("SHA256SUMS".into(), KB, true), ("SHA256SUMS.gpg".into(), 833, true), ("release-notes.html".into(), 48 * KB, false)]
    } else if name.contains('.') && !name.ends_with("2026") {
        vec![(name.to_owned(), size, true)]
    } else {
        (1..=6).map(|i| (format!("{name}/part-{i:02}.bin"), size / 6, true)).collect()
    };
    Sim {
        t: Torrent {
            id,
            hash: format!("{:040x}", 0xa41f_0000_9c0e_u128.wrapping_mul(id as u128 * 7919 + 13)),
            name: name.into(),
            state,
            error: None,
            size,
            done,
            uploaded: (done as f64 * ratio) as u64,
            down: if state == TorrentState::Downloading { down } else { 0 },
            up: if matches!(state, TorrentState::Downloading | TorrentState::Seeding) { up } else { 0 },
            eta_secs: None,
            peers_live: peers.0,
            peers_seen: peers.1,
            pieces: (size / (2 * MB)).max(1) as u32,
            save_path: "~/Downloads".into(),
            category: Some(cat.into()),
            trackers: if tracker.is_empty() { Vec::new() } else { vec![tracker.into()] },
            private: false,
        },
        rate_down: down,
        rate_up: up,
        files,
    }
}

impl DemoEngine {
    pub fn new() -> Self {
        use TorrentState::*;
        let sims = vec![
            sim(1, "ubuntu-26.10-desktop-amd64.iso", 6_549_825_126, 64, Downloading, 18_400 * KB + 400 * KB, 1_200 * KB + 29 * KB, (48, 312), 0.21, "Linux ISOs", "torrent.ubuntu.com"),
            sim(2, "Sintel.2010.4K.OpenMovie.mkv", 12 * GB + 819 * MB, 31, Downloading, 22 * MB + 717 * KB, 840 * KB, (61, 204), 0.08, "Films", ""),
            sim(3, "archlinux-2026.11.01-x86_64.iso", GB + 307 * MB, 92, Downloading, 6 * MB + 922 * KB, 2 * MB + 102 * KB, (33, 590), 0.64, "Linux ISOs", "tracker.archlinux.org"),
            sim(4, "wikipedia_en_all_maxi_2026-10.zim", 118 * GB, 8, Downloading, 410 * KB, 0, (4, 19), 0.0, "Archives", ""),
            sim(5, "Fedora-Workstation-Live-43-1.6.iso", 2 * GB + 410 * MB, 100, Seeding, 0, 3 * MB + 410 * KB, (12, 88), 2.41, "Linux ISOs", "torrent.fedoraproject.org"),
            sim(6, "Big.Buck.Bunny.2008.2160p.60fps.mp4", 4 * GB + 717 * MB, 100, Seeding, 0, MB + 102 * KB, (7, 41), 5.02, "Films", ""),
            sim(7, "debian-13.2.0-arm64-DVD-1.iso", 3 * GB + 922 * MB, 100, Seeding, 0, 620 * KB, (5, 133), 1.37, "Linux ISOs", "bttracker.debian.org"),
            sim(8, "Tears.of.Steel.2012.4K.mkv", 6 * GB + 307 * MB, 100, Seeding, 0, 96 * KB, (2, 17), 0.92, "Films", ""),
            sim(9, "blender-5.1-demo-files", 9 * GB + 205 * MB, 47, Paused, 4 * MB, 300 * KB, (0, 0), 0.11, "Software", ""),
            sim(10, "Project.Gutenberg.DVD.2026", 8 * GB, 73, Checking, 3 * MB, 200 * KB, (0, 0), 0.33, "Archives", ""),
            sim(11, "LibreOffice_26.2_MacOS_aarch64.dmg", 312 * MB, 0, Queued, 5 * MB, 100 * KB, (0, 0), 0.0, "Software", ""),
            sim(12, "nasa-apollo-11-flight-journal-audio", 21 * GB + 512 * MB, 100, Completed, 0, 0, (0, 9), 1.0, "Archives", ""),
        ];
        Self(Mutex::new(Inner { sims, next_id: 13, last: Instant::now(), limit_down: None, limit_up: None, frozen: false }))
    }

    /// Stops the clock, so every snapshot is the same (screenshots and tests).
    pub fn frozen() -> Self {
        let e = Self::new();
        e.lock().frozen = true;
        e
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Default for DemoEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Inner {
    fn find(&mut self, id: u64) -> anyhow::Result<&mut Sim> {
        self.sims.iter_mut().find(|s| s.t.id == id).ok_or_else(|| anyhow::anyhow!("no torrent {id}"))
    }

    fn advance(&mut self) {
        let dt = self.last.elapsed().as_secs_f64();
        self.last = Instant::now();
        if self.frozen {
            return;
        }
        let running: Vec<u64> = self.sims.iter().filter(|s| s.t.state == TorrentState::Downloading).map(|s| s.rate_down).collect();
        let want: u64 = running.iter().sum();
        let scale = match self.limit_down {
            Some(l) if want > l => l as f64 / want as f64,
            _ => 1.0,
        };
        let up_want: u64 = self.sims.iter().filter(|s| matches!(s.t.state, TorrentState::Downloading | TorrentState::Seeding)).map(|s| s.rate_up).sum();
        let up_scale = match self.limit_up {
            Some(l) if up_want > l => l as f64 / up_want as f64,
            _ => 1.0,
        };
        // A little wobble so the numbers look alive.
        let phase = self.last.elapsed().as_secs_f64() + std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
        for (i, s) in self.sims.iter_mut().enumerate() {
            let wobble = 1.0 + 0.12 * (phase * 0.7 + i as f64 * 1.9).sin();
            match s.t.state {
                TorrentState::Downloading => {
                    s.t.down = (s.rate_down as f64 * scale * wobble) as u64;
                    s.t.up = (s.rate_up as f64 * up_scale * wobble) as u64;
                    s.t.done = (s.t.done + (s.t.down as f64 * dt) as u64).min(s.t.size);
                    s.t.uploaded += (s.t.up as f64 * dt) as u64;
                    if s.t.done >= s.t.size {
                        s.t.state = TorrentState::Seeding;
                        s.t.down = 0;
                    }
                }
                TorrentState::Seeding => {
                    s.t.up = (s.rate_up as f64 * up_scale * wobble) as u64;
                    s.t.uploaded += (s.t.up as f64 * dt) as u64;
                }
                _ => {
                    s.t.down = 0;
                    s.t.up = 0;
                }
            }
        }
    }
}

impl Engine for DemoEngine {
    fn snapshot(&self, detail: Option<u64>) -> Snapshot {
        let mut g = self.lock();
        g.advance();
        let mut torrents: Vec<Torrent> = g.sims.iter().map(|s| s.t.clone()).collect();
        for t in &mut torrents {
            t.eta_secs = (t.state == TorrentState::Downloading && t.down > 0).then(|| (t.size - t.done) / t.down);
        }
        let detail = detail.and_then(|id| g.sims.iter().find(|s| s.t.id == id)).map(|s| {
            let total = s.t.pieces as usize;
            let have = (s.t.progress() as f64 * total as f64) as usize;
            // Mostly in order, with a ragged front where pieces are in flight.
            let front = (total / 12).max(1);
            let map = bucket_pieces(|i| i + front < have || (i < have + front && (i * 2654435761) % 5 < 3 && i < have + front / 2) || (i * 40503) % 97 == 0 && i < have + front * 3, total);
            let included: u64 = s.files.iter().filter(|f| f.2).map(|f| f.1).sum();
            Detail {
                id: s.t.id,
                files: s
                    .files
                    .iter()
                    .enumerate()
                    .map(|(index, (path, size, inc))| FileEntry {
                        index,
                        path: path.clone(),
                        size: *size,
                        done: if *inc && included > 0 { (*size as f64 * s.t.done.min(included) as f64 / included as f64) as u64 } else { 0 },
                        included: *inc,
                    })
                    .collect(),
                peers: if s.t.peers_live == 0 {
                    Vec::new()
                } else {
                    [("185.125.190.59:6881", "µTorrent 3.6", 4_100), ("91.189.95.21:51413", "qBittorrent 5.2", 3_600), ("[2a01:4f8:c17::2]:6881", "Transmission 4.1", 2_900), ("84.17.46.201:42000", "libtorrent 2.1", 2_200), ("203.0.113.77:4240", "rqbit 9.1", 1_400), ("198.51.100.4:6889", "qBittorrent 5.1", 1_100)]
                        .iter()
                        .take(s.t.peers_live as usize)
                        .map(|(a, c, k)| Peer { addr: (*a).into(), client: (*c).into(), state: "live".into(), downloaded: k * KB * 190, uploaded: k * KB * 12 })
                        .collect()
                },
                pieces_have: have as u32,
                piece_map: map,
            }
        });
        let session = SessionStats {
            down: torrents.iter().map(|t| t.down).sum(),
            up: torrents.iter().map(|t| t.up).sum(),
            dht_nodes: 412,
            port: Some(51413),
            limit_down: g.limit_down,
            limit_up: g.limit_up,
            alt_limits: false,
            free_disk: Some(812 * GB),
            download_dir: "~/Downloads".into(),
            ..Default::default()
        };
        Snapshot { torrents, session, detail }
    }

    fn add(&self, source: &str, paused: bool, dir: Option<&str>, trackers: &[String]) -> anyhow::Result<u64> {
        anyhow::ensure!(!source.trim().is_empty(), "nothing to add");
        let mut g = self.lock();
        let id = g.next_id;
        g.next_id += 1;
        let name = source
            .split("dn=")
            .nth(1)
            .map(|s| s.split('&').next().unwrap_or(s).replace('+', " "))
            .unwrap_or_else(|| source.rsplit('/').next().unwrap_or(source).trim_end_matches(".torrent").to_owned());
        let mut s = sim(id, &name, 2 * GB, 0, if paused { TorrentState::Paused } else { TorrentState::Downloading }, 9 * MB, 400 * KB, (14, 60), 0.0, "", "");
        s.t.category = None;
        if let Some(d) = dir {
            s.t.save_path = d.into();
        }
        s.t.trackers.extend(trackers.iter().filter_map(|u| crate::trackers::host(u)));
        g.sims.push(s);
        Ok(id)
    }

    fn add_trackers(&self, id: u64, trackers: &[String]) -> anyhow::Result<(u64, usize)> {
        let mut g = self.lock();
        let s = g.find(id)?;
        anyhow::ensure!(!s.t.private, "#{id} is a private torrent");
        let before = s.t.trackers.len();
        for h in trackers.iter().filter_map(|u| crate::trackers::host(u)) {
            if !s.t.trackers.contains(&h) {
                s.t.trackers.push(h);
            }
        }
        Ok((id, s.t.trackers.len() - before))
    }

    fn offline(&self) -> bool {
        true
    }

    fn pause(&self, id: u64) -> anyhow::Result<()> {
        let mut g = self.lock();
        let s = g.find(id)?;
        if !matches!(s.t.state, TorrentState::Completed | TorrentState::Paused) {
            s.t.state = if s.t.finished() { TorrentState::Completed } else { TorrentState::Paused };
            s.t.down = 0;
            s.t.up = 0;
            s.t.peers_live = 0;
        }
        Ok(())
    }

    fn resume(&self, id: u64) -> anyhow::Result<()> {
        let mut g = self.lock();
        let s = g.find(id)?;
        if !matches!(s.t.state, TorrentState::Downloading | TorrentState::Seeding) {
            s.t.state = if s.t.finished() { TorrentState::Seeding } else { TorrentState::Downloading };
            s.t.down = if s.t.finished() { 0 } else { s.rate_down };
            s.t.up = s.rate_up;
            s.t.peers_live = 9;
            s.t.peers_seen = s.t.peers_seen.max(40);
        }
        Ok(())
    }

    fn remove(&self, id: u64, _delete_files: bool) -> anyhow::Result<()> {
        let mut g = self.lock();
        g.find(id)?;
        g.sims.retain(|s| s.t.id != id);
        Ok(())
    }

    fn set_files(&self, id: u64, only: &[usize]) -> anyhow::Result<()> {
        let mut g = self.lock();
        let s = g.find(id)?;
        anyhow::ensure!(only.iter().all(|&i| i < s.files.len()), "torrent {id} has {} files", s.files.len());
        for (i, f) in s.files.iter_mut().enumerate() {
            f.2 = only.contains(&i);
        }
        Ok(())
    }

    fn set_limits(&self, down: Option<u64>, up: Option<u64>) {
        let mut g = self.lock();
        g.limit_down = down;
        g.limit_up = up;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pausing_stops_a_download_and_resuming_restarts_it() {
        let e = DemoEngine::frozen();
        e.pause(1).unwrap();
        let t = e.snapshot(None).torrents.into_iter().find(|t| t.id == 1).unwrap();
        assert_eq!((t.state, t.down), (TorrentState::Paused, 0));
        e.resume(1).unwrap();
        assert_eq!(e.snapshot(None).torrents[0].state, TorrentState::Downloading);
        assert!(e.pause(99).is_err());
    }

    #[test]
    fn skipping_a_file_shows_in_the_detail() {
        let e = DemoEngine::frozen();
        e.set_files(1, &[0, 1]).unwrap();
        let d = e.snapshot(Some(1)).detail.unwrap();
        assert_eq!(d.files.iter().map(|f| f.included).collect::<Vec<_>>(), [true, true, false, false]);
        assert!(e.set_files(1, &[9]).is_err());
    }
}
