//! The real engine: a `librqbit` session on its own tokio runtime. Every method blocks the calling
//! thread, so none of them may be called from the UI thread.

use std::collections::{HashMap, HashSet};
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::Context;
use librqbit::api::TorrentIdOrHash;
use librqbit::{AddTorrent, AddTorrentOptions, Api, ListenerMode, ListenerOptions, Session, SessionOptions, SessionPersistenceConfig, TorrentStatsState};

use super::Engine;
use crate::model::{Detail, FileEntry, Peer, SessionStats, Snapshot, Torrent, TorrentState, bucket_pieces};

pub struct RqbitEngine {
    rt: tokio::runtime::Runtime,
    api: Api,
    download_dir: PathBuf,
    /// Where librqbit keeps each torrent's `.torrent` and resume bitfield.
    session_dir: PathBuf,
    /// The inspected torrent's peers as last seen, to turn their byte counts into speeds.
    peer_samples: Mutex<HashMap<(u64, String), PeerSample>>,
}

/// A peer's byte counts when last measured, and the speeds worked out then.
struct PeerSample {
    at: Instant,
    fetched: u64,
    uploaded: u64,
    down: u64,
    up: u64,
}

/// librqbit counts bytes per peer but not speed: the speed is the change since the last
/// sample. A sample under half a second old keeps the speed it had, so a second snapshot taken
/// close behind another (a `torrz show`) doesn't read as a stall.
fn measure_peers(samples: &Mutex<HashMap<(u64, String), PeerSample>>, id: u64, peers: &mut [Peer]) {
    let mut g = samples.lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    g.retain(|(t, addr), _| *t == id && peers.iter().any(|p| &p.addr == addr));
    for p in peers {
        let s = g.entry((id, p.addr.clone())).or_insert(PeerSample { at: now, fetched: p.downloaded, uploaded: p.uploaded, down: 0, up: 0 });
        let dt = now.duration_since(s.at).as_secs_f64();
        if dt >= 0.5 {
            s.down = (p.downloaded.saturating_sub(s.fetched) as f64 / dt) as u64;
            s.up = (p.uploaded.saturating_sub(s.uploaded) as f64 / dt) as u64;
            (s.at, s.fetched, s.uploaded) = (now, p.downloaded, p.uploaded);
        }
        (p.down, p.up) = (s.down, s.up);
    }
}

pub struct RqbitOptions {
    pub download_dir: PathBuf,
    /// Where the session keeps its torrent list and resume data.
    pub state_dir: PathBuf,
    pub listen_port: u16,
    /// Join the DHT. Off only for tests, which would clash with a running Torrz over its port.
    pub dht: bool,
}

impl RqbitEngine {
    pub fn start(o: RqbitOptions) -> anyhow::Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().thread_name("torrz-engine").build()?;
        std::fs::create_dir_all(&o.download_dir).with_context(|| format!("cannot create {}", o.download_dir.display()))?;
        std::fs::create_dir_all(&o.state_dir)?;
        let session_dir = o.state_dir.join("session");
        let opts = SessionOptions {
            fastresume: true,
            persistence: Some(SessionPersistenceConfig::Json { folder: Some(session_dir.clone()) }),
            listen: Some(ListenerOptions {
                mode: ListenerMode::TcpAndUtp,
                listen_addr: (std::net::Ipv6Addr::UNSPECIFIED, o.listen_port).into(),
                enable_upnp_port_forwarding: true,
                ..Default::default()
            }),
            client_name_and_version: Some(format!("Torrz {}", env!("CARGO_PKG_VERSION"))),
            dht: if o.dht { SessionOptions::default().dht } else { None },
            disable_local_service_discovery: !o.dht,
            ..Default::default()
        };
        let session: Arc<Session> = rt.block_on(Session::new_with_opts(o.download_dir.clone(), opts)).context("cannot start the torrent session")?;
        Ok(Self { rt, api: Api::new(session, None), download_dir: o.download_dir, session_dir, peer_samples: Mutex::default() })
    }

    fn id(id: u64) -> TorrentIdOrHash {
        TorrentIdOrHash::Id(id as usize)
    }
}

