#!/usr/bin/env bash
# The promise, end to end: two computers (A and B — two sandboxed homes, each
# running the real ewe-sync binary in its own D-Bus session) against a REAL
# Nextcloud in a throwaway podman container on 127.0.0.1. Nothing on the host
# is touched: no real account, no real keyring (EWE_CLOUD_FAKE_KEYRING), no
# live shell or notifications (qs and notify-send are shimmed), no session
# reload (HYPRLAND_INSTANCE_SIGNATURE unset).
#   1 A keeps ~/Docs in the account: a file written there goes up (inotify)
#   2 B signs in to the same account: `ewe-conf sync` brings the folder LIST
#     (sync.folders is shared now), B's ewe-sync picks it up by itself and the
#     file comes down — log in, get your files back
#   3 a file added in the account (the web) reaches A (the folder-ETag poll)
#   4 a change made on B reaches A
#   5 A publishes sync-folders.json, `ewe-sync --status` prints it, and the
#     folder has a GTK bookmark
# Needs podman with docker.io/library/nextcloud:stable, nextcloudcmd, a built
# ewe-sync (cargo build in src-tauri) and an ewe checkout (EWE_REPO, default
# ../ewe). Without one of them it says what is missing and exits 0.
set -euo pipefail
cd "$(dirname "$0")/.."
EWE="${EWE_REPO:-../ewe}"
BIN="${EWE_SYNC_BIN:-src-tauri/target/debug/ewe-sync}"
IMG="${EWE_SYNC_E2E_IMAGE:-docker.io/library/nextcloud:stable}"
skip() { echo "skip: $*"; exit 0; }
command -v podman >/dev/null || skip "no podman"
podman image exists "$IMG" || skip "no $IMG (podman pull $IMG)"
command -v nextcloudcmd >/dev/null || skip "no nextcloudcmd (nextcloud-client)"
command -v dbus-run-session >/dev/null || skip "no dbus-run-session"
[ -x "$BIN" ] || skip "no ewe-sync binary at $BIN (cargo build)"
[ -x "$EWE/bin/ewe-conf" ] || skip "no ewe checkout with bin/ewe-conf (EWE_REPO)"
EWE=$(cd "$EWE" && pwd)
BIN=$(realpath "$BIN")

W=$(mktemp -d)
NAME="ewe-sync-e2e-$$"
PIDS=()
cleanup() {
    # each computer runs in its own session (setsid): its group is ewe-sync,
    # its D-Bus daemon and any nextcloudcmd in flight
    for p in "${PIDS[@]}"; do kill -- "-$p" 2>/dev/null || true; done
    podman rm -f "$NAME" >/dev/null 2>&1 || true
    rm -rf "$W"
}
trap cleanup EXIT
ok() { echo "ok  $*"; }
fail() {
    echo "FAIL  $*"
    for m in a b; do
        echo "--- $m: ewe-sync.json"; head -c 1500 "$W/$m/state/ewe/ewe-sync.json" 2>/dev/null; echo
        echo "--- $m: log"; tail -15 "$W/$m/ewe-sync.log" 2>/dev/null
    done
    exit 1
}
# wait_for <seconds> <description> <command…>
wait_for() {
    local t=$1 what=$2; shift 2
    for _ in $(seq 1 "$t"); do "$@" >/dev/null 2>&1 && return 0; sleep 1; done
    fail "$what (waited ${t}s)"
}

# ── the server ──────────────────────────────────────────────────────────────
PORT=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')
PASS="e2e-$(head -c 12 /dev/urandom | od -An -tx1 | tr -d ' \n')"
podman run -d --name "$NAME" -p "127.0.0.1:$PORT:80" \
    -e SQLITE_DATABASE=nextcloud -e NEXTCLOUD_ADMIN_USER=tester -e NEXTCLOUD_ADMIN_PASSWORD="$PASS" \
    -e NEXTCLOUD_TRUSTED_DOMAINS="127.0.0.1 localhost" "$IMG" >/dev/null
NC="http://127.0.0.1:$PORT"
installed() { curl -fs "$NC/status.php" | grep -q '"installed":true'; }
wait_for 300 "Nextcloud came up" installed
TOKEN=$(curl -fs -u "tester:$PASS" -H 'OCS-APIRequest: true' "$NC/ocs/v2.php/core/getapppassword?format=json" \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["ocs"]["data"]["apppassword"])')
[ -n "$TOKEN" ] || fail "no app password"
DAV="$NC/remote.php/dav/files/tester"
ok "a throwaway Nextcloud on $NC, an app password for tester"

