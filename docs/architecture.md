# Architecture

Torrz is built on one rule: **the only way anything changes is a `Command`.** A click, a key,
a row in the ⌘K palette and a `torrz` line in a terminal each produce the same value, and
one function runs it. The window and the command line cannot drift apart, because neither
has a way to do something the other cannot.

## The crates

| Crate | Binary | Holds |
|---|---|---|
| `crates/torrz-core` | none | `Command` and its CLI parser, the engines, categories and limits, the socket |
| `crates/torrz-app` | `torrz-app` | The window: state, views, and the threads that connect it to the engine and the socket |
| `crates/torrz-cli` | `torrz` | Parses a line into a `Command`, sends it, prints the answer |

`torrz-core` knows nothing about drawing, and `torrz-cli` does not link Rune or librqbit's
session. The CLI parser lives in core, not in the CLI binary, so that the app's tests can
check their controls against it.

## A command's life

```
   click / key / palette row                      torrz pause 1
              │                                         │
              │ Command                                 │ clap → Command → JSON line
              ▼                                         ▼
        dispatch(state, cmd)  ◄── UI thread ──  socket thread (one per connection)
              │
     ┌────────┴─────────┐
     │ Command::Ui(..)  │  anything else
     ▼                  ▼
  apply_ui          engine thread ──► Backend::exec ──► Engine (librqbit)
  (changes the           │
   window's state)       └──► AppHandle::update: log the result, take a fresh snapshot
```

1. **Something makes a `Command`** (`crates/torrz-core/src/command.rs`). In the window,
   every control is built with `act(element, command)` in `view/mod.rs`, which attaches the
   press handler and uses the command's own shell line as the element's key.
2. **`dispatch`** (`crates/torrz-app/src/state.rs`) runs it. Window commands (`Command::Ui`)
   change `State` directly and return. Everything else may block on the network or the
   disk, so it is sent to the engine thread.
3. **The engine thread** calls `Backend::exec`, then hands the window the result and a
   fresh snapshot through Rune's `AppHandle::update`, which runs on the UI thread.
4. **`State::finish`** appends the command to the log as its CLI line, with who asked (`ui`
   or `cli`) and what came back, and shows a toast for errors.

`Command::to_cli` turns any command back into the line that produces it. The palette, the
Log tab and the element keys all use it, and a test proves that every line parses back to
the command it came from.

## Threads

| Thread | Does | May block |
|---|---|---|
| UI (main) | Runs Rune's event loop, `dispatch`, and the views | Never |
| `torrz-engine-jobs` | Runs engine commands that came from the window; takes a snapshot every second | Yes |
| `torrz-ipc` and one per connection | Accept socket connections; run engine commands from the CLI directly, and pass window commands to the UI thread | Yes |
| `torrz-engine` (tokio pool) | librqbit's own runtime | Its own business |

The UI thread never calls the engine. It holds a `Snapshot` (every torrent, the session's
totals, and the inspected torrent's files, peers and piece map) and redraws when a new one
arrives. The other threads learn what is selected from a small shared mirror
(`state::Shared`) instead of asking the UI thread.

## The engine

`torrz_core::engine::Engine` is a small trait: `snapshot`, `add`, `pause`, `resume`,
`remove`, `set_files`, `set_limits`. There are two implementations.

- **`RqbitEngine`** wraps a librqbit `Session` and its `Api`, on a tokio runtime it owns.
  It listens on TCP and uTP, with UPnP port forwarding, DHT, and fast resume.
- **`DemoEngine`** simulates twelve torrents that download, seed and pause. Tests,
  headless screenshots and `--demo` use it. `DemoEngine::frozen()` stops its clock so every
  snapshot is identical.

**`Backend`** sits on top of whichever engine is running and adds what librqbit does not
have: categories (kept by info hash), speed limits with an alternate pair, and the icon
choice. It owns `config.json` and runs every command that is not about the window.

## The socket protocol

The app listens on a Unix socket (mode 0600) at `torrz.sock` in its data folder. The
protocol is one JSON object a line in each direction, and a connection may send several.
`crates/torrz-core/src/ipc.rs` is all of it.

A request is a `Command`, tagged by `cmd`; window commands are tagged again by `ui`:

```json
{"cmd":"pause","targets":["1"]}
{"cmd":"add","source":"magnet:?xt=urn:btih:…","paused":true,"category":"Films"}
{"cmd":"ui","ui":"filter","name":"paused"}
```

The answer is always the same shape:

```json
{"ok":true,"message":"paused 1 torrent","data":[1]}
{"ok":false,"message":"no torrent \"99\"","data":null}
```

`message` is for people; `data` is the command's result, documented with each command in
the [CLI reference](cli.md). A line that is not a valid command gets `ok: false` and the
connection stays open.

You can talk to it without the CLI:

```sh
printf '{"cmd":"stats"}\n' | nc -U "$HOME/Library/Application Support/Torrz/torrz.sock"
```

A second copy of the app finds the socket answering, asks the first to show its window,
and exits. A socket file nobody answers on is left over from a crash and is replaced.

## The window

The window is a [Rune](https://github.com/anomalyco/rune) app: a `State`, a `view` function
from state to elements, and handlers that change state between frames.

| File | Draws |
|---|---|
| `view/mod.rs` | The root, the status bar, global keys, and `act` |
| `view/sidebar.rs` | Library filters, categories, trackers, the alternate-limits switch |
| `view/table.rs` | The toolbar and the torrent table (virtualised rows) |
| `view/inspector.rs` | The six inspector tabs, including the speed graph and piece map |
| `view/overlays.rs` | The command palette, the dialogs, the toast |
| `theme.rs` | Colours, type and icons |
| `icon.rs` | The Dock icon, light or dark |

Views read state and never change it. The table's rows are derived: `State::recompute_rows`
filters, searches and sorts the snapshot into a list of indices whenever one of those
inputs changes.

## How parity is enforced

`crates/torrz-app/src/tests.rs` drives the real view headless, with real pointer and key
events, against the demo engine. One test,
`every_control_in_every_screen_has_a_command_line`, renders every screen (each dialog, the
palette, each inspector tab, the sidebar hidden), finds every element with an interactive
role, and requires its key to be a line the CLI parser accepts. A control built without
`act`, or a command the parser does not know, fails it.

Another test performs the same steps once by clicking and once by commands, and requires
`torrz ui state` to come out identical.
