# Changelog

All notable changes to Torrz are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/) once 1.0 ships.

## [Unreleased]

### Added

- The Graphite window: a sidebar of filters, categories and trackers; a sortable table; an
  inspector with General, Files, Peers, Trackers, Pieces and Log tabs.
- The `torrz` command line, with a command for every control in the window, and the ⌘K
  command palette that shows each action's command line.
- The real engine on librqbit (magnet links, DHT, peer exchange, uTP, UDP trackers, fast
  resume, speed limits, UPnP) and a `--demo` simulated swarm.
- A Mac app bundle that registers `magnet:` links, with light and dark dithered icons.
- `torrz ui default-app`: ask macOS to open magnet links with Torrz.
- A watched folder (`torrz watch`) that adds each `.torrent` file that lands in it.
- "Move the .torrent file to the Trash" when adding a file (`torrz add --trash`).
- A right-click menu on torrent rows: resume, pause, show files, reveal, copy name, hash or
  magnet link, category, add best trackers, remove.
- Best public trackers from newTrackon, probed from your machine and ranked by speed
  (`torrz trackers best|add|auto`). Private torrents are never given public trackers.
- Live per-peer speeds, connection type and piece times on the Peers tab, with a bar of
  where the download is coming from.
- A dithered piece map that animates pieces in flight and as they arrive.
- A website with the docs, at [torrz.imran.codes](https://torrz.imran.codes).
- A code of conduct, a security policy, issue and pull request templates, and CI.

[Unreleased]: https://github.com/imran-vz/torrz/commits/main
