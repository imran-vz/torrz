//! The real engine: a `librqbit` session on its own tokio runtime. Every method blocks the calling
//! thread, so none of them may be called from the UI thread.

use std::collections::HashSet;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use librqbit::api::TorrentIdOrHash;
use librqbit::{AddTorrent, AddTorrentOptions, Api, ListenerMode, ListenerOptions, Session, SessionOptions, SessionPersistenceConfig, TorrentStatsState};

use super::Engine;
use crate::model::{Detail, FileEntry, Peer, SessionStats, Snapshot, Torrent, TorrentState, bucket_pieces};

pub struct RqbitEngine {
    rt: tokio::runtime::Runtime,
    api: Api,
    download_dir: PathBuf,
}

pub struct RqbitOptions {
    pub download_dir: PathBuf,
    /// Where the session keeps its torrent list and resume data.
    pub state_dir: PathBuf,
    pub listen_port: u16,
}

impl RqbitEngine {
    pub fn start(o: RqbitOptions) -> anyhow::Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().thread_name("torrz-engine").build()?;
        std::fs::create_dir_all(&o.download_dir).with_context(|| format!("cannot create {}", o.download_dir.display()))?;
        std::fs::create_dir_all(&o.state_dir)?;
        let opts = SessionOptions {
            fastresume: true,
            persistence: Some(SessionPersistenceConfig::Json { folder: Some(o.state_dir.join("session")) }),
            listen: Some(ListenerOptions {
                mode: ListenerMode::TcpAndUtp,
                listen_addr: (std::net::Ipv6Addr::UNSPECIFIED, o.listen_port).into(),
                enable_upnp_port_forwarding: true,
                ..Default::default()
            }),
            client_name_and_version: Some(format!("Torrz {}", env!("CARGO_PKG_VERSION"))),
            ..Default::default()
        };
        let session: Arc<Session> = rt.block_on(Session::new_with_opts(o.download_dir.clone(), opts)).context("cannot start the torrent session")?;
        Ok(Self { rt, api: Api::new(session, None), download_dir: o.download_dir })
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
                Some(l) => (l.download_speed.as_bytes(), l.upload_speed.as_bytes(), l.snapshot.peer_stats.live as u32, l.snapshot.peer_stats.seen as u32),
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
            let trackers = session
                .get(Self::id(id as u64))
                .map(|h| {
                    let mut v: Vec<String> = h.shared().trackers.iter().filter_map(|u| u.host_str().map(str::to_owned)).collect();
                    v.sort();
                    v.dedup();
                    v
                })
                .unwrap_or_default();
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
                        .map(|(addr, s)| Peer { addr, client: s.client_name.unwrap_or_default(), state: s.state.to_owned(), downloaded: s.counters.fetched_bytes, uploaded: s.counters.uploaded_bytes })
                        .collect()
                })
                .unwrap_or_default();
            peers.sort_by(|a, b| b.downloaded.cmp(&a.downloaded).then_with(|| a.addr.cmp(&b.addr)));
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

    fn add(&self, source: &str, paused: bool, dir: Option<&str>) -> anyhow::Result<u64> {
        let source = source.trim();
        anyhow::ensure!(!source.is_empty(), "nothing to add: give a magnet link, a URL or a .torrent file");
        let opts = AddTorrentOptions { paused, overwrite: true, output_folder: dir.map(str::to_owned), ..Default::default() };
        let add = AddTorrent::from_cli_argument(source)?;
        let r = self.rt.block_on(self.api.api_add_torrent(add, Some(opts))).map_err(err)?;
        r.id.map(|i| i as u64).context("the torrent was listed but not added")
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
