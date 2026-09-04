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

  // ewe is dark by decision; the class is on <html> already. Follow the DE's
  // accent and surface live (re-read on focus + a light poll), like Komble.
  // The look, live. tokens.css is compiled in as the fallback, and it is built
  // from the DEFAULT accent — so on its own the app wears the wrong greys the
  // moment the user picks an accent. These values come from `ewe-theme show`,
  // which reads THIS machine's ewe.conf, so the whole derived set lands:
  // the brand ramp, and the neutrals carrying the accent's tint.
  //
  // Keyed on the ACCENT, not the theme name. There is one ewe look now — the
  // name never changes, so keying on it meant injecting once at startup and
  // never again, and every later accent change stopped at the app boundary.
  let injectedKey = "";
  async function applyThemeTokens(accent) {
    const key = String(accent || "");
    if (key === injectedKey) return;
    injectedKey = key;
    try {
      const t = await api.themeTokens("ewe");
      if (!t || !t.css_vars) return;
      for (const [k, v] of Object.entries(t.css_vars)) {
        document.documentElement.style.setProperty(k, v);
      }
    } catch {
      injectedKey = "";   // let a later attempt retry
      /* ewe-theme absent (dev, or ewe not deployed) — the compiled-in
         tokens.css already carries the default look */
    }
  }

  async function applyDePrefs() {
    try {
      const p = await api.dePrefs();
      if (!p) return;
      // "" = never picked, so the theme default in tokens.css stands
      if (p.accent) document.documentElement.style.setProperty("--accent", p.accent);
      else document.documentElement.style.removeProperty("--accent");
      document.documentElement.classList.toggle("dark", (p.colorScheme || "dark") !== "light");
      // one look — the ACCENT is what the derived token set follows
      applyThemeTokens(p.accent || "");
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

<div class="surface flex h-full">
  <Sidebar />
  <main class="min-w-0 flex-1 overflow-y-auto px-6 py-5">
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
  </main>
  <Toasts />
</div>
