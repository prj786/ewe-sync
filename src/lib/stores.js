import { writable, derived } from "svelte/store";

export const route = writable("overview");

/** ewe-cloud status (null = not probed yet) */
export const cloud = writable(null);
/** ewe-conf sync-status (null = not probed yet) */
export const sync = writable(null);
/** "" | "signin" | "sync" | "restore" | "logout" */
export const busy = writable("");
/** login URL of the sign-in in flight ("" = none) */
export const loginUrl = writable("");
/** ewe-mail status (null = not probed yet) */
export const mail = writable(null);
/** ewe-auth status (null = not probed yet) */
export const google = writable(null);
/** {path, exists, valid} for the user's own Google OAuth client */
export const gclient = writable(null);
/** consent URL of the Google connect in flight — kept apart from loginUrl so
 *  the two panes can never show each other's link ("" = none) */
export const consentUrl = writable("");
/** the machine registry */
export const machines = writable([]);
/** the folder runner's snapshot: { pairs: [...], auto, syncing, conflicts,
 *  errors, engines, app: { running, folders }, account_folder, takeover } */
export const folders = writable(null);
/** the ewe browser's sync as ewe-sync reads it: { installed, on, state, last_sync, error } */
export const browser = writable(null);
/** Safe's place and sync state: { installed, set_up, place, state, last } */
export const safe = writable(null);
/** what a restore would bring back: { exists, machine, modified, folders, apps, browser, safe } */
export const preview = writable(null);
/** a folder handed over by `ewe-sync --add-folder` (the file manager's
 *  "Keep in your account…"), waiting for the Folders pane's add form */
export const pendingAdd = writable("");
export const online = writable(true);
/** the account's picture (an <img> src), "" until loaded or signed out */
export const avatar = writable("");

export const signedIn = derived(cloud, ($c) => !!($c && $c.signed_in));

/**
 * The settings-sync conflict in an `ewe-conf sync-status` (or push) reply:
 * "remote-newer" | "remote-exists" | null.
 *
 * `sync-status` never put the guard's verdict in `error` (that is the
 * transport error), so keying on `error` alone meant the Push anyway /
 * Restore banner never showed and a stuck machine had no way out. Newer
 * ewe-conf answers `conflict`; an older one is read from `in_sync`.
 */
export function conflictOf(s) {
  if (!s) return null;
  if (s.error === "remote-newer" || s.error === "remote-exists") return s.error;
  if ("conflict" in s) return s.conflict || null;
  if (s.ok && s.remote && s.in_sync === false) return s.local_synced_at ? "remote-newer" : "remote-exists";
  return null;
}

/** what the tray shows — derived from everything above */
export const trayState = derived([cloud, sync, busy, folders], ([$c, $s, $b, $f]) => {
  if (!$c || !$c.signed_in) return "signed-out";
  if ($c.offline) return "offline";
  if ($b === "sync" || $b === "restore" || $f?.syncing) return "syncing";
  if (conflictOf($s)) return "conflict";
  // a folder conflict or a folder that keeps failing needs the user too
  if ($f?.conflicts || $f?.errors) return "conflict";
  return "idle";
});

// The Toast (design/system/components/Toast): one at a time, a new one
// replaces the current one. 5 s, or 8 s when it carries an action; the
// component pauses the timer while it is hovered or focused. `message` may
// name the thing in **bold**. tone: "info" | "success" | "warning" | "danger"
// ("error", the old name, is danger).
export const currentToast = writable(null);
let toastSeq = 0;
export function toast(message, tone = "info", ms = 0, action = null) {
  const t = tone === "error" ? "danger" : tone;
  const timeout = ms || (action ? 8000 : 5000);
  const id = ++toastSeq;
  currentToast.set({ id, message: String(message), tone: t, action, timeout });
  return id;
}
export function dismissToast(id) {
  currentToast.update((t) => (t && (id == null || t.id === id) ? null : t));
}

/**
 * ISO / epoch seconds / epoch ms → a relative time for recent things, then
 * the date (Writing guide, "Numbers, units and time"): "now", "2 min ago",
 * "3 h ago", "yesterday", "29 Oct" ("29 Oct 2025" in another year). ""
 * when unknown. Lower case: it sits inside sentences ("Up to date · now").
 */
