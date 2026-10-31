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

/** ISO/epoch → "2 Sep, 15:04" (local), "" when unknown */
export function fmtTime(v) {
  if (!v) return "";
  const d = typeof v === "number" ? new Date(v * 1000) : new Date(v);
  if (Number.isNaN(d.getTime())) return String(v);
  return d.toLocaleString(undefined, { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
}

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
