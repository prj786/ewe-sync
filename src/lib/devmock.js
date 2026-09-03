// Browser-only stand-in for the Tauri backend, so the UI can be opened (and
// screenshotted) with `npm run build` + any static server: `?mock=1`, and
// `&state=out` for the signed-out account. Never loaded inside Tauri.
function iso(minutesAgo) {
  return new Date(Date.now() - minutesAgo * 60000).toISOString();
}
function avatarDataUri() {
  const svg =
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96">' +
    '<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#8fbce0"/><stop offset="1" stop-color="#3d6d99"/></linearGradient></defs>' +
    '<circle cx="48" cy="48" r="48" fill="url(#g)"/>' +
    '<text x="48" y="60" font-family="sans-serif" font-size="40" font-weight="600" fill="#fff" text-anchor="middle">S</text></svg>';
  return "data:image/svg+xml;utf8," + encodeURIComponent(svg);
}

export function install(params) {
  const signedOut = params.get("state") === "out";
  const server = "https://ivo.lv.tab.digital";
  const state = {
    cloud: signedOut
      ? { ok: true, signed_in: false, keyring: true, keyring_state: "ok", keyring_locked: false }
      : {
          ok: true, signed_in: true, offline: false, server, user: "scubba", login_name: "scubba",
          display_name: "Scubba", email: "scubba@tab.digital",
          quota: { used: 1288490188, total: 5368709120, relative: 24 },
          keyring: true, keyring_state: "ok", keyring_locked: false
        },
    sync: {
      ok: true, provider: "nextcloud", server, folder: "ewe", enabled: true,
      remote_machine: "emoh", remote_modified: iso(120), recorded_remote_modified: iso(120),
      local_synced_at: iso(10), local_mtime: Math.floor(Date.now() / 1000) - 600, in_sync: true
    },
    folders: {
      pairs: [
        {
          key: "|two-way",
          pair: { remote: "/", local: "~/Nextcloud", mode: "two-way", trigger: "change", interval: 10, exclude: [".git"] },
          local_effective: "/home/scubba/Nextcloud", running: false,
          last_run: Math.floor(Date.now() / 1000) - 300, last_ok: true, last_error: "",
          conflicts: ["Documents/notes (conflicted copy 2026-09-02 153000).md"]
        },
        {
          key: "Photos|upload",
          pair: { remote: "/Photos", local: "~/Pictures", mode: "upload", trigger: "interval", interval: 30, exclude: [] },
          local_effective: "/home/scubba/Pictures", running: false,
          last_run: Math.floor(Date.now() / 1000) - 1800, last_ok: true, last_error: "", conflicts: []
        }
      ],
      syncing: false, conflicts: 1, engines: { nextcloudcmd: true, rclone: true }
    },
    machines: [
      { name: "emoh", last_seen: iso(120), ewe_version: "0.9.23", apps_count: 24 },
      { name: "ewe-vm", last_seen: iso(60 * 26), ewe_version: "0.9.22", apps_count: 6 }
    ]
  };
  const handlers = {
    de_prefs: () => ({
      accent: params.get("accent") || (params.get("theme") === "flock" ? "#ffcc00" : "#b1c5ff"),
      themeName: params.get("theme") === "flock" ? "flock" : "blacksheep",
      colorScheme: "dark"
    }),
    cloud_status: () => state.cloud,
    cloud_login: () => ({ ok: true, ...state.cloud }),
    cloud_logout: () => ({ ok: true, revoked: true }),
    cloud_avatar: () => ({ ok: true, path: avatarDataUri() }),
    keyring_reset: () => ({ ok: true, moved: "", relogin: true }),
    session_logout: () => null,
    conf_sync_status: () => state.sync,
    conf_push: () => ({ ok: true, machine: "emoh", modified: iso(0), local_synced_at: iso(0) }),
    conf_restore: () => ({ ok: true, apps: 3 }),
    conf_get_cmd: ({ key }) => (key === "sync.enabled" ? true : key === "sync.folders" ? [] : null),
    conf_set_cmd: () => null,
    shell_poke_cmd: () => null,
    machines_list: () => state.machines,
    machines_write: () => null,
    this_machine: () => ({ name: "emoh", ewe_version: "0.9.23", apps_count: 24 }),
    folders_list: () => state.folders,
    folders_set: ({ pairs }) => {
      state.folders.pairs = pairs.map((p) => ({
        key: `${String(p.remote || "").replace(/^\/+|\/+$/g, "")}|${p.mode}`, pair: p,
        local_effective: String(p.local).replace(/^~/, "/home/scubba"), running: false,
        last_run: 0, last_ok: false, last_error: "", conflicts: []
      }));
      state.folders.conflicts = 0;
      return state.folders;
    },
    folders_run: () => ({ ok: true }),
    folders_run_all: () => ({ ok: true, failed: [] }),
    folders_local_override: () => state.folders,
    folders_resolve: ({ key }) => {
      const it = state.folders.pairs.find((p) => p.key === key);
      if (it) it.conflicts = [];
      state.folders.conflicts = 0;
      return state.folders;
    },
    tray_state: () => null,
    tray_pause_label: () => null,
    // the other accounts (RFC-005): `&mail=out` / `&google=out` show the
    // empty states, which are the ones worth screenshotting
    mail_status: () =>
      params.get("mail") === "out"
        ? { ok: true, configured: false }
        : { ok: true, configured: true, host: "imap.tab.digital", user: "scubba@tab.digital", port: 993, keyring: true, keyring_state: "ok" },
    mail_login: ({ host, user, port }) => ({ ok: true, host, user, port }),
    mail_logout: () => ({ ok: true }),
    mail_unseen: () => ({
      ok: true,
      unread: 2,
      list: [
        { id: "1", from: "Nextcloud <no-reply@tab.digital>", subject: "Your app password was used", unread: true },
        { id: "2", from: "Arch <security@archlinux.org>", subject: "[arch-security] Advisory ASA-202609-1", unread: true }
      ]
    }),
    google_client_info: () =>
      params.get("google") === "out"
        ? { path: "/home/scubba/.config/ewe/oauth-client.json", exists: false, valid: false }
        : { path: "/home/scubba/.config/ewe/oauth-client.json", exists: true, valid: true },
    google_status: () =>
      params.get("google") === "out"
        ? { ok: true, configured: false, signed_in: false }
        : {
            ok: true, configured: true, signed_in: true, mail_state: "ok", mail_unread: 3,
            profile: { name: "Scubba", email: "macharashvili786@gmail.com" }
          },
    google_login: () => ({ ok: true }),
    google_logout: () => ({ ok: true })
  };
  window.__EWE_SYNC_MOCK__ = {
    invoke: (cmd, args) =>
      new Promise((res, rej) => {
        const h = handlers[cmd];
        setTimeout(() => (h ? res(h(args || {})) : rej(new Error("mock: unknown command " + cmd))), 30);
      }),
    listen: () => Promise.resolve(() => {}),
    emit: () => Promise.resolve()
  };
}
