# CLI reference

`torrz` sends one command to the running app and prints its answer. The app has to be
running: start it with `torrz open`. Apart from `open`, every command here is also something
the window does, and the window's Log tab shows each of them as it runs.

```
torrz [--json] <command> [arguments]
```

- **Exit status** is 0 when the command worked and 1 when it did not; the reason goes to
  standard error.
- **`--json`** prints the app's raw answer instead of the readable one. It works on every
  command, before or after the command's name.
- **`torrz help <command>`** and `torrz <command> --help` print the same text as below.

## Naming torrents

Commands that act on torrents take one or more **targets**:

| Target | Means |
|---|---|
| `3` | The torrent with id 3 (the `ID` column of `torrz list`) |
| a 40-character info hash | That torrent |
| `all` | Every torrent |
| `selected` | Whatever is selected in the window right now |

Ids are handed out by the engine as torrents are added. An info hash is the name to use
in a script that must find the same torrent later. An unknown target is an error and nothing
is changed.

## Rates

Speed limits are a number with a unit: `800k`, `12M`, `1.5m`, `2G` (binary: 1 k is 1024
bytes), all per second. `none`, `off`, `unlimited` and `0` remove the limit.

## Starting the app

### `torrz open [--demo]`

Starts the app, or brings its window forward if it is already running. It returns once the
app answers on its socket.

`--demo` starts a simulated swarm of twelve torrents instead of the real engine. Nothing is
downloaded and no settings are written, so it is safe for trying things out.

`torrz open` looks for `torrz-app` in the same folder as `torrz`.

## Torrents

### `torrz list` (alias `ls`)

Every torrent, one a line.

```
 ID  NAME                                SIZE   DONE  STATUS            DOWN         UP      ETA  RATIO
  1  ubuntu-26.10-desktop-amd64.iso    6.1 GB    64%  Downloading  18.4 MB/s   1.2 MB/s   2m 01s   0.21
```

With `--json`, `data` is a list of torrents:

```json
{
  "id": 1,
  "hash": "00000000000000000000000013dd308412e341c8",
  "name": "ubuntu-26.10-desktop-amd64.iso",
  "state": "paused",
  "error": null,
  "size": 6549825126,
  "done": 4193518281,
  "uploaded": 880403063,
  "down": 0,
  "up": 0,
  "eta_secs": null,
  "peers_live": 0,
  "peers_seen": 312,
  "pieces": 3123,
  "save_path": "~/Downloads",
  "category": "Linux ISOs",
  "trackers": ["torrent.ubuntu.com"]
}
```

Sizes are bytes and speeds are bytes a second. `state` is one of `downloading`, `seeding`,
`paused`, `checking`, `queued`, `completed` or `error`.

### `torrz show <target>`

One torrent with its files, its connected peers and how many pieces it has. With `--json`,
`data` is `{"torrent": …, "detail": …}`, where `detail` has `files` (index, path, size,
bytes done, whether it is included), `peers`, `pieces_have` and `piece_map` (piece
availability squeezed into at most 600 cells, each 0 to 255).

Each peer has `addr`, `client`, `state`, `kind` (`tcp`, `utp` or `socks`), `downloaded` and
`uploaded` (bytes so far), `down` and `up` (bytes a second, measured between snapshots, so
0 on the first look), `pieces` (pieces it gave that passed their hash check) and `piece_ms`
(the average time one took). Peers are listed fastest first. librqbit doesn't report a
peer's own progress, so Torrz can't tell seeds from other downloaders.

### `torrz add <source> [--paused] [--dir <dir>] [--category <name>] [--trash]`

Adds a magnet link, an http(s) URL of a `.torrent`, or the path of a `.torrent` file. Quote
magnet links: they contain `&`.

| Option | Does |
|---|---|
| `--paused` | Add it without starting it |
| `--dir <dir>` | Save it there instead of the download folder |
| `--category <name>` | File it under that category |
| `--trash` | Move the `.torrent` file to the Trash once it is added. Refused for a magnet link or a URL |

Files already in the folder are checked and kept, so adding a torrent you partly have
resumes it. `data` is `{"id": N, "trashed": true|false}`. If the torrent is added but the
file can't be moved to the Trash, the command still succeeds and the message says why.