export function fmtTime(v, now = Date.now()) {
  if (!v) return "";
  const d = typeof v === "number" ? new Date(v < 1e12 ? v * 1000 : v) : new Date(v);
  if (Number.isNaN(d.getTime())) return String(v);
  const mins = Math.round((now - d.getTime()) / 60000);
  if (mins < 1) return "now"; // a clock a little ahead counts as now too
  if (mins < 60) return `${mins} min ago`;
  const today = new Date(now);
  const same = (a, b) => a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
  const hours = Math.round(mins / 60);
  if (same(d, today) || hours < 6) return `${hours} h ago`;
  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  if (same(d, yesterday)) return "yesterday";
  const opts = { day: "numeric", month: "short" };
  if (d.getFullYear() !== today.getFullYear()) opts.year = "numeric";
  return d.toLocaleDateString(undefined, opts);
}

// ── plain words for what the engines say ─────────────────────────────────────

/**
 * A folder's last error (the engine's stderr tail, or ewe Sync's own
 * sentence) → { reason, hint, kind } in plain words (Writing guide: "Error
 * codes and technical details go in a details section or the log, never in
 * the headline"). `reason` follows "Couldn’t sync Documents: …"; the raw
 * text stays available behind Details.
 */
export function folderError(raw) {
  const s = String(raw || "");
  const l = s.toLowerCase();
  const has = (...w) => w.some((x) => l.includes(x));
  if (has("insufficient storage", "quota", " 507", "(507", "not enough space in your account"))
    return { kind: "full", reason: "your account is full", hint: "Make room in your account, then try again." };
  if (has("no space left on device"))
    return { kind: "disk", reason: "this computer’s disk is full", hint: "Free up some space here, then try again." };
  if (has("401", "unauthorized", "authentication", "wrong password", "app password", "not signed in"))
    return { kind: "signin", reason: "your account needs you to sign in again", hint: "Sign in on the Account page, then try again." };
  if (has("the nextcloud app is syncing", "nextcloud app already syncs"))
    return { kind: "app", reason: "the Nextcloud app syncs it too", hint: "Two sync apps on one folder get in each other’s way. Let ewe Sync take it over, or quit the Nextcloud app." };
  if (has("can’t sync into itself", "can't sync into itself"))
    return { kind: "mount", reason: "it’s inside your whole account", hint: "Pick a folder outside it, or change this one." };
  if (has("is not installed", "not found in path", "command not found"))
    return { kind: "tool", reason: "a sync tool is missing", hint: "Install it with Komble, then try again. Details name it." };
  if (has("host not found", "connection refused", "network is unreachable", "network unreachable", "timed out", "temporary failure in name resolution", "could not resolve", "no route to host", "can’t be reached"))
    return { kind: "offline", reason: "your server didn’t answer", hint: "It syncs again when the server answers." };
  if (has("permission denied", "operation not permitted", "read-only file system"))
    return { kind: "permission", reason: "some files can’t be read or written here", hint: "Check who owns the folder, then try again. Details name the file." };
  if (has("not found", "404", "doesn’t exist", "does not exist", "isn’t in your account"))
    return { kind: "missing", reason: "the folder isn’t there any more", hint: "Change the folder, or stop syncing it." };
  return { kind: "other", reason: "the sync stopped with an error", hint: "Try again. If it keeps failing, Details has what the sync engine said." };
}

/**
 * A conflict copy the Nextcloud engine left ("notes/plan (conflicted copy
 * 2026-10-31 153000).md", or the older "plan_conflict-20261031-153000.md")
 * → { file: "plan.md", dir: "notes", when: Date|null }. The copy is this
 * computer's version; the original name holds the account's.
 */
export function conflictInfo(path) {
  const p = String(path || "");
  const slash = p.lastIndexOf("/");
  const dir = slash >= 0 ? p.slice(0, slash) : "";
  const name = slash >= 0 ? p.slice(slash + 1) : p;
  let m = name.match(/^(.*) \(conflicted copy (\d{4})-(\d{2})-(\d{2}) (\d{2})(\d{2})(\d{2})[^)]*\)(.*)$/);
  if (m) return { file: m[1] + m[8], dir, when: new Date(+m[2], +m[3] - 1, +m[4], +m[5], +m[6], +m[7]) };
  m = name.match(/^(.*)_conflict-(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})(.*)$/);
  if (m) return { file: m[1] + m[8], dir, when: new Date(+m[2], +m[3] - 1, +m[4], +m[5], +m[6], +m[7]) };
  return { file: name, dir, when: null };
}