fn err(e: librqbit::ApiError) -> anyhow::Error {
    anyhow::anyhow!("{e:#}")
}

impl Engine for RqbitEngine {
    fn snapshot(&self, detail: Option<u64>) -> Snapshot {
        // Entering the runtime lets librqbit's internals that expect a tokio context work.
        let _g = self.rt.enter();
        let session = self.api.session();
        let list = self.api.api_torrent_list_ext(librqbit::api::ApiTorrentListOpts { with_stats: true });
        let mut torrents = Vec::with_capacity(list.torrents.len());
        for t in list.torrents {
            let Some(id) = t.id else { continue };
            let Some(stats) = t.stats else { continue };
            let finished = stats.finished;
            let (down, up, live_peers, seen) = match &stats.live {
                Some(l) => (l.download_speed.as_bytes(), l.upload_speed.as_bytes(), l.snapshot.peer_stats.live, l.snapshot.peer_stats.seen),
                None => (0, 0, 0, 0),
            };
            let state = match stats.state {
                TorrentStatsState::Error => TorrentState::Error,
                TorrentStatsState::Initializing { .. } => TorrentState::Checking,
                TorrentStatsState::Paused if finished => TorrentState::Completed,
                TorrentStatsState::Paused => TorrentState::Paused,
                TorrentStatsState::Live if finished => TorrentState::Seeding,
                TorrentStatsState::Live => TorrentState::Downloading,
            };
            let left = stats.total_bytes.saturating_sub(stats.progress_bytes);
            let handle = session.get(Self::id(id as u64));
            let trackers = handle
                .as_ref()
                .map(|h| {
                    let mut v: Vec<String> = h.shared().trackers.iter().filter_map(|u| u.host_str().map(str::to_owned)).collect();
                    v.sort();
                    v.dedup();
                    v
                })
                .unwrap_or_default();
            let private = handle.as_ref().and_then(|h| h.with_metadata(|m| m.info.info().private).ok()).unwrap_or(false);
            torrents.push(Torrent {
                id: id as u64,
                name: t.name.unwrap_or_else(|| format!("{}…", &t.info_hash[..12.min(t.info_hash.len())])),
                hash: t.info_hash,
                state,
                error: stats.error,
                size: stats.total_bytes,
                done: stats.progress_bytes,
                uploaded: stats.uploaded_bytes,
                down: if state == TorrentState::Downloading { down } else { 0 },
                up,
                eta_secs: (state == TorrentState::Downloading && down > 0).then(|| left / down),
                peers_live: live_peers,
                peers_seen: seen,
                pieces: t.total_pieces,
                save_path: t.output_folder,
                category: None,
                trackers,
                private,
            });
        }
        torrents.sort_by_key(|t| t.id);

        let ss = self.api.api_session_stats();
        let dht_nodes = self.api.api_dht_stats().map(|d| (d.routing_table_size + d.routing_table_size_v6) as u32).unwrap_or(0);
        let stats = SessionStats {
            down: ss.download_speed.as_bytes(),
            up: ss.upload_speed.as_bytes(),
            dht_nodes,
            port: session.announce_port(),
            limit_down: None,
            limit_up: None,
            alt_limits: false,
            free_disk: None,
            download_dir: self.download_dir.display().to_string(),
            ..Default::default()
        };

        let detail = detail.and_then(|id| {
            let d = self.api.api_torrent_details(Self::id(id)).ok()?;
            let progress = self.api.api_stats_v1(Self::id(id)).map(|s| s.file_progress).unwrap_or_default();
            let files = d
                .files
                .unwrap_or_default()
                .into_iter()
                .enumerate()
                .map(|(index, f)| FileEntry { index, path: f.components.join("/"), size: f.length, done: progress.get(index).copied().unwrap_or(0), included: f.included })
                .collect();
            let mut peers: Vec<Peer> = self
                .api
                .api_peer_stats(Self::id(id), Default::default())
                .map(|p| {
                    p.peers
                        .into_iter()
                        .map(|(addr, s)| {
                            let pieces = s.counters.downloaded_and_checked_pieces;
                            Peer {
                                addr,
                                client: s.client_name.unwrap_or_default(),
                                state: s.state.to_owned(),
                                downloaded: s.counters.fetched_bytes,
                                uploaded: s.counters.uploaded_bytes,
                                kind: s.conn_kind.map(|k| k.to_string()).unwrap_or_default(),
                                down: 0,
                                up: 0,
                                pieces,
                                piece_ms: if pieces == 0 { 0 } else { s.counters.total_piece_download_ms / pieces as u64 },
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            measure_peers(&self.peer_samples, id, &mut peers);
            // The fastest now first, then the ones that gave the most.
            peers.sort_by(|a, b| b.down.cmp(&a.down).then(b.downloaded.cmp(&a.downloaded)).then_with(|| a.addr.cmp(&b.addr)));
            let (piece_map, pieces_have) = match self.api.api_dump_haves(Self::id(id)) {
                Ok((bf, total)) => {
                    let total = total as usize;
                    (bucket_pieces(|i| bf.get(i).map(|b| *b).unwrap_or(false), total), bf.iter().take(total).filter(|b| **b).count() as u32)
                }
                Err(_) => (Vec::new(), 0),
            };
            Some(Detail { id, files, peers, piece_map, pieces_have })
        });
        Snapshot { torrents, session: stats, detail }
    }

    fn add(&self, source: &str, paused: bool, dir: Option<&str>, trackers: &[String]) -> anyhow::Result<u64> {
        let source = source.trim();
        anyhow::ensure!(!source.is_empty(), "nothing to add: give a magnet link, a URL or a .torrent file");
        let mut opts = AddTorrentOptions { paused, overwrite: true, output_folder: dir.map(str::to_owned), ..Default::default() };
        let add = match AddTorrent::from_cli_argument(source)? {
            // librqbit reads a magnet's trackers from the link alone. A private torrent rarely
            // comes as a magnet; one whose tracker carries a passkey is left as it is.
            AddTorrent::Url(m) if m.starts_with("magnet:") && !trackers.is_empty() && !magnet_tracker_urls(&m).iter().any(|t| crate::trackers::looks_private(t)) => {
                AddTorrent::Url(crate::trackers::magnet_with_trackers(&m, trackers).into())
            }
            AddTorrent::TorrentFileBytes(bytes) => {
                let private = librqbit::torrent_from_bytes(&bytes).map(|t| t.info.data.private).unwrap_or(true);
                if !private && !trackers.is_empty() {
                    opts.trackers = Some(trackers.to_vec());
                }
                AddTorrent::TorrentFileBytes(bytes)
            }
            // An http(s) URL is downloaded by librqbit, so whether it is private isn't known
            // here: it keeps its own trackers.
            other => other,
        };
        let r = self.rt.block_on(self.api.api_add_torrent(add, Some(opts))).map_err(err)?;
        r.id.map(|i| i as u64).context("the torrent was listed but not added")
    }

    fn add_trackers(&self, id: u64, extra: &[String]) -> anyhow::Result<(u64, usize)> {
        let _g = self.rt.enter();
        let h = self.api.session().get(Self::id(id)).with_context(|| format!("no torrent {id}"))?;
        let private = h.with_metadata(|m| m.info.info().private).map_err(|_| anyhow::anyhow!("#{id} has no metadata yet; try again once it starts downloading"))?;
        anyhow::ensure!(!private, "#{id} is a private torrent: it may only use its own tracker");
        let mut have: Vec<String> = h.shared().trackers.iter().map(|u| u.to_string()).collect();
        let hosts: Vec<String> = have.iter().filter_map(|u| crate::trackers::host(u)).collect();
        let new: Vec<String> = extra.iter().filter(|u| crate::trackers::host(u).is_some_and(|x| !hosts.contains(&x))).cloned().collect();
        if new.is_empty() {
            return Ok((id, 0));
        }
        // librqbit fixes a torrent's trackers when it starts, so it is added again with more.
        // Its resume bitfield is kept across, so it comes back after a spot check of its pieces
        // instead of hashing every byte again.
        let bytes = h.with_metadata(|m| m.torrent_bytes.clone()).map_err(|e| anyhow::anyhow!("{e:#}"))?;
        let opts = |trackers: Option<Vec<String>>| AddTorrentOptions {
            paused: h.is_paused(),
            only_files: h.only_files(),
            output_folder: Some(h.output_folder().display().to_string()),
            overwrite: true,
            trackers,
            ..Default::default()
        };
        let (with, without) = (opts(Some(have.iter().chain(&new).cloned().collect())), opts(Some(std::mem::take(&mut have))));
        let bitv = self.session_dir.join(format!("{:?}.bitv", h.info_hash()));
        let kept = bitv.with_extension("bitv.keep");
        let saved = std::fs::copy(&bitv, &kept).is_ok();
        drop(h);
        self.rt.block_on(self.api.api_torrent_action_forget(Self::id(id))).map_err(err)?;
        if saved {
            let _ = std::fs::rename(&kept, &bitv);
        }
        let add = |o| self.rt.block_on(self.api.api_add_torrent(AddTorrent::TorrentFileBytes(bytes.clone()), Some(o)));
        let r = match add(with) {
            Ok(r) => r,
            // Never leave it gone: put it back as it was.
            Err(e) => {
                let back = add(without).map_err(err).context("and it couldn't be added back; add its .torrent again")?;
                anyhow::bail!("couldn't add the trackers to #{id} ({e:#}); it is back as #{}", back.id.unwrap_or_default());
            }
        };
        Ok((r.id.map(|i| i as u64).context("the torrent was listed but not added")?, new.len()))
    }

    fn pause(&self, id: u64) -> anyhow::Result<()> {
        self.rt.block_on(self.api.api_torrent_action_pause(Self::id(id))).map(|_| ()).map_err(err)
    }

    fn resume(&self, id: u64) -> anyhow::Result<()> {
        self.rt.block_on(self.api.api_torrent_action_start(Self::id(id))).map(|_| ()).map_err(err)
    }

    fn remove(&self, id: u64, delete_files: bool) -> anyhow::Result<()> {
        let f = async {
            if delete_files { self.api.api_torrent_action_delete(Self::id(id)).await } else { self.api.api_torrent_action_forget(Self::id(id)).await }
        };
        self.rt.block_on(f).map(|_| ()).map_err(err)
    }

    fn set_files(&self, id: u64, only: &[usize]) -> anyhow::Result<()> {
        let set: HashSet<usize> = only.iter().copied().collect();
        self.rt.block_on(self.api.api_torrent_action_update_only_files(Self::id(id), &set)).map(|_| ()).map_err(err)
    }

    fn set_limits(&self, down: Option<u64>, up: Option<u64>) {
        let nz = |v: Option<u64>| v.and_then(|b| NonZeroU32::new(b.min(u32::MAX as u64) as u32));
        let l = &self.api.session().ratelimits;
        l.set_download_bps(nz(down));
        l.set_upload_bps(nz(up));
    }
}

/// The `tr` parameters of a magnet link.
fn magnet_tracker_urls(magnet: &str) -> Vec<String> {
    let query = magnet.split_once('?').map(|(_, q)| q).unwrap_or("");
    url::form_urlencoded::parse(query.as_bytes()).filter(|(k, _)| k == "tr").map(|(_, v)| v.into_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn peer(addr: &str, downloaded: u64, uploaded: u64) -> Peer {
        Peer { addr: addr.into(), client: String::new(), state: "live".into(), downloaded, uploaded, kind: "tcp".into(), down: 0, up: 0, pieces: 0, piece_ms: 0 }
    }

    #[test]
    fn peer_speeds_come_from_the_bytes_between_samples() {
        let samples = Mutex::default();
        let mut ps = [peer("a:1", 1_000, 100)];
        measure_peers(&samples, 1, &mut ps);
        assert_eq!((ps[0].down, ps[0].up), (0, 0), "one sample is no speed yet");
        // Pretend the first sample is a second old.
        samples.lock().unwrap().get_mut(&(1, "a:1".to_owned())).unwrap().at -= Duration::from_secs(1);
        let mut ps = [peer("a:1", 501_000, 50_100), peer("b:2", 7, 7)];
        measure_peers(&samples, 1, &mut ps);
        assert!((490_000..=500_000).contains(&ps[0].down), "{}", ps[0].down);
        assert!((49_000..=50_000).contains(&ps[0].up), "{}", ps[0].up);
        // Straight after, the speed holds rather than reading as a stall.
        let mut again = [peer("a:1", 501_000, 50_100)];
        measure_peers(&samples, 1, &mut again);
        assert_eq!(again[0].down, ps[0].down);
        // Another torrent's peers replace these.
        measure_peers(&samples, 2, &mut [peer("c:3", 0, 0)]);
        assert_eq!(samples.lock().unwrap().len(), 1);
    }

    fn wait_complete(e: &RqbitEngine, id: u64) -> Torrent {
        let start = Instant::now();
        loop {
            if let Some(t) = e.snapshot(None).torrents.into_iter().find(|t| t.id == id)
                && t.finished()
                && t.state != TorrentState::Checking
            {
                return t;
            }
            assert!(start.elapsed() < Duration::from_secs(60), "#{id} never finished checking");
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// A real librqbit session on a torrent made here (it binds ports and joins the DHT):
    /// `cargo test -p torrz-core trackers_are_added -- --ignored`.
    #[test]
    #[ignore]
    fn trackers_are_added_by_adding_it_again_with_its_files_and_resume_data() {
        let root = std::env::temp_dir().join(format!("torrz-rqbit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let data = root.join("dl");
        std::fs::create_dir_all(&data).unwrap();
        let file = data.join("payload.bin");
        std::fs::write(&file, (0..8u32 << 20).map(|i| i.wrapping_mul(2654435761) as u8).collect::<Vec<u8>>()).unwrap();
        let e = RqbitEngine::start(RqbitOptions { download_dir: data.clone(), state_dir: root.join("state"), listen_port: 0, dht: false }).unwrap();
        let made = e
            .rt
            .block_on(async {
                let spawner = librqbit::spawn_utils::BlockingSpawner::new(4);
                librqbit::create_torrent(&file, librqbit::CreateTorrentOptions { name: None, trackers: vec!["udp://one.example:6969/announce".into()], piece_length: Some(256 << 10) }, &spawner).await
            })
            .unwrap();
        let tf = root.join("payload.torrent");
        std::fs::write(&tf, made.as_bytes().unwrap()).unwrap();

        let id = e.add(tf.to_str().unwrap(), false, Some(data.to_str().unwrap()), &[]).unwrap();
        let before = wait_complete(&e, id);
        assert_eq!(before.trackers, ["one.example"]);
        assert!(!before.private);
        let bitv = e.session_dir.join(format!("{}.bitv", before.hash));
        assert!(bitv.exists(), "librqbit keeps resume data at {}", bitv.display());

        let (now, added) = e.add_trackers(id, &["udp://two.example:1337/announce".into(), "udp://one.example:6969/announce".into()]).unwrap();
        assert_eq!(added, 1, "only the tracker it lacked");
        let after = wait_complete(&e, now);
        assert_eq!((after.hash.as_str(), after.done, after.trackers.clone()), (before.hash.as_str(), before.size, vec!["one.example".to_owned(), "two.example".to_owned()]));
        assert!(bitv.exists(), "its resume data is still there");
        assert_eq!(e.add_trackers(now, &["udp://two.example:1337/announce".into()]).unwrap(), (now, 0), "nothing new: left alone");
        let _ = std::fs::remove_dir_all(&root);
    }
}
