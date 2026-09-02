import { invoke } from "@tauri-apps/api/core";

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

// tray
export const trayState = (state, tooltip) => invoke("tray_state", { state, tooltip: tooltip || null });
export const trayPauseLabel = (paused) => invoke("tray_pause_label", { paused });
