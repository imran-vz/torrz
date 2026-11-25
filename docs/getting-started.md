# Getting started

Torrz is a desktop BitTorrent client in which every control is also a command. This page
takes you from a clone to a running window, then through the everyday things: adding
torrents, opening magnet links, watching a folder and driving it all from the terminal.

## What you need

- macOS. Torrz is built and tested there. Linux should build but is untested, and Windows
  has no command line yet.
- A recent Rust toolchain (the project is built with Rust 1.99).
- Xcode, for the Mac app bundle: it compiles the light and dark icon.

## Install

```sh
git clone https://github.com/imran-vz/torrz
cd torrz
```

**As a Mac app.** This one has the icon and opens `magnet:` links.

```sh
scripts/make-app.sh
cp -R dist/Torrz.app /Applications/
```

The bundle includes the `torrz` command at `Torrz.app/Contents/MacOS/torrz`. It is signed ad
hoc, not notarised, so on another Mac you approve it the first time you open it.

**As plain binaries** on your `PATH`:

```sh
cargo install --path crates/torrz-app
cargo install --path crates/torrz-cli
```

Install both: `torrz open` starts the `torrz-app` that sits beside it.

## First run

```sh
torrz open           # the real engine, saving to ~/Downloads
torrz open --demo    # a simulated swarm that touches neither the network nor the disk
```

The demo is the quickest way to look around: twelve torrents download, seed and pause, and
every control works.

## Add a torrent

Press ⌘O, paste a magnet link or a URL, or choose a `.torrent` file. You can also drop a
`.torrent` file on the window. When the source is a file, tick **Move the .torrent file to
the Trash** to tidy up once it is added.

From a terminal:

```sh
torrz add 'magnet:?xt=urn:btih:…' --category "Linux ISOs"
torrz add ~/Downloads/file.torrent --trash
```

## Open magnet links from the browser

The app bundle registers `magnet:` links, but if another torrent client got there first,
macOS keeps sending them to it. Choose **Open Magnet Links with Torrz** in the ⌘K palette
(or the link at the bottom of the Add dialog, or run `torrz ui default-app`) and confirm when
macOS asks. A link that arrives while the window is closed brings it back with the Add
dialog filled in.

## Watch a folder

The folder button in the toolbar watches a folder for `.torrent` files and adds each one that
arrives, moving it to the Trash afterwards if you like. Files already in the folder are left
alone.

```sh
torrz watch ~/Downloads --trash on
torrz watch --off
```

While a folder is watched, the status bar says so; click it to change or stop it.

## Better-connected torrents

Torrz can give a torrent the fastest public trackers: it takes
[newTrackon](https://newtrackon.com)'s list of trackers that stay up, probes each from your
machine, and keeps the 20 that answer quickest. Use **Add Best Trackers** on the Trackers
tab or in a row's right-click menu, or tick the box there to give every new torrent them.
Private torrents are never touched.

```sh
torrz trackers best          # the ranking, fastest first
torrz trackers add 1 2
torrz trackers auto on
```

## Drive it from the terminal

With the window open, anything you can click you can run:

```sh
torrz list
torrz pause 1 2                    # ids, info hashes, `all` or `selected`
torrz files 1 --skip 3             # don't download file 3
torrz limit --down 12M --up none
torrz ui filter downloading        # what the sidebar does
torrz ui select 1                  # what clicking a row does
torrz ui tab peers
```

Every command takes `--json` and exits non-zero when it fails, so it scripts cleanly:

```sh
torrz list --json | jq -r '.data[] | select(.state == "seeding") | .name'
```

The [CLI reference](cli.md) has every command and the JSON it returns.

## In the window

| Key | Does |
|---|---|
| ⌘K | Open the command palette |
| ⌘O or ⌘N | Add a torrent |
| ⌘F | Search by name |
| Space | Pause or resume the selection |
| ⌫ | Remove the selection (asks first) |
| ↑ ↓ | Move the selection; with ⇧, extend it |
| ⌘A | Select every row shown |
| ⌘1 … ⌘6 | Inspector tab: General, Files, Peers, Trackers, Pieces, Log |
| ⌘I | Show or hide the inspector |
| ⌥⌘S | Show or hide the sidebar |
| ⌘, | Speed limits |
| Esc | Close the palette or dialog, then clear the search, then the selection |

Click a row to select it, ⌘-click to add or remove one, ⇧-click to select a range, and
double-click to show its files in Finder. Right-click a row for its menu: resume or pause,
show files, reveal in Finder, copy the name, info hash or magnet link, category, add best
trackers, and remove.

Closing the window leaves the torrents running; quit from the palette or with
`torrz ui window quit`.

## Where things are kept

| What | Where |
|---|---|
| Downloads | `~/Downloads` |
| Settings (limits, categories, icon, watched folder, trackers) | `config.json` in the data folder |
| Torrent list and resume data | `session/` in the data folder |
| Command socket | `torrz.sock` in the data folder |

The data folder is `~/Library/Application Support/Torrz` on macOS and
`$XDG_DATA_HOME/torrz` (or `~/.local/share/torrz`) elsewhere. Set `TORRZ_HOME` to move it
and `TORRZ_SOCK` to move only the socket.
