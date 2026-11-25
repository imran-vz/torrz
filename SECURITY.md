# Security policy

## Supported versions

Torrz is early and has no stable releases yet. Security fixes go to the `main` branch.

## Reporting a vulnerability

Please do not open a public issue. Report it privately through GitHub instead:
**Security → Report a vulnerability** on
[github.com/imran-vz/torrz](https://github.com/imran-vz/torrz/security/advisories/new).

Include what an attacker can do, how to reproduce it, and the version or commit you tested.
You'll get a reply within a week. Once a fix is ready, we'll agree on when to publish and
credit you in the advisory unless you'd rather not be named.

## Scope

Worth reporting:

- The command socket (`torrz.sock`): it should only accept the user who started Torrz.
- Anything that lets a magnet link, a `.torrent` file or a tracker run code, or read or write
  files outside the download folder.
- The watched folder and "Move to Trash" acting on files they shouldn't.

Bugs in [librqbit](https://github.com/ikatson/rqbit) or [Rune](https://github.com/anomalyco/rune)
belong with those projects, though a note here is welcome if Torrz is affected.