/** the opening of a raw message: one line, for a place that has room for one */
export function firstLine(raw) {
  return String(raw || "").split("\n").map((s) => s.trim()).filter(Boolean)[0] || "";
}

// ── one place for problems ───────────────────────────────────────────────────

/**
 * Everything that needs the person, as one list: Overview's "Needs you"
 * alerts, the count on each nav item and the state line under the name in
 * the rail are all this. Offline is a state, not a task: it shows in the
 * rail and on Overview but is not counted. Each item names the pane that
 * resolves it (`route`).
 *   { kind: "conflict", route: "folders", key, name, files }
 *   { kind: "error",    route: "folders", key, name, raw }
 *   { kind: "settings", route: "machine", conflict, machine, mine, when }
 *   { kind: "settings-error", route: "machine", raw }
 *   { kind: "browser",  route: "overview", raw }
 */
export const attention = derived([cloud, sync, folders, browser], ([$c, $s, $f, $b]) => {
  const out = [];
  if (!$c?.signed_in) return out;
  for (const p of $f?.pairs || []) {
    if (p.conflicts?.length) out.push({ kind: "conflict", route: "folders", key: p.key, name: p.name, files: p.conflicts });
    else if (p.state === "error") out.push({ kind: "error", route: "folders", key: p.key, name: p.name, raw: p.last_error });
  }
  const sc = conflictOf($s);
  if (sc)
    out.push({
      kind: "settings", route: "machine", conflict: sc, machine: $s.remote_machine || "",
      mine: $s.remote_is_this_machine ?? null, when: $s.remote_modified || ""
    });
  else if ($s?.error && $s.error !== "nothing-synced" && $s.error !== "not-signed-in")
    out.push({ kind: "settings-error", route: "machine", raw: $s.error });
  if ($b?.installed && $b.on && $b.state === "error") out.push({ kind: "browser", route: "overview", raw: $b.error || "" });
  return out;
});

/** per nav item: { count, tone } — tone danger when anything there failed */
export const navCounts = derived(attention, ($a) => {
  const n = {};
  for (const it of $a) {
    if (it.route === "overview") continue;
    const e = (n[it.route] ||= { count: 0, tone: "warning" });
    e.count++;
    if (it.kind === "error" || it.kind === "settings-error") e.tone = "danger";
  }
  return n;
});

/**
 * The rail's state line under your name, with its tone:
 * { word, tone: "muted" | "warning" | "danger", dot: "" | "away" | "busy" | "offline" }
 */
export const statusLine = derived([cloud, busy, folders, attention], ([$c, $b, $f, $a]) => {
  if (!$c?.signed_in) return { word: "Not signed in", tone: "muted", dot: "offline" };
  if ($c.offline) return { word: "Offline", tone: "muted", dot: "offline" };
  if ($a.length) {
    const decide = $a.every((i) => i.kind === "conflict" || i.kind === "settings");
    const failed = $a.every((i) => i.kind !== "conflict" && i.kind !== "settings");
    const n = $a.length;
    const word = decide
      ? `${n} need${n === 1 ? "s" : ""} a decision`
      : failed
        ? `${n === 1 ? "1 thing" : `${n} things`} couldn’t sync`
        : `${n} things need you`;
    return { word, tone: failed ? "danger" : "warning", dot: failed ? "busy" : "away" };
  }
  if ($b === "sync" || $b === "restore" || $f?.syncing || ($f?.pairs || []).some((p) => p.running))
    return { word: "Syncing…", tone: "muted", dot: "" };
  return { word: "Up to date", tone: "muted", dot: "" };
});

/** Overview → Folders: the folder whose conflict or error to bring into view */
export const focusFolder = writable("");

/** "~/Documents" for a path in the home folder (the snapshot's paths are absolute) */
export function tildePath(p, home = "") {
  const s = String(p || "");
  if (home && (s === home || s.startsWith(home + "/"))) return "~" + s.slice(home.length);
  return s.replace(/^\/home\/[^/]+(?=\/|$)/, "~");
}

export function fmtBytes(n) {
  n = Number(n) || 0;
  const u = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  while (n >= 1000 && i < u.length - 1) {
    n /= 1000;
    i++;
  }
  return `${n < 10 && i > 0 ? n.toFixed(1) : Math.round(n)}\u00a0${u[i]}`;
}