The app reads the path, so give it absolute (or starting with `~/`).

### `torrz pause <targets…>` and `torrz resume <targets…>` (alias `start`)

Pauses or resumes. `data` is the list of ids affected.

### `torrz rm <targets…> [--delete-files]` (alias `remove`)

Removes torrents from the list. Their downloaded files stay on disk unless you pass
`--delete-files`, which deletes them and cannot be undone. The command line does not ask
for confirmation; the window does.

### `torrz files <target> [--include 0,2] [--skip 3]`

Chooses which files of a torrent to download. Indices are the ones `torrz show` prints,
comma separated. Files you name neither way are left as they are. `data` is the list of
indices that will be downloaded.

### `torrz category <targets…> --name <name>` or `--clear`

Puts torrents in a category, or takes them out of theirs. Categories exist only as long as
a torrent is in them; there is nothing to create first.

### `torrz reveal <target>`

Shows the torrent's files in Finder (or the system's file manager) and prints their path.

### `torrz watch [<dir>] [--off] [--trash on|off]`

Watches a folder: every `.torrent` file that arrives in it is added, as `torrz add` would
add it, within a couple of seconds. The files already in the folder when you start watching
it are left alone. With `--trash on`, each file goes to the Trash once it is added; with
`--trash off` it stays, and Torrz remembers it so it isn't added again. `--off` stops
watching. With no arguments it only reports what is watched.

```sh
torrz watch ~/Downloads --trash on
torrz watch --trash off            # keep the folder, change what happens to the files
torrz watch --off
```

`data` is `{"dir": "/Users/you/Downloads", "trash": true}` (`dir` is `null` when nothing is
watched). Adds made by the watched folder show in the Log tab as coming from `watch`.

### `torrz trackers best [--refresh]`

The best public trackers, fastest first, as `{"url", "ms"}` objects in `data`: newTrackon's
stable list, probed from this machine (a UDP handshake for `udp://`, a request to the
announce URL for `http(s)://`), with the silent ones dropped and at most 20 kept, one per
host. The ranking is kept for six hours; `--refresh` probes again. The demo answers with a
made-up list and never touches the network.

```
$ torrz trackers best
   40 ms  udp://tracker.willy.pro:6969/announce
   87 ms  udp://tracker.cn.nyaa.net:6969/announce
  …
```

### `torrz trackers add <targets…> [--url <url>]…`

Gives torrents the best public trackers, or the `--url` ones instead. A tracker whose host a
torrent already has is skipped, and private torrents are skipped (they may only use their
own tracker). `data` lists each torrent as `{"id", "now", "added"}`: the real engine adds a
torrent again to change its trackers, and `now` is its id afterwards. Its files, file
choices, paused state and resume data carry over.

### `torrz trackers auto [on|off]`

Whether each new torrent gets the best public trackers as it is added; with no argument it
toggles. A `.torrent` file marked private, or a magnet link whose tracker carries a passkey,
keeps only its own. A torrent added from an http(s) URL keeps its own too, since whether it is
private isn't known until librqbit has downloaded it.

## Session

### `torrz stats`

Current speeds and limits, whether the alternate limits are on, the number of DHT nodes,
the listening port, the download folder and its free space.

```json
{
  "down": 29367796,
  "up": 8243857,
  "limit_down": null,
  "limit_up": null,
  "alt_limits": false,
  "dht_nodes": 412,
  "port": 51413,
  "free_disk": 871878361088,
  "download_dir": "~/Downloads",
  "watch_dir": null,
  "watch_trash": false,
  "auto_trackers": false
}
```

### `torrz limit [--down <rate>] [--up <rate>]`

Sets one or both limits; the side you leave out is unchanged. While the alternate limits
are on, this changes the alternate pair, since those are the ones in force.

### `torrz alt-limits [on|off]`

Switches to the alternate limits and back; with no argument it toggles. A fresh install's
alternates are 1 MB/s down and 256 kB/s up.

## The window: `torrz ui …`

These move the window itself. They need a window process but not a visible window: they
work while it is hidden.

### `torrz ui state`

