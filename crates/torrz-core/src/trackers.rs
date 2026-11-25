//! Public trackers from newTrackon, ranked for this machine.
//!
//! newTrackon's stable list is already the trackers that have stayed up (95% or better). On
//! top of that, Torrz asks each one itself: a UDP tracker gets the connect handshake (BEP 15)
//! and an HTTP(S) one a request to its announce URL, TLS included. A TCP connection alone
//! isn't enough: many HTTPS trackers sit behind a CDN whose nearest edge answers in a few
//! milliseconds whatever the tracker is doing. Those that don't answer (or answer with a server
//! error) are dropped, a host listed twice keeps its fastest address, and the fastest [`KEEP`]
//! are the "best". A tracker reachable only over IPv6 is left out when this machine has no IPv6
//! route.

use std::io::ErrorKind;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

use anyhow::Context;

use crate::model::RankedTracker;

/// The list Torrz starts from.
pub const NEWTRACKON: &str = "https://newtrackon.com/api/stable";
/// How many trackers count as the best.
pub const KEEP: usize = 20;
/// How long a probe waits for an answer.
const PROBE_TIMEOUT: Duration = Duration::from_millis(2500);

/// The tracker URLs in newTrackon's answer: one a line, blank lines between, deduplicated.
pub fn parse_list(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if scheme(line).is_some() && host(line).is_some() && !out.iter().any(|u| u == line) {
            out.push(line.to_owned());
        }
    }
    out
}

/// `udp`, `http` or `https`, the schemes a tracker can have.
pub fn scheme(url: &str) -> Option<&'static str> {
    let u = url::Url::parse(url).ok()?;
    ["udp", "http", "https"].into_iter().find(|s| *s == u.scheme())
}

/// A tracker URL's host, lowercased.
pub fn host(url: &str) -> Option<String> {
    url::Url::parse(url).ok()?.host_str().map(str::to_ascii_lowercase)
}

/// Whether a tracker URL carries what looks like a private tracker's passkey: such a torrent
/// must not be given public trackers.
pub fn looks_private(url: &str) -> bool {
    let Ok(u) = url::Url::parse(url) else { return false };
    let lower = url.to_ascii_lowercase();
    lower.contains("passkey") || lower.contains("authkey") || u.path_segments().into_iter().flatten().any(|seg| seg.len() >= 16 && seg.chars().all(|c| c.is_ascii_alphanumeric()))
}

/// `magnet` with each of `trackers` it doesn't name yet added as a `tr` parameter.
pub fn magnet_with_trackers(magnet: &str, trackers: &[String]) -> String {
    let mut out = magnet.to_owned();
    for t in trackers {
        let encoded: String = url::form_urlencoded::byte_serialize(t.as_bytes()).collect();
        if !magnet.contains(t.as_str()) && !magnet.contains(&encoded) {
            out += &format!("{}tr={encoded}", if out.contains('?') { "&" } else { "?" });
        }
    }
    out
}

/// Whether this machine can reach the IPv6 internet. Connecting a UDP socket sends nothing; it
/// only asks the system for a route.
pub fn has_ipv6() -> bool {
    UdpSocket::bind("[::]:0").and_then(|s| s.connect("[2001:4860:4860::8888]:53")).is_ok()
}

/// newTrackon's stable list, without IPv6-only trackers unless `ipv6`.
pub fn fetch(ipv6: bool) -> anyhow::Result<Vec<String>> {
    let url = format!("{NEWTRACKON}?include_ipv6_only_trackers={}", if ipv6 { 1 } else { 0 });
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    let text = rt.block_on(async {
        let client = reqwest::Client::builder().timeout(Duration::from_secs(15)).user_agent(concat!("Torrz/", env!("CARGO_PKG_VERSION"))).build()?;
        let resp = client.get(&url).send().await?.error_for_status()?;
        anyhow::Ok(resp.text().await?)
    });
    let list = parse_list(&text.with_context(|| format!("couldn't fetch {NEWTRACKON}"))?);
    anyhow::ensure!(!list.is_empty(), "{NEWTRACKON} listed no trackers");
    Ok(list)
}

