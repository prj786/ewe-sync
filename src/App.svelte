<script>
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { listen as tauriListen } from "@tauri-apps/api/event";
  // in a plain browser (devmock) there is no event bus — subscribe to nothing
  const listen = (ev, cb) => (window.__EWE_SYNC_MOCK__ ? Promise.resolve(() => {}) : tauriListen(ev, cb));
  import * as api from "./lib/api";
  import { route, cloud, sync, busy, loginUrl, trayState, toast } from "./lib/stores";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import Account from "./lib/components/Account.svelte";
  import ThisMachine from "./lib/components/ThisMachine.svelte";
  import Machines from "./lib/components/Machines.svelte";
  import Folders from "./lib/components/Folders.svelte";
  import Toasts from "./lib/components/Toasts.svelte";

  // ewe is dark by decision; the class is on <html> already. Follow the DE's
  // accent and surface live (re-read on focus + a light poll), like Komble.
  async function applyDePrefs() {
    try {
      const p = await api.dePrefs();
      if (!p) return;
      document.documentElement.style.setProperty("--accent", p.accent);
      document.documentElement.classList.toggle("blacksheep", (p.themeName || "flock") === "blacksheep");
      document.documentElement.classList.toggle("dark", (p.colorScheme || "dark") !== "light");
    } catch {
      /* outside the desktop */
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
  }

  async function syncNow() {
    if (get(busy)) return;
    busy.set("sync");
    try {
      const r = await api.push(false);
      if (r.ok) toast("Settings backed up", "success");
      else if (r.error === "remote-newer") toast("Another machine saved newer settings — restore it first, or push anyway from This machine.", "error");
      else if (r.error === "remote-exists") toast("A backup already exists — restore it first, or push anyway from This machine.", "error");
      else toast(r.message || r.error || "Sync failed", "error");
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

  const ROUTES = ["account", "machine", "machines", "folders"];
  onMount(() => {
    const unlisteners = [];
    // #account / #machine / #machines / #folders open a pane directly (deep
    // links from the shell and Settings; harmless inside Tauri)
    const h = location.hash.replace(/^#/, "");
    if (ROUTES.includes(h)) route.set(h);
    unlisteners.push(route.subscribe((r) => {
      if (ROUTES.includes(r) && location.hash !== "#" + r) history.replaceState(null, "", "#" + r);
    }));
    (async () => {
      applyDePrefs();
      window.addEventListener("focus", applyDePrefs);
      const deTimer = setInterval(applyDePrefs, 4000);
      unlisteners.push(() => {
        window.removeEventListener("focus", applyDePrefs);
        clearInterval(deTimer);
      });

      await refresh();
      const timer = setInterval(refresh, 30000);
      unlisteners.push(() => clearInterval(timer));
      window.addEventListener("focus", refresh);
      unlisteners.push(() => window.removeEventListener("focus", refresh));

      unlisteners.push(await listen("login-url", (e) => loginUrl.set(String(e.payload || ""))));
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

<div class="flex h-full">
  <Sidebar />
  <main class="min-w-0 flex-1 overflow-y-auto px-6 py-5">
    {#if $route === "account"}
      <Account {refresh} />
    {:else if $route === "machine"}
      <ThisMachine {refresh} {syncNow} />
    {:else if $route === "machines"}
      <Machines />
    {:else}
      <Folders />
    {/if}
  </main>
  <Toasts />
</div>