Everything the window shows, as JSON: the filter, the rows in the table in order, the
selection, the sort, the search text, the inspector tab, whether the sidebar and inspector
are open, any open dialog with its fields, the palette with its rows and their command
lines, the toast, the icon style, and the last twenty log lines.

```json
{
  "filter": "paused",
  "title": "Paused",
  "rows": [1, 9],
  "selected": [1],
  "sort": null,
  "search": "",
  "tab": "general",
  "sidebar": true,
  "inspector": true,
  "dialog": null,
  "palette": null,
  "toast": null,
  "icon": "auto",
  "magnet_default": false,
  "demo": true,
  "log": [
    { "time": "02:19:33", "from": "cli", "command": "torrz ui select 1", "result": "selected 1 torrent", "ok": true }
  ]
}
```

### List and selection

| Command | Does |
|---|---|
| `torrz ui filter <name>` | Shows one group: `all`, `downloading`, `seeding`, `completed`, `paused`, `errors`, `category:<name>`, `tracker:<host>`, or `tracker:dht` for torrents with no tracker |
| `torrz ui select [targets…]` | Selects torrents; with none, clears the selection. The last one named is the one the inspector shows |
| `torrz ui search <text>` | Keeps rows whose name contains every word; `""` clears it |
| `torrz ui sort <column> [--asc\|--desc]` | Sorts by `name`, `size`, `progress`, `status`, `down`, `up`, `eta`, `ratio` or `peers`; with neither flag, sorting the same column again reverses it. `added` restores the original order |

### Layout

| Command | Does |
|---|---|
| `torrz ui tab <name>` | Shows an inspector tab: `general`, `files`, `peers`, `trackers`, `pieces` or `log` (and opens the inspector) |
| `torrz ui sidebar [on\|off]` | Shows, hides or toggles the sidebar |
| `torrz ui inspector [on\|off]` | Shows, hides or toggles the inspector |
| `torrz ui icon <style>` | The Dock icon while Torrz runs: `auto` (follows the system), `light` or `dark` |
| `torrz ui window <action>` | `show`, `hide` (torrents keep running), `minimize` or `quit` |
| `torrz ui copy <what> <target>` | Copies a torrent's `name`, `hash` or `magnet` link to the clipboard; `data` is the text |
| `torrz ui default-app` | Asks macOS to open magnet links with Torrz. macOS asks you to confirm, and the answer arrives as a toast. `magnet_default` in `torrz ui state` says whether it does (`null` when Torrz isn't running from its app bundle) |

### Command palette

| Command | Does |
|---|---|
| `torrz ui palette [on\|off] [--query <text>]` | Opens, closes or toggles it; `--query` types into it |
| `torrz ui palette-move <n>` | Moves the highlight `n` rows (negative is up; it wraps) |
| `torrz ui palette-run` | Runs the highlighted row and closes the palette |

### Dialogs

| Command | Does |
|---|---|
| `torrz ui dialog <name>` | Opens `add`, `remove`, `limits` or `watch`; `none` closes whichever is open. `remove` needs a selection |
| `torrz ui field <name> <value>` | Sets a field of the open dialog (table below) |
| `torrz ui confirm` | Presses the dialog's main button |
| `torrz ui cancel` | Closes the palette if it is open, otherwise the dialog |

| Dialog | Fields |
|---|---|
| `add` | `source` (text), `paused` (`on` or `off`), `trash` (`on` or `off`; used only when `source` is a `.torrent` file) |
| `remove` | `delete-files` (`on` or `off`) |
| `limits` | `down`, `up` (a rate, or empty for none) |
| `watch` | `dir` (a folder, or empty to stop watching), `trash` (`on` or `off`) |

A dialog is the long way round: `torrz add`, `torrz rm`, `torrz limit` and `torrz watch` do the same work
directly. The dialog commands exist so a script can drive the window exactly as a person
would, for demos and tests.

`torrz ui confirm` and `torrz ui palette-run` answer as soon as the engine has been asked,
with `running torrz …`; the outcome is in `torrz ui state`'s log.

## Environment

| Variable | Does |
|---|---|
| `TORRZ_HOME` | The data folder (settings, session, socket) |
| `TORRZ_SOCK` | The socket's path alone. Unix socket paths are limited to about 100 characters |
