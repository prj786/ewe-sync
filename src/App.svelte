<script>
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { listen as tauriListen } from "@tauri-apps/api/event";
  // in a plain browser (devmock) there is no event bus — subscribe to nothing
  const listen = (ev, cb) => (window.__EWE_SYNC_MOCK__ ? Promise.resolve(() => {}) : tauriListen(ev, cb));
  import * as api from "./lib/api";
  import {
    route, cloud, sync, busy, loginUrl, folders, toast, mail, google, gclient, consentUrl,
    browser, safe, pendingAdd, avatar
  } from "./lib/stores";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import Overview from "./lib/components/Overview.svelte";
  import Account from "./lib/components/Account.svelte";
  import Mail from "./lib/components/Mail.svelte";
  import Google from "./lib/components/Google.svelte";
  import ThisMachine from "./lib/components/ThisMachine.svelte";
  import Machines from "./lib/components/Machines.svelte";
  import Folders from "./lib/components/Folders.svelte";
  import Toasts from "./lib/components/Toasts.svelte";
  import { watchTheme } from "./lib/theme.js";

  // The look, live (lib/theme.js): `ewe-theme show` at start, on focus, and
  // whenever the DE's user-theme.json changes — its whole content is the
  // key, so a scheme, light/dark, accent, look preset or accessibility mode
  // picked in Settings reaches this window while it sits in another tile.
  const themeKey = async () => {
    const p = await api.dePrefs();
    return JSON.stringify(p?.raw ?? p ?? null);
  };

  // the sections (Side navigation); Ctrl+1 … Ctrl+7 jump to them. What the
  // account keeps comes first (Overview, Folders, Settings & apps,
  // Computers); the account itself and the other accounts after. The ids are
  // `ewe-sync --pane <id>` (public: the shell and ewe Settings open them), so
  // they never change with the labels — "machines" is the Computers page.
  const NAV = [
    { id: "overview", label: "Overview", icon: "home" },
    { id: "folders", label: "Folders", icon: "folder" },
    { id: "machine", label: "Settings & apps", icon: "sliders" },
    { id: "machines", label: "Computers", icon: "laptop" },
    { id: "account", label: "Account", icon: "user" },
    { id: "mail", label: "Mail", icon: "mail" },
    { id: "google", label: "Google", icon: "globe" }
  ];
  function globalKey(e) {
    if (e.ctrlKey && !e.altKey && !e.shiftKey && /^[1-9]$/.test(e.key)) {
      const it = NAV[Number(e.key) - 1];
      if (it) {
        e.preventDefault();
        route.set(it.id);
      }
    }
  }

  // This page exists only while the window is on screen: `ewe-sync --hidden`
  // (the login unit) starts with no window, close destroys it, and the bar's
  // sync glyph is fed from Rust (src-tauri/src/bar.rs) — so everything here
  // is for someone reading it, every 30 s and on focus. (Until 0.16.2 a
  // hidden page kept polling for the glyph; until 0.17 it kept a WebKit
  // process alive all session for nothing else.)

  /** the account + the one file, in one round, then what else the account
   *  carries and the other accounts */
  export async function refresh() {
    try {
      cloud.set(await api.cloudStatus());
    } catch (e) {
      cloud.set({ ok: false, signed_in: false, error: String(e) });
    }
    if (get(cloud)?.signed_in) {
      try {
        sync.set(await api.syncStatus());
      } catch (e) {
        sync.set({ ok: false, error: String(e) });
      }
      // what else the account carries: the browser and Safe report their own
      api.browserStatus().then((b) => browser.set(b)).catch(() => browser.set(null));
      api.safeStatus().then((s) => safe.set(s)).catch(() => safe.set(null));
    } else {
      sync.set(null);
    }
    // the other accounts (RFC-005): each answers with one JSON object and
    // never throws for "not configured", so a missing tool is the only catch
    try {
      mail.set(await api.mailStatus());
    } catch {
      mail.set(null);
    }
    try {
      gclient.set(await api.googleClientInfo());
    } catch {
      gclient.set(null);
    }
    try {
      google.set(await api.googleStatus());
    } catch {
      google.set(null);
    }
  }

  // Sync now means everything: settings & apps (pull, merge, push), every
  // folder, ewe Browser and ewe Safe (overview.rs sync_everything)
  async function syncNow() {
    if (get(busy)) return;
    busy.set("sync");
    try {
      const r = await api.syncEverything();
      if (r.ok) toast(r.changed?.length ? "Synced. Changes from your other computers are here." : "Everything is up to date", "success");
      else if (r.error === "remote-newer" || r.error === "remote-exists") {
        // the decision (whose settings to keep) lives on Settings & apps
        route.set("machine");
        toast(
          r.error === "remote-exists"
            ? "Your account already has settings. Choose which ones this computer keeps."
            : r.remote_is_this_machine
              ? "Your account has a different copy of these settings. Choose which one to keep."
              : "Two computers changed your settings. Choose which ones to keep.",
          "warning"
        );
      } else toast(r.message || r.error || "Couldn’t sync. Try again.", "danger");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy.set("");
      refresh();
    }
  }

  const ROUTES = ["overview", "account", "mail", "google", "machine", "machines", "folders"];
  // the pages' names since 0.16, accepted beside the ids
  const ALIAS = { computers: "machines", settings: "machine" };
  const toRoute = (id) => (ROUTES.includes(id) ? id : ALIAS[id] || "");

  // `ewe-sync --pane <id>` / `--add-folder <path>` (the shell, ewe Settings,
  // the file manager's "Keep in your account…"). Rust stores the link and
  // says "deep-link"; the page takes it — on mount (the window was just
  // built for it) or on the event (it was already open), never both.
  function openLink(l) {
    if (!l) return;
    if (l.add_folder) pendingAdd.set(String(l.add_folder));
    if (l.pane && toRoute(l.pane)) route.set(toRoute(l.pane));
  }
  const takeLink = () => api.takeDeepLink().then(openLink).catch(() => {});
  onMount(() => {
    const unlisteners = [];
    // #overview / #account / #mail / #google / #machine / #machines / #folders
    // open a pane directly (the dev mock; inside Tauri the --pane flag)
    const h = toRoute(location.hash.replace(/^#/, ""));
    if (h) route.set(h);
    unlisteners.push(route.subscribe((r) => {
      if (ROUTES.includes(r) && location.hash !== "#" + r) history.replaceState(null, "", "#" + r);
    }));
    (async () => {
      unlisteners.push(watchTheme(themeKey, 4000));

      await refresh();
      const timer = setInterval(refresh, 30000);
      unlisteners.push(() => clearInterval(timer));
      // back in front (the bar, a second launch): read everything at once
      const onFocus = () => refresh();
      window.addEventListener("focus", onFocus);
      unlisteners.push(() => window.removeEventListener("focus", onFocus));

      unlisteners.push(await listen("deep-link", takeLink));
      takeLink();
      unlisteners.push(await listen("login-url", (e) => loginUrl.set(String(e.payload || ""))));
      unlisteners.push(await listen("google-consent-url", (e) => consentUrl.set(String(e.payload || ""))));
      // the folder runner reports every change of state (a run, a conflict)
      unlisteners.push(await listen("folders-status", (e) => folders.set(e.payload)));
      try {
        folders.set(await api.foldersList());
      } catch {
        /* outside the desktop */
      }
    })();
    return () => unlisteners.forEach((u) => u && u());
  });

  // the account's picture, once per sign-in (the rail, Overview, Account);
  // a server without one is asked once, not on every refresh
  let avatarAsked = false;
  $effect(() => {
    if (!$cloud?.signed_in) {
      avatarAsked = false;
      if ($avatar) avatar.set("");
      return;
    }
    if ($avatar || avatarAsked) return;
    avatarAsked = true;
    api.cloudAvatar().then((a) => {
      if (a?.ok && a.path) avatar.set(api.fileSrc(a.path));
    }).catch(() => {});
  });
</script>

<svelte:window onkeydown={globalKey} />

<div class="ewe-appwin">
  <Sidebar items={NAV} />
  <!-- the pane: the main landmark, inset from the window's edges (App shell) -->
  <main class="ewe-appwin__pane">
    <div class="ewe-appwin__scroll">
      {#if $route === "overview"}
        <Overview {refresh} {syncNow} />
      {:else if $route === "account"}
        <Account {refresh} />
      {:else if $route === "mail"}
        <Mail {refresh} />
      {:else if $route === "google"}
        <Google {refresh} />
      {:else if $route === "machine"}
        <ThisMachine {refresh} {syncNow} />
      {:else if $route === "machines"}
        <Machines />
      {:else}
        <Folders />
      {/if}
    </div>
  </main>
</div>
<Toasts />
