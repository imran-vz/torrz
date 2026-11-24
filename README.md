<img src="assets/icon.svg" width="64" height="64" alt="">

# Torrz

A desktop BitTorrent client whose every control is also a command. The window is built with
[Rune](https://github.com/anomalyco/rune); the engine is [librqbit](https://github.com/ikatson/rqbit).

## Run it

```sh
cargo build
./target/debug/torrz open          # start the app (real engine, saves to ~/Downloads)
./target/debug/torrz open --demo   # or a simulated swarm: nothing touches the network
```

Or build the Mac app, which has the icon and takes `magnet:` links:

```sh
scripts/make-app.sh                # writes dist/Torrz.app (needs Xcode)
cp -R dist/Torrz.app /Applications/
```

Then drive it from the terminal while the window is open:

```sh
torrz add 'magnet:?xt=urn:btih:…' --category "Linux ISOs"
torrz list
torrz pause 1 2            # ids, info hashes, `all` or `selected`
torrz files 1 --skip 3
torrz limit --down 12M --up none
torrz ui filter downloading
torrz ui select 1
torrz ui tab peers
torrz ui palette on --query "pause"
torrz ui icon dark         # the app icon: auto (follows the system), light or dark
torrz ui state             # everything the window shows, as JSON
torrz --help
```

In the window: ⌘K opens the command palette (⌘C there copies the highlighted row as its CLI
line), ⌘O adds a torrent, Space pauses or resumes the selection, ⌫ removes it, ⌘F searches,
⌘I toggles the inspector. The inspector's Log tab shows every click as the command it ran.

## How it fits together

| Crate | What it is |
|---|---|
| `crates/torrz-core` | The `Command` set, the CLI parser, the engine (librqbit, plus a demo engine), categories and limits, and the Unix socket the CLI talks over. |
| `crates/torrz-app` | The window (`torrz-app`). Every control dispatches a `Command`; nothing changes state any other way. |
| `crates/torrz-cli` | `torrz`: parses a line into a `Command`, sends it to the running app, prints the answer. |

A test in `torrz-app` renders every screen headless and fails if any control lacks a command
line the CLI accepts, so the two cannot drift apart.

```sh
cargo test                                             # core, CLI round-trips, headless UI
./target/debug/torrz-app --snap shot.png --scene palette --settle   # render a screen to PNG
```

The icon is drawn by `scripts/make-icon.py classic`, in a light and a dark version. The
bundle carries both, and macOS 26 and later switch between them with the system's appearance.
While the app runs it shows the one you chose (⌘K, "App Icon", or `torrz ui icon`).

State lives in `~/Library/Application Support/Torrz` on macOS (`$TORRZ_HOME` overrides it;
`$TORRZ_SOCK` moves the socket). `design/mocks/index.html` has the original UI directions.

## Not there yet

- Windows: the command socket is Unix-only.
- BitTorrent v2, and piece picking other than sequential (librqbit limits).
- Column resizing, a tray icon, and opening `.torrent` files from Finder (dropping one on
  the window works).

Created in [T3 Code](https://t3.codes).

## License

MIT. See [LICENSE](LICENSE).
