<script>
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { listen as tauriListen } from "@tauri-apps/api/event";
  // in a plain browser (devmock) there is no event bus — subscribe to nothing
  const listen = (ev, cb) => (window.__EWE_SYNC_MOCK__ ? Promise.resolve(() => {}) : tauriListen(ev, cb));
  import * as api from "./lib/api";
  import { route, cloud, sync, busy, loginUrl, folders, trayState, toast, mail, google, gclient, consentUrl } from "./lib/stores";
  import Sidebar from "./lib/components/Sidebar.svelte";
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

  // the sections (Side navigation); Ctrl+1 … Ctrl+6 jump to them
  const NAV = [
    { id: "account", label: "Account", icon: "user" },
    { id: "mail", label: "Mail", icon: "mail" },
    { id: "google", label: "Google", icon: "globe" },
    { id: "machine", label: "This machine", icon: "monitor" },
    { id: "machines", label: "Machines", icon: "machines" },
    { id: "folders", label: "Folders", icon: "folderSync" }
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

  /** the account + the one file, in one round */
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

  async function syncNow() {
    if (get(busy)) return;
    busy.set("sync");
    try {
      // the folders too, when there are any — the tray's Sync now means everything
      if (get(folders)?.pairs?.length) api.foldersRunAll().catch(() => {});
      const r = await api.push(false);
      if (r.ok) toast("Settings backed up", "success");
      else if (r.error === "remote-newer") toast("Another machine saved newer settings. Restore them first, or push anyway from This machine.", "danger");
      else if (r.error === "remote-exists") toast("A backup already exists. Restore it first, or push anyway from This machine.", "danger");
      else toast(r.message || r.error || "Couldn’t sync. Try again.", "danger");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy.set("");
      refresh();
    }
  }

  async function togglePause() {
    const on = !!get(sync)?.enabled;
    try {
      await api.confSet("sync.enabled", !on);
      await api.trayPauseLabel(on);
      toast(on ? "Auto-sync paused" : "Auto-sync resumed", "info");
    } catch (e) {
      toast(String(e), "error");
    }
    refresh();
  }

  const ROUTES = ["account", "mail", "google", "machine", "machines", "folders"];
  onMount(() => {
    const unlisteners = [];
    // #account / #mail / #google / #machine / #machines / #folders open a pane
    // directly (deep
    // links from the shell and Settings; harmless inside Tauri)
    const h = location.hash.replace(/^#/, "");
    if (ROUTES.includes(h)) route.set(h);
    unlisteners.push(route.subscribe((r) => {
      if (ROUTES.includes(r) && location.hash !== "#" + r) history.replaceState(null, "", "#" + r);
    }));
    (async () => {
      unlisteners.push(watchTheme(themeKey));

      await refresh();
      const timer = setInterval(refresh, 30000);
      unlisteners.push(() => clearInterval(timer));
      window.addEventListener("focus", refresh);
      unlisteners.push(() => window.removeEventListener("focus", refresh));

      unlisteners.push(await listen("login-url", (e) => loginUrl.set(String(e.payload || ""))));
      unlisteners.push(await listen("google-consent-url", (e) => consentUrl.set(String(e.payload || ""))));
      // the folder runner reports every change of state (a run, a conflict)
      unlisteners.push(await listen("folders-status", (e) => folders.set(e.payload)));
      try {
        folders.set(await api.foldersList());
      } catch {
        /* outside the desktop */
      }
      unlisteners.push(
        await listen("tray-action", (e) => {
          if (e.payload === "sync-now") syncNow();
          if (e.payload === "pause") togglePause();
        })
      );
    })();
    return () => unlisteners.forEach((u) => u && u());
  });

  // the tray follows the state machine in stores.js
  $effect(() => {
    const c = $cloud;
    const tip = c?.signed_in ? `ewe-sync — ${c.display_name || c.user}` : "ewe-sync — not signed in";
    api.trayState($trayState, tip).catch(() => {});
  });
  $effect(() => {
    const s = $sync;
    if (s && typeof s.enabled === "boolean") api.trayPauseLabel(!s.enabled).catch(() => {});
  });
</script>

<svelte:window onkeydown={globalKey} />

<div class="ewe-appwin">
  <Sidebar items={NAV} />
  <!-- the pane: the main landmark, inset from the window's edges (App shell) -->
  <main class="ewe-appwin__pane">
    <div class="ewe-appwin__scroll">
      {#if $route === "account"}
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
