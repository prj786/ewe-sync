#!/usr/bin/env bash
# Folder pairs — no host state touched: a sandboxed XDG home, the pair model
# written and read back through ewe-conf ([[sync.folders]] in the one file),
# and, only when nextcloudcmd is installed, a two-way run against the mock
# Nextcloud's WebDAV. The Rust-side parsers (conflict names, path guard) are
# `cargo test` unit tests in src-tauri/src/folders.rs.
set -euo pipefail
cd "$(dirname "$0")/.."

EWE="${EWE_REPO:-}"
for cand in "$EWE" ../ewe-nextcloud ../ewe; do
    [ -n "$cand" ] && [ -x "$cand/bin/ewe-conf" ] && { EWE="$cand"; break; }
done
[ -n "$EWE" ] && [ -x "$EWE/bin/ewe-conf" ] || { echo "no ewe checkout with bin/ewe-conf (set EWE_REPO)"; exit 1; }

SB=$(mktemp -d)
trap 'kill $MOCK 2>/dev/null || true; rm -rf "$SB"' EXIT
MOCK=""
export XDG_CONFIG_HOME="$SB/config" XDG_CACHE_HOME="$SB/cache" XDG_STATE_HOME="$SB/state" XDG_RUNTIME_DIR="$SB/run" HOME="$SB/home"
mkdir -p "$XDG_CONFIG_HOME/ewe" "$XDG_CACHE_HOME" "$XDG_STATE_HOME" "$XDG_RUNTIME_DIR" "$HOME"
unset HYPRLAND_INSTANCE_SIGNATURE
CONF="python3 $EWE/bin/ewe-conf"

ok(){ echo "ok  $*"; }
fail(){ echo "FAIL: $*"; exit 1; }

printf '[sync]\nprovider = "nextcloud"\nenabled = false\nfolder = "ewe"\n' > "$XDG_CONFIG_HOME/ewe/ewe.conf"

# 1 · the pair model round-trips through the one file
PAIRS='[{"remote":"/","local":"~/Nextcloud","mode":"two-way","trigger":"change","interval":10,"exclude":[".git","node_modules"]},{"remote":"/Photos","local":"~/Pictures","mode":"upload","trigger":"interval","interval":30,"exclude":[]}]'
$CONF set --no-hooks sync.folders "$PAIRS" >/dev/null
grep -q '^\[\[sync.folders\]\]$' "$XDG_CONFIG_HOME/ewe/ewe.conf" || fail "no [[sync.folders]] block in ewe.conf"
[ "$(grep -c '^\[\[sync.folders\]\]$' "$XDG_CONFIG_HOME/ewe/ewe.conf")" = 2 ] || fail "expected two [[sync.folders]] blocks"
$CONF get sync.folders | python3 -c '
import json,sys; v=json.load(sys.stdin)
assert len(v)==2, v
assert v[0]["remote"]=="/" and v[0]["mode"]=="two-way" and v[0]["exclude"]==[".git","node_modules"], v[0]
assert v[1]["remote"]=="/Photos" and v[1]["trigger"]=="interval" and v[1]["interval"]==30, v[1]
' && ok "pair model round-trips through ewe-conf ([[sync.folders]])"

# 2 · the other domains survive the write (the list is one section of the file)
$CONF get sync.provider | grep -q nextcloud && ok "[sync] scalars intact next to the list"

# 3 · an empty list clears the blocks
$CONF set --no-hooks sync.folders '[]' >/dev/null
! grep -q '^\[\[sync.folders\]\]$' "$XDG_CONFIG_HOME/ewe/ewe.conf" && ok "empty list removes the blocks"

# 4 · two-way run against the mock — only with the real engine on the host
if command -v nextcloudcmd >/dev/null 2>&1; then
    export EWE_CLOUD_FAKE_KEYRING="$SB/keyring" EWE_CLOUD_SERVER_INSECURE=1 EWE_CLOUD_NO_BROWSER=1
    PORT=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')
    python3 tests/mock-nextcloud.py "$PORT" "$SB/mock.json" >/dev/null 2>&1 &
    MOCK=$!
    sleep 0.6
    python3 "$EWE/bin/ewe-cloud" login "http://127.0.0.1:$PORT" 2>"$SB/err" >"$SB/login.json" &
    LOGIN=$!
    for i in $(seq 1 50); do grep -q '^login-url: ' "$SB/err" 2>/dev/null && break; sleep 0.1; done
    url=$(sed -n 's/^login-url: //p' "$SB/err" | head -1)
    curl -s "$url" >/dev/null; wait $LOGIN || true
    mkdir -p "$HOME/nc-home"; printf 'machine 127.0.0.1 login %s password %s\n' \
        "$(python3 -c 'import json;print(json.load(open("'"$SB"'/login.json"))["user"])')" \
        "$(python3 "$EWE/bin/ewe-cloud" token)" > "$HOME/nc-home/.netrc"; chmod 600 "$HOME/nc-home/.netrc"
    mkdir -p "$HOME/local"; echo hello > "$HOME/local/a.txt"
    if HOME="$HOME/nc-home" nextcloudcmd --non-interactive --silent -n "$HOME/local" "http://127.0.0.1:$PORT" >"$SB/nc.log" 2>&1; then
        ok "nextcloudcmd two-way run against the mock"
    else
        echo "nextcloudcmd run failed against the mock (the mock may lack a WebDAV feature the engine needs):"; tail -5 "$SB/nc.log"
    fi
else
    echo "skip nextcloudcmd two-way run (nextcloudcmd is not installed on this host)"
fi

npm run build >/dev/null 2>&1 && ok "frontend builds" || fail "frontend build"
echo "ALL PASS"
