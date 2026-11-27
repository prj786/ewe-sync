import { invoke as tauriInvoke, convertFileSrc } from "@tauri-apps/api/core";

// Outside Tauri (a plain browser with `?mock=1`) src/lib/devmock.js answers.
const invoke = (cmd, args) =>
  window.__EWE_SYNC_MOCK__ ? window.__EWE_SYNC_MOCK__.invoke(cmd, args) : tauriInvoke(cmd, args);
/** a local file path → something an <img> can load (data URIs pass through) */
export const fileSrc = (p) => (window.__EWE_SYNC_MOCK__ || String(p).startsWith("data:") ? p : convertFileSrc(p));

export const dePrefs = () => invoke("de_prefs");
/** the generated look, from ewe-theme.conf — see applyDePrefs in App.svelte */
export const themeTokens = (theme) => invoke("theme_tokens", { theme });

// account (ewe-cloud)
export const cloudStatus = () => invoke("cloud_status");
// no user/password = the browser flow; with both, the password goes to ewe-cloud on stdin
export const cloudLogin = (server, user, password) =>
  invoke("cloud_login", user && password ? { server, user, password } : { server });
export const cloudLogout = () => invoke("cloud_logout");
export const cloudAvatar = () => invoke("cloud_avatar");
export const keyringReset = () => invoke("keyring_reset");
export const sessionLogout = () => invoke("session_logout");

// the one file (ewe-conf)
export const syncStatus = () => invoke("conf_sync_status");
export const push = (force = false) => invoke("conf_push", { force });
export const restore = () => invoke("conf_restore");
export const confGet = (key) => invoke("conf_get_cmd", { key });
export const confSet = (key, value) => invoke("conf_set_cmd", { key, value });
export const shellPoke = (verb) => invoke("shell_poke_cmd", { verb });

// mail (ewe-mail) — any number of IMAP/SMTP mailboxes; passwords go on stdin, never argv.
// `extra` (snake_case, as the CLI names them): email, name, smtp_host, smtp_port,
// smtp_security, smtp_user, smtp_password (only when the outgoing password differs).
export const mailStatus = () => invoke("mail_status");
export const mailAccounts = () => invoke("mail_accounts");
export const mailAutoconfig = (email) => invoke("mail_autoconfig", { email });
export const mailLogin = (host, port, user, password, starttls = false, extra = null) =>
  invoke("mail_login", extra ? { host, port, user, password, starttls, extra } : { host, port, user, password, starttls });
/** one account by id ("imap:me@example.org"); no id = the only one */
export const mailLogout = (id = null) => invoke("mail_logout", id ? { id } : {});
export const mailSetName = (id, name) => invoke("mail_set_name", { id, name });
export const mailUnseen = () => invoke("mail_unseen");

// google (ewe-auth) — the optional extra; ewe ships no client of its own
export const googleClientInfo = () => invoke("google_client_info");
export const googleStatus = () => invoke("google_status");
export const googleLogin = () => invoke("google_login");
export const googleLogout = () => invoke("google_logout");

// machines
export const machinesList = () => invoke("machines_list");
export const machinesWrite = () => invoke("machines_write");
export const thisMachine = () => invoke("this_machine");

// folders (the runner)
export const foldersList = () => invoke("folders_list");
export const foldersSet = (pairs) => invoke("folders_set", { pairs });
export const foldersRun = (key) => invoke("folders_run", { key });
export const foldersRunAll = () => invoke("folders_run_all");
export const foldersLocalOverride = (key, local) => invoke("folders_local_override", { key, local });
export const foldersResolve = (key, path, keep) => invoke("folders_resolve", { key, path, keep });
export const foldersPause = (key, paused) => invoke("folders_pause", { key, paused });
export const foldersTakeover = () => invoke("folders_takeover");
export const foldersSuggest = () => invoke("folders_suggest");
export const folderSize = (path) => invoke("folder_size", { path });
export const folderOpen = (path) => invoke("folder_open", { path });
export const remoteList = (path) => invoke("remote_list", { path });

// everything the account carries (overview.rs)
export const takeDeepLink = () => invoke("take_deep_link");
export const browserStatus = () => invoke("browser_status");
export const safeStatus = () => invoke("safe_status");
export const openSafe = () => invoke("open_safe");
export const syncEverything = () => invoke("sync_everything");
export const restorePreview = () => invoke("restore_preview");
export const restoreEverything = () => invoke("restore_everything");
export const afterSignIn = () => invoke("after_sign_in");
export const afterSignOut = () => invoke("after_sign_out");
