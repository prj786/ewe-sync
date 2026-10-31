# ewe-sync — the account and sync app

**Your ewe account, in one place** — sign in once and everything follows:
your settings and apps (`ewe.conf`), your browser, your passwords and your
folders, on every computer you sign in to. The bar shows the state; the
window opens on the Overview.

ewe-sync is the account app of [ewe OS](https://github.com/prj786/ewe-os)
(RFC-006; drafted under the working name Flock). Komble installs apps and records them in the one file;
ewe-settings edits the one file; **ewe-sync moves the one file and your folders
between your machines**. Nothing else in ewe has a sync button.

## What it does

| pane | what |
|---|---|
| Overview | everything the account keeps, on one page: settings & apps, the browser, passwords (Safe), folders — each with its state and, where it is this computer's choice, a switch. Signed out it says what signing in brings; on a **new computer** whose account holds a backup it offers **Bring everything back** (what comes back, with sizes, before you say yes); on the **first** computer it offers the folders worth keeping (with sizes against the account's free space). One **Sync now** for everything |
| Account | sign in to *your* Nextcloud (any provider, or your own server) through the server's own login page; who you are, storage; sign out; links to providers where you can create an account |
| Mail | any IMAP mailbox — the inbox your Nextcloud provider gives you, one you host, or a work account: add, change, remove, check unread. The password goes to the keyring; only host/user/port reach `ewe.conf`. (Whether new mail *notifies* is a per-machine setting, in Settings → User) |
| Google | the optional extra: where your own `oauth-client.json` goes and whether it parses, connect / disconnect, Gmail and Drive state. ewe ships no Google client — never settings sync |
| Settings & apps | the one file: backup saved by which machine and when, when this computer last synced, auto-sync switch, Sync now / Back up / Restore, the decision when two computers changed it |
| Machines | your machines — every machine that backed up to the account, with its ewe version and app count |
| Folders | every folder connected to the account: the ones ewe-sync keeps in step (open, sync now, pause on this computer, change, stop syncing), suggested user folders one click away, folders the Nextcloud desktop app still syncs (with **Let ewe-sync take over**), and the whole account as `~/Nextcloud`; conflicts resolved in place |

The bar shows the state (syncing · needs you · offline — silent while all is
well); a click opens the window. Rule 6 holds: the browser and Safe sync by
themselves, ewe-sync is the one place that shows them and its Sync now asks
them to go (the browser watches `~/.local/state/ewe/sync-now`; Safe gets
`ewe-safe sync`).

## Folder sync

