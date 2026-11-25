# Contributing

Thanks for looking. Torrz is small and early, and issues and pull requests are both welcome.

## Before you start

- For anything larger than a fix, open an issue first so the approach can be agreed before
  you write it.
- Read [docs/architecture.md](docs/architecture.md). The one rule that shapes everything is
  that every change to the app goes through a `Command`, which makes it available from the
  window and the command line at once.

## Making a change

1. Build and run it on the demo swarm: `cargo build && ./target/debug/torrz open --demo`.
2. Follow [Add a command](docs/development.md#add-a-command) if you are adding anything a
   person can do.
3. Run `cargo test` and `cargo clippy --all-targets`. New behaviour needs a test; for the
   window, that is a headless test in `crates/torrz-app/src/tests.rs`.
4. If you changed how something looks, attach a screenshot. `torrz-app --snap` makes one
   without opening a window ([how](docs/development.md#see-the-window-without-opening-it)).
5. Update `docs/cli.md` when a command's arguments or output change, and add a line to
   `CHANGELOG.md` under Unreleased.
6. If you changed anything in `docs/`, rebuild the website so it matches:
   `cd site && npm install && npm run build`, and commit `site/public`.

## Reporting a bug

Include what you did, what you expected and what happened, your OS version, and the
relevant lines from the inspector's Log tab (or `torrz ui state --json`). The log records
every command that ran, which is usually enough to reproduce a problem.

Please do not attach `.torrent` files or magnet links for copyrighted material.

## License

By contributing you agree that your work is released under the [MIT license](LICENSE).
