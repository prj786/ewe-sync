import { writable, derived } from "svelte/store";

export const route = writable("account");

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
/** the folder runner's snapshot: { pairs: [...], syncing, conflicts, engines } */
export const folders = writable(null);
export const online = writable(true);

export const signedIn = derived(cloud, ($c) => !!($c && $c.signed_in));

/** what the tray shows — derived from everything above */
export const trayState = derived([cloud, sync, busy, folders], ([$c, $s, $b, $f]) => {
  if (!$c || !$c.signed_in) return "signed-out";
  if ($c.offline) return "offline";
  if ($b === "sync" || $b === "restore" || $f?.syncing) return "syncing";
  if ($s && ($s.error === "remote-newer" || $s.error === "remote-exists")) return "conflict";
  if ($f?.conflicts) return "conflict";
  return "idle";
});

export const toasts = writable([]);
let toastId = 0;
export function toast(message, type = "info", ms) {
  const id = ++toastId;
  const timeout = ms ?? (type === "error" ? 8000 : 4000);
  toasts.update((t) => [...t, { id, message: String(message), type }]);
  setTimeout(() => dismissToast(id), timeout);
  return id;
}
export function dismissToast(id) {
  toasts.update((t) => t.filter((x) => x.id !== id));
}

/** ISO/epoch → "2 Sep, 15:04" (local), "" when unknown */
export function fmtTime(v) {
  if (!v) return "";
  const d = typeof v === "number" ? new Date(v * 1000) : new Date(v);
  if (Number.isNaN(d.getTime())) return String(v);
  return d.toLocaleString(undefined, { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
}

export function fmtBytes(n) {
  n = Number(n) || 0;
  const u = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  while (n >= 1024 && i < u.length - 1) {
    n /= 1024;
    i++;
  }
  return `${n < 10 && i > 0 ? n.toFixed(1) : Math.round(n)} ${u[i]}`;
}
