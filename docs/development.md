# Development

## Build and run

```sh
cargo build                         # debug binaries in target/debug
./target/debug/torrz open --demo    # the window on a simulated swarm
./target/debug/torrz-app --demo     # the same, in the foreground with its output
```

The first build compiles Rune and librqbit and takes a few minutes. Use `--demo` while
working on the interface: it needs no network and writes nothing.

To keep a development copy away from your real torrents, give it its own data folder:

```sh
export TORRZ_HOME=/tmp/torrz-dev
```

Unix socket paths are limited to about 100 characters. If `TORRZ_HOME` is a long path, set
`TORRZ_SOCK` to a short one as well.

## Test

```sh
cargo test          # everything
cargo clippy --all-targets
```

| Where | What it covers |
|---|---|
| `torrz-core` unit tests | Rates and sizes, the demo engine, target resolution, limits, file selection, the socket, and every command round-tripping through its CLI line |
| `torrz-app/src/tests.rs` | The real window, headless: clicks, keys, dialogs, the palette, sorting, and the rule that every control has a command |

The window tests build and lay out the real view but draw nothing, so they run in well
under a second.

One thing to know when writing them: an element that has just been mounted is still
animating in and cannot be clicked yet. The `settle` helper in `tests.rs` advances the
clock; use it (through `click`, `key` and `run`) after anything that opens a dialog or
changes which controls exist.

## See the window without opening it

Debug builds take Rune's tooling flags, and the release build has them too (the `tooling`
feature). They render with the same renderer as the window, on the frozen demo engine.

```sh
A=./target/debug/torrz-app
$A --list-scenes
$A --snap shot.png --scene palette --settle            # a PNG of a named state
$A --snap shot.png --scene selected --size 900x600 --scale 2
$A --tree tree.json --scene files --settle             # every element: key, role, rect, text
```

Scenes are named states for these snapshots, defined in `spec()` in
`crates/torrz-app/src/main.rs`. Add one when you add a screen. The screenshots in
`design/screens` are made this way.

## Add a command

Adding a control and adding a command are the same job. In order:

1. **`crates/torrz-core/src/command.rs`:** add a variant to `Command` (or to `Ui` if it only
   moves the window), and its line in `to_cli`.
2. **`crates/torrz-core/src/cli.rs`:** add the clap subcommand with doc comments (they are
   the `--help` text), map it in `into_parsed`, and add a case to the round-trip test.
3. **Run it:** an engine command goes in `Backend::try_exec`
   (`crates/torrz-core/src/backend.rs`); a window command goes in `apply_ui`
   (`crates/torrz-app/src/state.rs`).
4. **Draw it:** build the control with `act(element, command)`. If it belongs in the
   palette, add it to `palette_items`.
5. **Print it:** if its answer deserves more than its message, add a case to `print` in
   `crates/torrz-cli/src/main.rs`.
6. **Document it** in `docs/cli.md`.

`cargo test` will tell you if step 2 or 4 was missed.

A handler must not call the engine or touch the disk: it runs on the UI thread. Return a
`Command` and let `dispatch` route it.

## The icon

The icon is drawn by a script, not by hand, on a 48 by 48 pixel grid with ordered (Bayer)
dithering.

```sh
scripts/make-icon.py              # every variant, into design/icons/
scripts/make-icon.py classic      # and make that one the app's icon
scripts/render-icons.sh           # then rasterise the PNGs the app embeds (needs Chrome)
```

`make-icon.py classic` writes `assets/icon.svg` (light), `assets/icon-dark.svg`, and
`assets/Torrz.icon`, an Icon Composer document holding both. `render-icons.sh` turns the two
SVGs into `assets/icon-light.png` and `assets/icon-dark.png`, which `crates/torrz-app`
embeds for the Dock icon. The PNGs are checked in, so a normal build needs neither script.

## The Mac app bundle

```sh
scripts/make-app.sh               # writes dist/Torrz.app
```

It builds both binaries in release mode, compiles `assets/Torrz.icon` with Xcode's `actool`
into `Assets.car` (light, dark and tinted, which macOS 26 and later switch between) and a
fallback `Torrz.icns`, writes `Info.plist` with the `magnet:` scheme, and signs the bundle.

Signing is ad hoc unless you name an identity:

```sh
TORRZ_SIGN="Developer ID Application: Your Name (TEAMID)" scripts/make-app.sh
```

The bundle is not notarised.

## The website

`site/` holds the website: a static landing page and these docs, served by Cloudflare. The
docs pages are made from the Markdown in `docs/`, so after changing a doc, rebuild:

```sh
cd site
npm install            # once: the Markdown renderer
npm run build          # writes site/public
```

Commit `site/public` with the change. The landing page is `site/src/index.html`. The site
is built on [Oat](https://oat.ink), a small classless UI library: it styles the semantic
HTML and supplies the grid (`.row`, `.col-*`), cards, buttons, tables, tabs (`<ot-tabs>`)
and the docs sidebar (`data-sidebar-layout`). Oat 1.1.0 is vendored in `site/public/vendor/oat`
with its license; `site/public/assets/site.css` holds Torrz's theme (Oat's variables) and the
few pieces Oat has no component for. Prefer an Oat component or class to new CSS. Screenshots in `site/public/screens/`
come from `torrz-app --snap` (see above) at `--scale 2`, saved as WebP.

The link preview card, `site/public/og.png`, is drawn as HTML in `site/og/og.html`; run
`site/og/render.sh` after changing it (it uses headless Chrome).

To deploy, sign in once with `npx wrangler login`, then run `npm run deploy` (build, then
upload). `npx wrangler dev` serves the site locally first. Nothing runs on the server: the
Worker named in `site/wrangler.jsonc` serves the files in `site/public` as they are.

## Updating Rune

Rune is pre-release and is used as a git dependency pinned to one revision of its `dev`
branch. Cargo does not inherit a dependency's `[patch.crates-io]` entries, so the root
`Cargo.toml` repeats Rune's own patch list at the same revision.

To update:

1. Pick a revision of `dev` and read Rune's `CHANGELOG.md` between the old one and it.
2. Check Rune's root `Cargo.toml` at that revision for its current `[patch.crates-io]` list.
3. Replace the revision everywhere in this repo's `Cargo.toml`: the `rune` dependency and
   every patch entry. They must all match.
4. `cargo test`, then look at a few `--snap` screenshots.

Rune takes issues, not pull requests.

## Conventions

- **Comments say why**, and say what the code does now. No history, no notes to reviewers.
- **Errors are sentences a person can act on**: "no torrent "99"", not "not found".
- **No blocking on the UI thread**, and no file access in views or handlers.
- **Everything the user can do is a `Command`.** If you are reaching for a closure that
  changes state directly, it should be a command instead.
