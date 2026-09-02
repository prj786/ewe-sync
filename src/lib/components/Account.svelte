<script>
  import { openUrl } from "@tauri-apps/plugin-opener";
  import * as api from "../api";
  import { cloud, busy, loginUrl, toast, fmtBytes } from "../stores";

  let { refresh } = $props();

  let server = $state(localStorage.getItem("ewe-sync.lastServer") || "");
  let error = $state("");
  let avatar = $state("");
  let resetDone = $state(false);

  const providers = [
    { name: "I run my own", url: "https://nextcloud.com/install/", note: "any Nextcloud you administer" },
    { name: "Murena", url: "https://murena.io/signup", note: "the de-Googled cloud, free tier" },
    { name: "Disroot", url: "https://user.disroot.org/pages/signup", note: "privacy-first, community run" },
    { name: "tab.digital", url: "https://tab.digital/", note: "hosted Nextcloud, free tier" },
    { name: "Infomaniak", url: "https://www.infomaniak.com/en/ksuite", note: "Swiss, free kSuite" }
  ];

  const keyringState = $derived($cloud?.keyring_state || "ok");
  const keyringTrouble = $derived(
    keyringState === "locked" || keyringState === "unavailable" || String($cloud?.reason || "").startsWith("keyring")
  );

  async function signIn() {
    if ($busy) return;
    error = "";
    loginUrl.set("");
    let s = server.trim();
    if (!s) {
      error = "Type your server address first.";
      return;
    }
    if (!s.includes("://")) s = "https://" + s;
    localStorage.setItem("ewe-sync.lastServer", s);
    busy.set("signin");
    try {
      const r = await api.cloudLogin(s);
      if (r.ok) {
        toast(`Signed in as ${r.display_name || r.user}`, "success");
        await refresh();
        try {
          const a = await api.cloudAvatar();
          if (a?.ok && a.path) avatar = api.fileSrc(a.path) + (String(a.path).startsWith("data:") ? "" : "?t=" + Date.now());
        } catch {}
        await api.machinesWrite().catch(() => {});
        await api.shellPoke("refresh").catch(() => {});
      } else {
        error = r.message || r.error || "Sign-in failed";
      }
    } catch (e) {
      error = String(e);
    } finally {
      busy.set("");
      loginUrl.set("");
    }
  }

  async function signOut() {
    if ($busy) return;
    busy.set("logout");
    try {
      await api.cloudLogout();
      avatar = "";
      toast("Signed out", "info");
      await api.shellPoke("refresh").catch(() => {});
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy.set("");
      refresh();
    }
  }

  async function resetKeyring() {
    try {
      const r = await api.keyringReset();
      if (r.ok) {
        resetDone = !!r.relogin;
        toast(r.message || "Keyring reset", "info", 10000);
      } else toast(r.message || r.error, "error");
    } catch (e) {
      toast(String(e), "error");
    }
  }

  $effect(() => {
    if ($cloud?.signed_in && !avatar) {
      api.cloudAvatar().then((a) => {
        if (a?.ok && a.path) avatar = api.fileSrc(a.path);
      }).catch(() => {});
    }
  });

  const quotaPct = $derived(Math.max(0, Math.min(100, Math.round(($cloud?.quota?.relative ?? 0) * 1))));
</script>