A folder pair is a local folder and a folder in your account. The
definitions live in `ewe.conf` as `[[sync.folders]]` (remote, local, mode,
trigger, interval, exclude, hidden) and **follow you to every computer** —
`ewe-conf sync` merges them like any other shared key, and a running
ewe-sync picks up folders another computer added and brings them down. The
local path can differ per computer and each computer can pause a folder for
itself (`~/.local/state/ewe/ewe-sync.json`, which also keeps each pair's
last run, conflicts and the account folder's ETag).

- **Two-way** runs `nextcloudcmd`, the headless twin of the Nextcloud
  desktop client (flags checked against Nextcloud 35.0.0), one folder pair
  per run, runs serialised and de-duplicated. A folder missing in the account
  is created first. Credentials go through a `.netrc` (0600) in a private
  `HOME` under `~/.local/state/ewe/`, so the app password is never on a
  command line.
- **Upload only / download only** copy with rclone over WebDAV (`copy`: adds
  and updates, never deletes). rclone is configured through environment
  variables for that one process; no config file.
- **When it runs** ("automatic", the default): when the local folder changes
  (inotify; a burst of writes becomes one run after 5 quiet seconds), when
  the folder in the account changes (its ETag, asked every 45 s — Nextcloud
  moves a folder's ETag whenever anything inside changes, the signal the
  desktop client uses), and once shortly after start. Or every N minutes, or
  once at login. Automatic runs honour the auto-sync switch and the pause;
  "Sync now" always runs.
- **Where a folder may go:** never your whole home folder, never one synced
  folder inside another, never inside `~/Nextcloud` while that is the
  account's mount, never on a folder the Nextcloud desktop app syncs.
- **The Nextcloud desktop app:** its folders for this account (read from
  `~/.config/Nextcloud/nextcloud.cfg`) are listed as connected. *Let
  ewe-sync take over* quits the app (`nextcloud --quit`), moves its
  autostart entry to `~/.local/state/ewe/nextcloud-app/` (kept, not
  deleted) and adds the same folders here — the files stay where they are.
- **Conflicts:** when both sides changed a file the engine keeps the server's
  version and writes yours as `name (conflicted copy <date>).ext`; a
  notification says so. The Folders pane lists them; *Keep mine* renames your
  copy over the original (it uploads on the next run), *Keep theirs* deletes
  the copy. Only paths inside the pair's folder are ever touched.
- **What other apps see:** every change of state is written to
  `~/.local/state/ewe/sync-folders.json` (the folders, their state, the
  whole-account folder) — ewe browser's file manager shows synced folders
  and their state from it, `ewe-files` reads it — and each connected folder
  that is not a standard user folder gets a GTK bookmark, so Nemo and every
  Open/Save dialog list it (only bookmarks ewe-sync added are removed).

## What it stores where

- The **app password** your server hands out at sign-in: the keyring only
  (`secret-tool`, service `ewe-cloud`). Revoke it any time in Nextcloud →
  Settings → Security (“ewe”).
- **Non-secret account facts** (server, login name, display name):
  `~/.config/ewe/cloud.json`.
- **The one file** `~/.config/ewe/ewe.conf` and, in your account,
  `ewe/ewe.conf` + `ewe/ewe.conf.meta.json` (who saved it, when) +
  `ewe/machines/<name>.json` (your machines).
- **This computer's folder state** (local paths, pauses, last runs,
  conflicts): `~/.local/state/ewe/ewe-sync.json`; what other apps read:
  `~/.local/state/ewe/sync-folders.json`; the "sync now, everyone" nudge:
  `~/.local/state/ewe/sync-now`. None of these leave the computer.
- Your **folders** stay where they are; their copies live in your account
  under the names you gave them.
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

`ewe-sync --hidden` starts without a window (the autostart unit does this);
`ewe-sync --pane <overview|folders|machine|machines|account|mail|google>`
opens the window on a pane; `ewe-sync --add-folder <path>` opens Folders with
that folder ready to add (ewe browser's "Keep in your account…"); both reach
a running ewe-sync. `ewe-sync --status` prints `sync-folders.json` as one
JSON line; `ewe-sync --check` prints the tools it found, the account status,
the sync status and your machines — for support and the smoke test.

## Tests

`tests/folders-test.sh` round-trips the pair model through `ewe-conf`
(sandboxed) and, when `nextcloudcmd` exists on the host, syncs a tree both
ways against the mock; the conflict-name parser and path guard are Rust
unit tests (`cargo test`).

`tests/e2e-nextcloud.sh` is the promise end to end: a throwaway real
Nextcloud in podman on 127.0.0.1, two sandboxed computers each running the
built ewe-sync — a file written on A goes up, B signs in and gets the folder
list and the file, a file added in the account and an edit on B reach A, and
the snapshot, `--status` and the bookmark agree. It skips (exit 0) without
podman, the `nextcloud:stable` image, `nextcloudcmd` or a built binary.

`tests/smoke.sh` starts the mock Nextcloud from prj786/ewe, signs in with
`ewe-cloud` against it in a sandboxed `XDG_*` home, and — when a built
`ewe-sync` is available — runs `ewe-sync --check` against that state.
Without the binary it still builds the frontend and checks the tools.

## Licence

MIT.