/// How long a UDP tracker took to answer the connect handshake, or `None` when it didn't.
pub fn probe_udp_url(url: &str, ipv6: bool) -> Option<Duration> {
    let u = url::Url::parse(url).ok()?;
    if u.scheme() != "udp" {
        return None;
    }
    let host = u.host_str()?.trim_start_matches('[').trim_end_matches(']').to_owned();
    let addrs: Vec<SocketAddr> = (host.as_str(), u.port()?).to_socket_addrs().ok()?.collect();
    // IPv4 first: it is the route most machines have.
    let addr = addrs.iter().find(|a| a.is_ipv4()).or_else(|| addrs.iter().find(|a| ipv6 && a.is_ipv6()))?;
    probe_udp(*addr)
}

/// How long each HTTP(S) tracker took to answer a request to its announce URL. Any answer short
/// of a server error counts: a tracker turns down a bare request, but it is the tracker that
/// turned it down.
fn probe_web(urls: &[String]) -> Vec<(String, Option<Duration>)> {
    let silent = || urls.iter().map(|u| (u.clone(), None)).collect();
    let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() else { return silent() };
    let Ok(client) = reqwest::Client::builder().timeout(PROBE_TIMEOUT).redirect(reqwest::redirect::Policy::none()).user_agent(concat!("Torrz/", env!("CARGO_PKG_VERSION"))).build() else { return silent() };
    rt.block_on(async {
        let mut set = tokio::task::JoinSet::new();
        for u in urls {
            let (c, u) = (client.clone(), u.clone());
            set.spawn(async move {
                let start = Instant::now();
                let answered = c.get(&u).send().await.is_ok_and(|r| !r.status().is_server_error());
                (u, answered.then(|| start.elapsed()))
            });
        }
        let mut out = Vec::new();
        while let Some(r) = set.join_next().await {
            out.extend(r.ok());
        }
        out
    })
}

