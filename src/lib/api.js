import { invoke as tauriInvoke, convertFileSrc } from "@tauri-apps/api/core";

// Outside Tauri (a plain browser with `?mock=1`) src/lib/devmock.js answers.
const invoke = (cmd, args) =>
  window.__EWE_SYNC_MOCK__ ? window.__EWE_SYNC_MOCK__.invoke(cmd, args) : tauriInvoke(cmd, args);
/** a local file path → something an <img> can load (data URIs pass through) */
export const fileSrc = (p) => (window.__EWE_SYNC_MOCK__ || String(p).startsWith("data:") ? p : convertFileSrc(p));

export const dePrefs = () => invoke("de_prefs");

// account (ewe-cloud)
export const cloudStatus = () => invoke("cloud_status");
export const cloudLogin = (server) => invoke("cloud_login", { server });
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

// tray
export const trayState = (state, tooltip) => invoke("tray_state", { state, tooltip: tooltip || null });
export const trayPauseLabel = (paused) => invoke("tray_pause_label", { paused });
