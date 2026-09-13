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

<div class="mx-auto max-w-5xl">
  <div class="eyebrow">Your account · Nextcloud</div>

  {#if $cloud?.signed_in}
    <div class="card p-6">
      <div class="flex items-start gap-6">
        {#if avatar}
          <img src={avatar} alt="" class="avatar" />
        {:else}
          <div class="avatar">
            {($cloud.display_name || $cloud.user || "?").slice(0, 1).toUpperCase()}
          </div>
        {/if}
        <div class="min-w-0 flex-1 pt-1">
          <div class="truncate text-xl font-semibold text-fg">{$cloud.display_name || $cloud.user}</div>
          <div class="mt-1 truncate text-sm text-dim">{$cloud.email || ""}</div>
          <div class="truncate text-sm text-dim">{$cloud.server}</div>
        </div>
        <button class="btn-ghost shrink-0" disabled={!!$busy} onclick={signOut}>Sign out</button>
      </div>
      {#if $cloud.offline}
        <div class="callout is-warning mt-4">Your server did not answer — showing what was known. {$cloud.reason || ""}</div>
      {/if}
      {#if $cloud.quota && $cloud.quota.total > 0}
        <div class="mt-6">
          <div class="mb-2 flex justify-between text-sm text-dim">
            <span>Storage</span>
            <span class="tabular-nums">{fmtBytes($cloud.quota.used)} of {fmtBytes($cloud.quota.total)}</span>
          </div>
          <div class="meter">
            <span class="meter-fill {quotaPct >= 95 ? 'is-danger' : quotaPct >= 80 ? 'is-warning' : ''}" style="width: {quotaPct}%"></span>
          </div>
        </div>
      {/if}
      <div class="mt-4 -mx-1">
        <div class="kv">
          <span class="text-muted">Signed in as</span>
          <span class="text-fg">{$cloud.user}</span>
        </div>
      </div>
      <div class="mt-3 text-sm text-dim">
        This device appears in your Nextcloud under Settings → Security as “ewe”. Revoking it there signs this machine out.
      </div>
    </div>
  {:else}
    <div class="card p-6">
      <div class="text-sm text-muted">
        Sign in to your own Nextcloud — self-hosted or a hosted provider. The browser opens your server's login page; ewe never sees your password.
      </div>
      {#if $cloud?.reason === "revoked"}
        <div class="callout is-warning mt-4">This machine's access was revoked on the server. Sign in again to continue.</div>
      {/if}
      {#if keyringTrouble}
        <div class="callout is-warning mt-4">
          {#if keyringState === "locked"}
            Your keyring is locked: an “Unlock keyring” prompt will appear — answer it with your login password. If it keeps rejecting that password, reset the keyring.
          {:else if keyringState === "unavailable"}
            No keyring is running — gnome-keyring must be installed and started for this session.
          {:else}
            The keyring refused the last sign-in. Reset it, log out and back in, then sign in again.
          {/if}
          <div class="mt-2 flex gap-3">
            {#if !resetDone}<button class="link text-xs" onclick={resetKeyring}>Reset the keyring</button>{/if}
            {#if resetDone}<button class="link text-xs" onclick={() => api.sessionLogout().catch((e) => toast(String(e), "error"))}>Log out now</button>{/if}
          </div>
        </div>
      {/if}
      <div class="mt-4 flex gap-2">
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
        <div class="mt-3 text-sm text-dim">
          Waiting for the browser… finish signing in on your server, then come back here.
          {#if $loginUrl}
            <div class="mt-2 flex gap-4">
              <button class="link" onclick={() => openUrl($loginUrl)}>Open the sign-in page</button>
              <button class="link" onclick={() => navigator.clipboard.writeText($loginUrl)}>Copy the link</button>
            </div>
          {/if}
        </div>
      {/if}
      {#if error}<div class="mt-3 text-sm text-danger">{error}</div>{/if}
    </div>

    <div class="eyebrow mt-6">Create an account</div>
    <div class="card p-6">
      <div class="mb-3 text-sm text-dim">
        ewe-sync cannot create accounts — only a server's own signup page can. Pick a provider, sign up in the browser, then come back and sign in. Examples, not endorsements.
      </div>
      <div class="list-well">
        {#each providers as p}
          <button class="list-row text-left" onclick={() => openUrl(p.url)}>
            <span class="min-w-0 flex-1 truncate">
              <span class="font-medium">{p.name}</span>
              <span class="ml-2 text-dim">{p.note}</span>
            </span>
            <span class="icon text-dim">{String.fromCodePoint(0xE06F)}</span>
          </button>
        {/each}
      </div>
    </div>
  {/if}
</div>
