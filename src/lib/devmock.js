// Browser-only stand-in for the Tauri backend, so the UI can be opened (and
// screenshotted) with `npm run dev` (or a build + any static server):
// `?mock=1`, and `&state=out` for the signed-out account. Never loaded
// inside Tauri. Screenshot parameters, as in ewe-settings' dev-mock.html:
//   scheme=ewe-dark|ewe-light|a11y   which theme_tokens reply to serve — real
//                                    `ewe-theme show` output in .dev-mock/
//                                    (git-ignored); without it tokens.css stands
//   still                            no transitions or animations (headless
//                                    screenshots freeze them mid-way)
//   pane=<nav label>                 open that section ("Folders")
//   click=<css selector>             click that element
//   focus=<css selector>             keyboard-focus that element
//   conflict=newer|mine|exists       a settings-sync conflict (another machine /
//                                    this computer's own unrecorded copy /
//                                    never synced + a backup exists)
//   scene=new                        a new computer, the account has a backup
//                                    (Overview's "Welcome back")
//   scene=first                      the first computer: no backup, no folders
//   app=1                            the Nextcloud desktop app syncs ~/Nextcloud
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
  const scene = params.get("scene") || "";
  if (scene === "new") params.set("conflict", "exists");
  const now = Math.floor(Date.now() / 1000);
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
    // conflict=newer|mine|exists: the settings-sync conflict states
    sync: {
      ok: true, provider: "nextcloud", server, folder: "ewe", enabled: true, error: null,
      remote: { id: '"etag"', modifiedTime: iso(120), appProperties: { machine: "emoh" } },
      remote_machine: params.get("conflict") === "newer" ? "work-pc" : "emoh",
      remote_modified: iso(120), recorded_remote_modified: iso(120),
      remote_is_this_machine: params.get("conflict") === "mine" ? true : params.get("conflict") === "newer" ? false : null,
      local_synced_at: params.get("conflict") === "exists" || scene === "first" ? "" : iso(10),
      local_mtime: Math.floor(Date.now() / 1000) - 600,
      conflict: { newer: "remote-newer", mine: "remote-newer", exists: "remote-exists" }[params.get("conflict")] || null,
      in_sync: !params.get("conflict")
    },
    folders: {
      pairs:
        scene === "new" || scene === "first" || params.get("app") === "1"
          ? []
          : [
              {
                key: "Documents|two-way", name: "Documents", state: "synced", paused: false, offline: false,
                pair: { remote: "Documents", local: "~/Documents", mode: "two-way", trigger: "change", interval: 10, exclude: [] },
                local_effective: "/home/scubba/Documents", running: false,
                last_run: now - 120, last_ok: true, last_error: "", conflicts: []
              },
              {
                key: "Projects|two-way", name: "Projects", state: "conflict", paused: false, offline: false,
                pair: { remote: "Projects", local: "~/Projects", mode: "two-way", trigger: "change", interval: 10, exclude: ["node_modules", "target"] },
                local_effective: "/home/scubba/Projects", running: false,
                last_run: now - 300, last_ok: true, last_error: "",
                conflicts: ["notes/plan (conflicted copy 2026-10-31 153000).md"]
              },
              {
                key: "Photos|upload", name: "Pictures", state: "paused", paused: true, offline: false,
                pair: { remote: "Photos", local: "~/Pictures", mode: "upload", trigger: "change", interval: 30, exclude: [] },
                local_effective: "/home/scubba/Pictures", running: false,
                last_run: now - 86400, last_ok: true, last_error: "", conflicts: []
              }
            ],
      auto: true, syncing: false, conflicts: 1, errors: 0, engines: { nextcloudcmd: true, rclone: true },
      app: params.get("app") === "1"
        ? { running: true, folders: [{ local: "/home/scubba/Nextcloud", remote: "", paused: false, name: "Nextcloud" }] }
        : { running: false, folders: [] },
      account_folder: { path: "/home/scubba/Nextcloud", state: params.get("app") === "1" ? "synced" : "mounted" },
      takeover: null
    },
    suggestions: [
      { kind: "desktop", name: "Desktop", local: "~/Desktop", remote: "Desktop", in_account: false, remote_size: null, exclude: [] },
      { kind: "documents", name: "Documents", local: "~/Documents", remote: "Documents", in_account: true, remote_size: 314572800, exclude: [] },
      { kind: "pictures", name: "Pictures", local: "~/Pictures", remote: "Pictures", in_account: false, remote_size: null, exclude: [] },
      { kind: "music", name: "Music", local: "~/Music", remote: "Music", in_account: false, remote_size: null, exclude: [] }
    ],
    machines: [
      { name: "emoh", last_seen: iso(120), ewe_version: "0.9.23", apps_count: 24 },
      { name: "ewe-vm", last_seen: iso(60 * 26), ewe_version: "0.9.22", apps_count: 6 }
    ]
  };
  if (scene === "first") Object.assign(state.sync, { remote: null, remote_machine: "", remote_modified: "", conflict: null, in_sync: false });
  const scheme = params.get("scheme") || "ewe-dark";
  const handlers = {
    theme_tokens: () =>
      fetch(`/.dev-mock/theme-${scheme}.json`).then((r) => (r.ok ? r.json() : Promise.reject(new Error("no mock theme")))),
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
        name: String(p.local).replace(/\/+$/, "").split("/").pop(), state: "syncing", paused: false, offline: false,
        local_effective: String(p.local).replace(/^~/, "/home/scubba"), running: true,
        last_run: 0, last_ok: false, last_error: "", conflicts: []
      }));
      state.folders.conflicts = 0;
      return state.folders;
    },
    folders_run: () => ({ ok: true }),
    folders_run_all: () => ({ ok: true, failed: [] }),
    folders_local_override: () => state.folders,
    folders_pause: ({ key, paused }) => {
      const it = state.folders.pairs.find((p) => p.key === key);
      if (it) Object.assign(it, { paused, state: paused ? "paused" : "synced" });
      return state.folders;
    },
    folders_takeover: () => {
      state.folders.app = { running: false, folders: [] };
      state.folders.pairs = [{
        key: "|two-way", name: "Nextcloud", state: "syncing", paused: false, offline: false,
        pair: { remote: "", local: "~/Nextcloud", mode: "two-way", trigger: "change", interval: 10, exclude: [], hidden: true },
        local_effective: "/home/scubba/Nextcloud", running: true, last_run: 0, last_ok: false, last_error: "", conflicts: []
      }];
      return state.folders;
    },
    folders_suggest: () => ({
      items: state.suggestions.filter((s) => !state.folders.pairs.some((p) => p.pair.local === s.local))
    }),
    folder_size: ({ path }) => ({ bytes: { "~/Desktop": 52428800, "~/Documents": 1288490188, "~/Pictures": 4294967296, "~/Music": 9663676416 }[path] || 1048576, files: 812, more: path === "~/Music" }),
    folder_open: () => null,
    remote_list: ({ path }) => ({
      path: path || "",
      size: 1288490188,
      free: path ? null : 4080218931,
      used: 1288490188,
      items: path
        ? [{ name: "2026", path: `${path}/2026`, dir: true, size: 734003200 }, { name: "cover.png", path: `${path}/cover.png`, dir: false, size: 204800 }]
        : [
            { name: "Documents", path: "Documents", dir: true, size: 314572800 },
            { name: "ewe", path: "ewe", dir: true, size: 409600 },
            { name: "Photos", path: "Photos", dir: true, size: 943718400 },
            { name: "Readme.md", path: "Readme.md", dir: false, size: 1024 }
          ]
    }),
    take_deep_link: () => (params.get("add") ? { pane: "folders", add_folder: params.get("add") } : null),
    browser_status: () => ({ installed: true, on: true, state: "idle", last_sync: Date.now() - 240000, error: null }),
    safe_status: () => ({ installed: true, answered: true, set_up: true, place: "nextcloud", state: "in-sync", last: iso(30) }),
    open_safe: () => null,
    sync_everything: () => ({ ok: true, action: "none" }),
    restore_preview: () => ({
      exists: true, machine: "emoh", mine: false, modified: iso(60 * 5),
      folders: [
        { name: "Documents", local: "~/Documents", remote: "Documents", mode: "two-way", size: 314572800 },
        { name: "Projects", local: "~/Projects", remote: "Projects", mode: "two-way", size: 1825361100 }
      ],
      apps: 24, browser: true, safe: true, scheme: "ewe-dark"
    }),
    restore_everything: () => ({ ok: true, apps: 24, folders: 2, machine: "emoh" }),
    after_sign_in: () => null,
    after_sign_out: () => null,
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
  if (params.has("still")) {
    const st = document.createElement("style");
    st.textContent = "*,*::before,*::after{transition:none!important;animation:none!important}";
    document.head.appendChild(st);
  }
  // install() runs before the app mounts (main.js), so the timers start from
  // there rather than from "load", which may already have fired
  {
    const pane = params.get("pane");
    setTimeout(() => {
      if (pane) document.querySelector(`.ewe-navitem[title="${pane}"]`)?.click();
      setTimeout(() => {
        if (params.get("click")) document.querySelector(params.get("click"))?.click();
        if (params.get("focus")) {
          const el = document.querySelector(params.get("focus"));
          // a real keyboard focus, so :focus-visible draws the ring
          el?.focus({ focusVisible: true });
        }
      }, 400);
    }, 300);
  }
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
