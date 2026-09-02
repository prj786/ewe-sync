# ewe-sync — the account and sync app

**Your ewe account, in one place** — the Nextcloud account, the one file
(`ewe.conf`), the machines that share it, and (next release) the folders that
sync with it. A tray icon shows the state; the window is four panes.

ewe-sync is the account app of [ewe OS](https://github.com/prj786/ewe-os)
(RFC-006; drafted under the working name Flock). Komble installs apps and records them in the one file;
ewe-settings edits the one file; **ewe-sync moves the one file and your folders
between your machines**. Nothing else in ewe has a sync button.

## What it does

| pane | what |
|---|---|
| Account | sign in to *your* Nextcloud (any provider, or your own server) through the server's own login page; who you are, storage; sign out; links to providers where you can create an account |
| This machine | the one file: backup saved by which machine and when, when this machine last synced, auto-sync switch, Sync now / Back up / Restore |
| Machines | your machines — every machine that backed up to the account, with its ewe version and app count |
| Folders | which folders sync where: two-way through the Nextcloud sync engine, or one-way copies; on change, on a timer, or at login; conflicts resolved in place |

Tray: state icon (idle · syncing · conflict · offline · signed out), menu
with Sync now · Pause auto-sync · Open ewe-sync · Quit. Left-click opens the
window.

## Folder sync

A folder pair is a local folder and a folder in your account. The
definitions live in `ewe.conf` as `[[sync.folders]]` (remote, local, mode,
trigger, interval, exclude) and follow you to every machine; the local path
can be overridden per machine (`~/.local/state/ewe/ewe-sync.json`, which
also keeps each pair's last run and conflict list).

- **Two-way** runs `nextcloudcmd`, the headless twin of the Nextcloud
  desktop client, one folder pair per run, runs serialised. Renames,
  deletions and conflicts are the engine's — the same behaviour as the
  desktop client, without its window and tray. Credentials go through a
  `.netrc` (0600) in a private `HOME` under `~/.local/state/ewe/`, so the app
  password is never on a command line.
- **Upload only / download only** copy with rclone over WebDAV (`copy`: adds
  and updates, never deletes). rclone is configured through environment
  variables for that one process; no config file.
- **Triggers:** on change (inotify on the local folder, a burst of writes
  becomes one run after 5 quiet seconds), every N minutes, or once at login.
  Automatic runs honour the auto-sync switch; "Sync now" always runs.
- **Conflicts:** when both sides changed a file the engine keeps the server's
  version and writes yours as `name (conflicted copy <date>).ext`. The
  Folders pane lists them; *Keep mine* renames your copy over the original
  (it uploads on the next run), *Keep theirs* deletes the copy. Only paths
  inside the pair's folder are ever touched.

Limits: no selective sync inside a pair (use excludes), no bandwidth
limits, and the `nextcloudcmd` flags were written against the documented
interface — see `src-tauri/src/folders.rs` → `mod nccmd` if a client version
disagrees.

## What it stores where

- The **app password** your server hands out at sign-in: the keyring only
  (`secret-tool`, service `ewe-cloud`). Revoke it any time in Nextcloud →
  Settings → Security (“ewe”).
- **Non-secret account facts** (server, login name, display name):
  `~/.config/ewe/cloud.json`.
- **The one file** `~/.config/ewe/ewe.conf` and, in your account,
  `ewe/ewe.conf` + `ewe/ewe.conf.meta.json` (who saved it, when) +
  `ewe/machines/<name>.json` (your machines).
- Nothing about you in this repository, the package, or anyone's servers but
  your own.

## What it cannot do

Create accounts. Nextcloud's user-creation API needs administrator
credentials, and a provider's signup form is theirs. ewe-sync links to signup
pages; you create the account in the browser and sign in here.

## How it works

ewe-sync is a thin UI over the ewe tools, run as argv, never through a shell:

- `ewe-cloud` — Login Flow v2, account facts, the app password
- `ewe-conf` — push / pull / sync-status of the one file (WebDAV with the
  server's own `If-Match` conflict guard), and `[[sync.folders]]`
- `nextcloudcmd` / `rclone` — the folder engines (see Folder sync)
- `ewe-auth keyring-reset` — the keyring playbook when a prompt keeps
  rejecting the login password
- `qs ipc call cloud refresh` — keeps the desktop shell's account card in step

Nothing needs root; there is no privileged helper.

## Build

```bash
npm install
npm run tauri dev
```

Arch package: `makepkg -si` (the PKGBUILD is the packaging path; Tauri has
no pacman bundler). `options=(!lto)` and `npm run tauri build -- --no-bundle`
are load-bearing — see Komble's PKGBUILD for why.

`ewe-sync --hidden` starts tray-only (the autostart unit does this);
`ewe-sync --check` prints one JSON line with the tools it found, the
account status, the sync status and your machines — for support and the smoke
test.

## Tests

`tests/folders-test.sh` round-trips the pair model through `ewe-conf`
(sandboxed) and, when `nextcloudcmd` exists on the host, syncs a tree both
ways against the mock; the conflict-name parser and path guard are Rust
unit tests (`cargo test`).

`tests/smoke.sh` starts the mock Nextcloud from prj786/ewe, signs in with
`ewe-cloud` against it in a sandboxed `XDG_*` home, and — when a built
`ewe-sync` is available — runs `ewe-sync --check` against that state.
Without the binary it still builds the frontend and checks the tools.

## Licence

MIT.