# ── two computers ───────────────────────────────────────────────────────────
mkdir -p "$W/shim"
for t in qs notify-send xdg-open ewe-browser; do printf '#!/bin/sh\nexit 0\n' >"$W/shim/$t"; chmod +x "$W/shim/$t"; done
on() { # on <a|b> <command…> — inside that computer's sandbox
    local d="$W/$1"; shift
    env -u HYPRLAND_INSTANCE_SIGNATURE \
        HOME="$d/home" XDG_CONFIG_HOME="$d/config" XDG_STATE_HOME="$d/state" XDG_CACHE_HOME="$d/cache" \
        XDG_RUNTIME_DIR="$d/run" PATH="$W/shim:$PATH" EWE_REPO="$EWE" \
        EWE_CLOUD_FAKE_KEYRING="$d/keyring" EWE_CLOUD_SERVER_INSECURE=1 \
        EWE_CONF_MACHINE_ID_FILE="$d/machine-id" EWE_FILES_MOUNT="$d/home/Nextcloud" \
        EWE_SYNC_POLL_SECS=4 EWE_SYNC_START_DELAY_SECS=2 "$@"
}
for m in a b; do
    d="$W/$m"
    mkdir -p "$d/home" "$d/config/ewe" "$d/state" "$d/cache" "$d/run"
    chmod 700 "$d/run"
    printf '{"server": "%s", "user": "tester", "login_name": "tester"}\n' "$NC" >"$d/config/ewe/cloud.json"
    printf '%s' "$TOKEN" >"$d/keyring"
    echo "machine-$m-$$-0123456789" >"$d/machine-id"
    printf 'schema = 1\n\n[sync]\nprovider = "nextcloud"\nserver = "%s"\nuser = "tester"\nfolder = "ewe"\nenabled = true\n' \
        "$NC" >"$d/config/ewe/ewe.conf"
done
start() { # start <a|b>
    on "$1" setsid dbus-run-session -- "$BIN" --hidden >"$W/$1/ewe-sync.log" 2>&1 &
    PIDS+=($!)
}
state_of() { # state_of <a|b> → the first folder's state in the snapshot
    python3 -c 'import json,sys; j=json.load(open(sys.argv[1])); print(j["folders"][0]["state"] if j.get("folders") else "")' \
        "$W/$1/state/ewe/sync-folders.json" 2>/dev/null
}
remote_is() { [ "$(curl -fs -u "tester:$TOKEN" "$DAV/$1")" = "$2" ]; }
local_is() { [ "$(cat "$W/$1/home/$2" 2>/dev/null)" = "$3" ]; }
synced() { [ "$(state_of "$1")" = synced ]; }

# 1 · A keeps ~/Docs in the account
on a python3 "$EWE/bin/ewe-conf" set --no-hooks sync.folders '[{"remote": "Docs", "local": "~/Docs"}]' >/dev/null
start a
wait_for 90 "A's first run of ~/Docs" synced a
echo "hello from A" >"$W/a/home/Docs/a.txt"
wait_for 60 "a file written on A reached the account" remote_is Docs/a.txt "hello from A"
ok "A: a file written in ~/Docs went up by itself"

# 2 · B signs in: the folder list comes with the one file, the file comes down
on a python3 "$EWE/bin/ewe-conf" sync --no-hooks >/dev/null
start b
on b python3 "$EWE/bin/ewe-conf" sync --no-hooks >/dev/null
on b python3 "$EWE/bin/ewe-conf" get sync.folders | grep -q '"Docs"' || fail "B did not get A's folder list"
wait_for 90 "the file came down to B" local_is b Docs/a.txt "hello from A"
ok "B: signed in, got the folder list from the account, and the file came down"

# 3 · a change in the account (the web) reaches A — nobody touched A
curl -fs -u "tester:$TOKEN" -T - "$DAV/Docs/web.txt" <<<"from the web" >/dev/null
wait_for 60 "a file added in the account reached A" local_is a Docs/web.txt "from the web"
ok "A: a file added in the account came down by itself (the folder ETag)"

# 4 · a change on B reaches A
wait_for 60 "B settled" synced b
echo "edited on B" >"$W/b/home/Docs/a.txt"
wait_for 90 "B's edit reached A" local_is a Docs/a.txt "edited on B"
ok "A: an edit made on B arrived"

# 5 · what other apps see
wait_for 30 "A settled" synced a
python3 - "$W/a/state/ewe/sync-folders.json" "$W/a/home/Docs" <<'PY' || fail "the snapshot"
import json, sys
j = json.load(open(sys.argv[1]))
assert j["version"] == 1 and j["signed_in"] and j["auto"], j
f = j["folders"][0]
assert f["local"] == sys.argv[2] and f["remote"] == "Docs" and f["engine"] == "ewe", f
assert f["state"] == "synced" and f["last_sync"] > 0 and f["conflicts"] == 0, f
PY
on a "$BIN" --status | python3 -c 'import json,sys; j=json.loads(sys.stdin.read()); assert j["folders"][0]["remote"] == "Docs", j' \
    || fail "ewe-sync --status"
grep -q "Docs" "$W/a/config/gtk-3.0/bookmarks" || fail "no GTK bookmark for ~/Docs"
ok "A: sync-folders.json, ewe-sync --status and the GTK bookmark all name ~/Docs"

echo "ALL PASS"