<div class="mx-auto max-w-2xl">
  <div class="section-title">Your account · Nextcloud</div>

  {#if $cloud?.signed_in}
    <div class="card divide-y divide-zinc-200 dark:divide-zinc-700/60">
      <div class="flex items-center gap-4 px-4 py-4">
        {#if avatar}
          <img src={avatar} alt="" class="h-14 w-14 rounded-full object-cover" />
        {:else}
          <div class="flex h-14 w-14 items-center justify-center rounded-full text-xl font-semibold text-white" style="background: var(--accent)">
            {($cloud.display_name || $cloud.user || "?").slice(0, 1).toUpperCase()}
          </div>
        {/if}
        <div class="min-w-0 flex-1">
          <div class="truncate text-base font-semibold">{$cloud.display_name || $cloud.user}</div>
          <div class="truncate text-xs text-zinc-500 dark:text-zinc-400">{$cloud.email || ""}</div>
          <div class="truncate text-xs text-zinc-500 dark:text-zinc-400">{$cloud.server}</div>
        </div>
        <button class="btn-ghost" disabled={!!$busy} onclick={signOut}>Sign out</button>
      </div>
      {#if $cloud.offline}
        <div class="px-4 py-2.5 text-xs text-amber-500">Your server did not answer — showing what was known. {$cloud.reason || ""}</div>
      {/if}
      {#if $cloud.quota && $cloud.quota.total > 0}
        <div class="px-4 py-3">
          <div class="mb-1 flex justify-between text-xs text-zinc-500 dark:text-zinc-400">
            <span>Storage</span>
            <span>{fmtBytes($cloud.quota.used)} of {fmtBytes($cloud.quota.total)}</span>
          </div>
          <div class="h-1.5 w-full overflow-hidden rounded-full bg-zinc-200 dark:bg-zinc-700">
            <div class="h-full rounded-full" style="width: {quotaPct}%; background: var(--accent)"></div>
          </div>
        </div>
      {/if}
      <div class="kv">
        <span class="text-zinc-500 dark:text-zinc-400">Signed in as</span>
        <span class="font-mono text-xs">{$cloud.user}</span>
      </div>
      <div class="px-4 py-2.5 text-xs text-zinc-500 dark:text-zinc-400">
        This device appears in your Nextcloud under Settings → Security as “ewe”. Revoking it there signs this machine out.
      </div>
    </div>
  {:else}
    <div class="card">
      <div class="px-4 pt-4 text-sm">
        Sign in to your own Nextcloud — self-hosted or a hosted provider. The browser opens your server's login page; ewe never sees your password.
      </div>
      {#if $cloud?.reason === "revoked"}
        <div class="mx-4 mt-3 note">This machine's access was revoked on the server. Sign in again to continue.</div>
      {/if}
      {#if keyringTrouble}
        <div class="mx-4 mt-3 note">
          {#if keyringState === "locked"}
            Your keyring is locked: an “Unlock keyring” prompt will appear — answer it with your login password. If it keeps rejecting that password, reset the keyring.
          {:else if keyringState === "unavailable"}
            No keyring is running — gnome-keyring must be installed and started for this session.
          {:else}
            The keyring refused the last sign-in. Reset it, log out and back in, then sign in again.
          {/if}
          <div class="mt-2 flex gap-3">
            {#if !resetDone}<button class="text-xs underline" onclick={resetKeyring}>Reset the keyring</button>{/if}
            {#if resetDone}<button class="text-xs underline" onclick={() => api.sessionLogout().catch((e) => toast(String(e), "error"))}>Log out now</button>{/if}
          </div>
        </div>
      {/if}
      <div class="flex gap-2 px-4 py-4">
        <input
          class="input"
          placeholder="https://cloud.example.org"
          bind:value={server}
          disabled={$busy === "signin"}
          onkeydown={(e) => e.key === "Enter" && signIn()}
        />
        {#if $busy === "signin"}
          <button class="btn-ghost" onclick={() => (busy.set(""), loginUrl.set(""))}>Cancel</button>
        {:else}
          <button class="btn-primary" onclick={signIn}>Sign in</button>
        {/if}
      </div>
      {#if $busy === "signin"}
        <div class="px-4 pb-3 text-xs text-zinc-500 dark:text-zinc-400">
          Waiting for the browser… finish signing in on your server, then come back here.
          {#if $loginUrl}
            <div class="mt-2 flex gap-4">
              <button class="underline" onclick={() => openUrl($loginUrl)}>Open the sign-in page</button>
              <button class="underline" onclick={() => navigator.clipboard.writeText($loginUrl)}>Copy the link</button>
            </div>
          {/if}
        </div>
      {/if}
      {#if error}<div class="px-4 pb-3 text-xs text-red-500">{error}</div>{/if}
    </div>

    <div class="section-title">Create an account</div>
    <div class="card divide-y divide-zinc-200 dark:divide-zinc-700/60">
      <div class="px-4 py-3 text-xs text-zinc-500 dark:text-zinc-400">
        ewe-sync cannot create accounts — only a server's own signup page can. Pick a provider, sign up in the browser, then come back and sign in. Examples, not endorsements.
      </div>
      {#each providers as p}
        <button class="kv w-full text-left hover:bg-zinc-100 dark:hover:bg-zinc-700/40" onclick={() => openUrl(p.url)}>
          <span>
            <span class="font-medium">{p.name}</span>
            <span class="ml-2 text-xs text-zinc-500 dark:text-zinc-400">{p.note}</span>
          </span>
          <span class="ph-i text-[14px] text-zinc-400">{String.fromCodePoint(0xe13a)}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>