/// The UDP tracker connect request: a magic number, action 0 and a transaction id; the tracker
/// echoes the action and the id with a connection id.
fn probe_udp(addr: SocketAddr) -> Option<Duration> {
    let socket = UdpSocket::bind(if addr.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" }).ok()?;
    socket.set_read_timeout(Some(PROBE_TIMEOUT)).ok()?;
    socket.connect(addr).ok()?;
    let tx: u32 = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(7) ^ std::process::id();
    let mut req = [0u8; 16];
    req[..8].copy_from_slice(&0x0417_2710_1980u64.to_be_bytes());
    req[12..].copy_from_slice(&tx.to_be_bytes());
    let start = Instant::now();
    socket.send(&req).ok()?;
    let mut buf = [0u8; 64];
    loop {
        match socket.recv(&mut buf) {
            Ok(n) if n >= 16 && buf[..4] == [0, 0, 0, 0] && buf[4..8] == tx.to_be_bytes() => return Some(start.elapsed()),
            Ok(_) => continue,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return None,
        }
    }
}

/// The trackers that answered, fastest first, one a host, at most [`KEEP`].
pub fn rank(urls: &[String], ipv6: bool) -> Vec<RankedTracker> {
    let (udp, web): (Vec<String>, Vec<String>) = urls.iter().cloned().partition(|u| scheme(u) == Some("udp"));
    let timed: Vec<(String, Option<Duration>)> = std::thread::scope(|s| {
        let jobs: Vec<_> = udp.iter().map(|u| (u.clone(), s.spawn(move || probe_udp_url(u, ipv6)))).collect();
        // The HTTP ones run here meanwhile.
        let mut timed = probe_web(&web);
        timed.extend(jobs.into_iter().map(|(u, j)| (u, j.join().ok().flatten())));
        timed
    });
    pick(timed)
}

/// [`rank`] after the probes: drops the silent, sorts by time, keeps the fastest of each host.
pub fn pick(timed: Vec<(String, Option<Duration>)>) -> Vec<RankedTracker> {
    let mut answered: Vec<(String, Duration)> = timed.into_iter().filter_map(|(u, d)| Some((u, d?))).collect();
    // UDP is cheaper to announce to, so it wins a tie.
    answered.sort_by_key(|(u, d)| (*d, scheme(u) != Some("udp")));
    let mut out: Vec<RankedTracker> = Vec::new();
    for (url, d) in answered {
        if out.len() == KEEP {
            break;
        }
        if out.iter().any(|t| host(&t.url) == host(&url)) {
            continue;
        }
        out.push(RankedTracker { url, ms: d.as_millis().min(u32::MAX as u128) as u32 });
    }
    out
}

/// The best trackers right now: newTrackon's list, probed from here.
pub fn best() -> anyhow::Result<Vec<RankedTracker>> {
    let ipv6 = has_ipv6();
    let list = fetch(ipv6)?;
    let ranked = rank(&list, ipv6);
    anyhow::ensure!(!ranked.is_empty(), "none of the {} trackers newTrackon lists answered from here", list.len());
    Ok(ranked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_parses_and_dedupes() {
        let text = "udp://a.example:80/announce\n\nhttp://b.example:6969/announce\n\nudp://a.example:80/announce\n\nnot a url\n\nwss://c.example/announce\n";
        assert_eq!(parse_list(text), ["udp://a.example:80/announce", "http://b.example:6969/announce"]);
    }

    #[test]
    fn ranking_drops_the_silent_and_keeps_one_a_host() {
        let ms = |n| Some(Duration::from_millis(n));
        let ranked = pick(vec![
            ("http://slow.example:80/announce".into(), ms(300)),
            ("udp://fast.example:6969/announce".into(), ms(20)),
            ("http://fast.example:80/announce".into(), ms(15)),
            ("udp://dead.example:1337/announce".into(), None),
            ("udp://tie.example:1/announce".into(), ms(40)),
            ("http://tie2.example:1/announce".into(), ms(40)),
        ]);
        let urls: Vec<&str> = ranked.iter().map(|t| t.url.as_str()).collect();
        assert_eq!(urls, ["http://fast.example:80/announce", "udp://tie.example:1/announce", "http://tie2.example:1/announce", "http://slow.example:80/announce"]);
        assert_eq!(ranked[0].ms, 15);
    }

    #[test]
    fn magnets_gain_trackers_they_lack() {
        let m = magnet_with_trackers("magnet:?xt=urn:btih:abc&tr=udp%3A%2F%2Fa.example%3A80%2Fannounce", &["udp://a.example:80/announce".into(), "udp://b.example:80/announce".into()]);
        assert_eq!(m, "magnet:?xt=urn:btih:abc&tr=udp%3A%2F%2Fa.example%3A80%2Fannounce&tr=udp%3A%2F%2Fb.example%3A80%2Fannounce");
    }

    #[test]
    fn passkeys_look_private() {
        assert!(looks_private("https://tracker.example/announce?passkey=1234"));
        assert!(looks_private("https://tracker.example/a1b2c3d4e5f6a7b8c9d0/announce"));
        assert!(!looks_private("udp://tracker.opentrackr.org:1337/announce"));
    }
}

/// Hits the network: `cargo test -p torrz-core live_ranking -- --ignored --nocapture`.
#[cfg(test)]
#[test]
#[ignore]
fn live_ranking() {
    let started = Instant::now();
    let best = best().expect("a ranking");
    for t in &best {
        println!("{:>5} ms  {}", t.ms, t.url);
    }
    println!("ipv6: {}, took {:?}", has_ipv6(), started.elapsed());
    assert!(!best.is_empty() && best.len() <= KEEP);
}
