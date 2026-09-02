#!/usr/bin/env bash
# Smoke test — no host state touched: a sandboxed XDG home, the mock
# Nextcloud on a loopback port, ewe-cloud (from $EWE_REPO or a sibling
# ewe checkout) signed in against it, then `ewe-sync --check` if a built
# binary exists. Frontend build always.
set -euo pipefail
cd "$(dirname "$0")/.."

EWE="${EWE_REPO:-}"
for cand in "$EWE" ../ewe-nextcloud ../ewe; do
    [ -n "$cand" ] && [ -x "$cand/bin/ewe-cloud" ] && { EWE="$cand"; break; }
done
[ -n "$EWE" ] && [ -x "$EWE/bin/ewe-cloud" ] || { echo "no ewe checkout with bin/ewe-cloud (set EWE_REPO)"; exit 1; }

SB=$(mktemp -d)
trap 'kill $MOCK 2>/dev/null || true; rm -rf "$SB"' EXIT
export XDG_CONFIG_HOME="$SB/config" XDG_CACHE_HOME="$SB/cache" XDG_STATE_HOME="$SB/state" XDG_RUNTIME_DIR="$SB/run" HOME="$SB/home"
mkdir -p "$XDG_CONFIG_HOME/ewe" "$XDG_CACHE_HOME" "$XDG_STATE_HOME" "$XDG_RUNTIME_DIR" "$HOME"
export EWE_CLOUD_FAKE_KEYRING="$SB/keyring" EWE_CLOUD_SERVER_INSECURE=1 EWE_CLOUD_NO_BROWSER=1 EWE_REPO="$EWE"

PORT=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')
python3 tests/mock-nextcloud.py "$PORT" "$SB/mock.json" >/dev/null 2>&1 &
MOCK=$!
sleep 0.6

ok(){ echo "ok  $*"; }
fail(){ echo "FAIL: $*"; exit 1; }

# sign in: the login URL comes on stderr; "the person" completes it with a GET
python3 "$EWE/bin/ewe-cloud" login "http://127.0.0.1:$PORT" 2>"$SB/err" >"$SB/login.json" &
LOGIN=$!
for i in $(seq 1 50); do grep -q '^login-url: ' "$SB/err" 2>/dev/null && break; sleep 0.1; done
url=$(sed -n 's/^login-url: //p' "$SB/err" | head -1)
[ -n "$url" ] || fail "no login-url from ewe-cloud"
curl -s "$url" >/dev/null || python3 -c "import urllib.request; urllib.request.urlopen('$url').read()"
wait $LOGIN || true
python3 -c 'import json,sys; j=json.load(open(sys.argv[1])); assert j["ok"], j; print("signed in as", j["user"])' "$SB/login.json" && ok "ewe-cloud login against the mock"

# the one file: a minimal ewe.conf with the nextcloud provider, then push
printf '[sync]\nprovider = "nextcloud"\nenabled = true\nfolder = "ewe"\n' > "$XDG_CONFIG_HOME/ewe/ewe.conf"
python3 "$EWE/bin/ewe-conf" push | python3 -c 'import json,sys; j=json.load(sys.stdin); assert j["ok"], j' && ok "ewe-conf push"
python3 "$EWE/bin/ewe-conf" sync-status | python3 -c 'import json,sys; j=json.load(sys.stdin); assert j.get("provider")=="nextcloud", j' && ok "ewe-conf sync-status"

# the app's self-check, when a binary exists
BIN=""
for b in src-tauri/target/release/ewe-sync src-tauri/target/debug/ewe-sync "$(command -v ewe-sync || true)"; do
    [ -n "$b" ] && [ -x "$b" ] && { BIN="$b"; break; }
done
if [ -n "$BIN" ]; then
    "$BIN" --check > "$SB/check.json" || true
    python3 -c '
import json,sys; j=json.load(open(sys.argv[1]))
assert j["ok"], j["tools"]
assert j["cloud"]["signed_in"], j["cloud"]
assert j["sync"].get("provider")=="nextcloud", j["sync"]
print("tools:", [t["path"] for t in j["tools"]])' "$SB/check.json" && ok "ewe-sync --check"
else
    echo "skip ewe-sync --check (no built binary — cargo is not on this host)"
fi

npm run build >/dev/null 2>&1 && ok "frontend builds" || fail "frontend build"
echo "ALL PASS"
