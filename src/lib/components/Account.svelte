<script>
  import { openUrl } from "@tauri-apps/plugin-opener";
  import * as api from "../api";
  import { cloud, busy, loginUrl, toast, fmtBytes } from "../stores";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Row from "./ui/Row.svelte";
  import KV from "./ui/KV.svelte";
  import Alert from "./ui/Alert.svelte";
  import Icon from "./ui/Icon.svelte";

  let { refresh } = $props();

  let server = $state(localStorage.getItem("ewe-sync.lastServer") || "");
  let error = $state("");
  let avatar = $state("");
  let resetDone = $state(false);

  const providers = [
    { name: "I run my own", url: "https://nextcloud.com/install/", note: "Any Nextcloud you administer" },
    { name: "Murena", url: "https://murena.io/signup", note: "The de-Googled cloud, free tier" },
    { name: "Disroot", url: "https://user.disroot.org/pages/signup", note: "Privacy-first, community run" },
    { name: "tab.digital", url: "https://tab.digital/", note: "Hosted Nextcloud, free tier" },
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
      error = "Enter your server’s address, then sign in.";
      return;
    }
    if (!s.includes("://")) s = "https://" + s;
    localStorage.setItem("ewe-sync.lastServer", s);
    busy.set("signin");
    try {
      const r = await api.cloudLogin(s);
      if (r.ok) {
        toast(`Signed in as **${r.display_name || r.user}**`, "success");
        await refresh();
        try {
          const a = await api.cloudAvatar();
          if (a?.ok && a.path) avatar = api.fileSrc(a.path) + (String(a.path).startsWith("data:") ? "" : "?t=" + Date.now());
        } catch {}
        await api.machinesWrite().catch(() => {});
        await api.shellPoke("refresh").catch(() => {});
      } else {
        error = r.message || r.error || "Couldn’t sign in. Check the address and try again.";
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

{#snippet signOutAction()}
  <button class="ewe-btn ewe-btn--secondary" disabled={!!$busy} onclick={signOut}>
    <Icon name="logOut" />{$busy === "logout" ? "Signing out…" : "Sign out"}
  </button>
{/snippet}

<Page
  title="Account"
  desc="Your Nextcloud account: it backs up ewe.conf and syncs your folders."
  actions={$cloud?.signed_in ? signOutAction : undefined}
>
  {#if $cloud?.signed_in}
    <Group title="Nextcloud">
      <Row tall title={$cloud.display_name || $cloud.user} sub={$cloud.email || $cloud.user}>
        {#snippet lead()}
          {#if avatar}
            <img class="ewe-avatar ewe-avatar--lg avatar-img" src={avatar} alt="" />
          {:else}
            <span class="ewe-avatar ewe-avatar--lg" aria-hidden="true">
              {($cloud.display_name || $cloud.user || "?").slice(0, 1).toUpperCase()}
            </span>
          {/if}
        {/snippet}
      </Row>
      <div class="ewe-list__divider"></div>
      <KV k="Server" v={$cloud.server} mono />
      <KV k="Signed in as" v={$cloud.user} />
    </Group>

    {#if $cloud.offline}
      <Alert tone="warning" title="Your server didn’t answer">
        Showing what was known at the last check.{$cloud.reason ? ` ${$cloud.reason}` : ""}
      </Alert>
    {/if}

    {#if $cloud.quota && $cloud.quota.total > 0}
      <Group title="Storage">
        <div class="ewe-row ewe-row--block">
          <div
            class="ewe-meter {quotaPct >= 95 ? 'ewe-meter--danger' : quotaPct >= 80 ? 'ewe-meter--warning' : ''}"
          >
            <div class="ewe-meter__head">
              <Icon name="cloud" />
              <span>{fmtBytes($cloud.quota.used)} of {fmtBytes($cloud.quota.total)} used</span>
              <span class="ewe-meter__value">{quotaPct}%</span>
            </div>
            <div
              class="ewe-meter__track"
              role="meter"
              aria-label="Storage used"
              aria-valuemin="0"
              aria-valuemax="100"
              aria-valuenow={quotaPct}
            >
              <div class="ewe-meter__fill" style="--value: {quotaPct}%"></div>
            </div>
          </div>
        </div>
      </Group>
    {/if}

    <p class="note">
      This machine appears in your Nextcloud under Settings → Security as “ewe”. Revoking it there signs this
      machine out.
    </p>
  {:else}
    <Group title="Sign in" well={false}>
      <div class="ewe-card">
        <p class="ewe-card__desc">
          Sign in to your own Nextcloud, self-hosted or from a provider. Your browser opens your server’s
          sign-in page; ewe never sees your password.
        </p>
        {#if $cloud?.reason === "revoked"}
          <Alert tone="warning" title="Access was revoked">
            This machine’s access was revoked on the server. Sign in again to continue.
          </Alert>
        {/if}
        {#if keyringTrouble}
          <Alert tone="warning" title="The keyring needs attention">
            {#if keyringState === "locked"}
              Your keyring is locked. An “Unlock keyring” prompt will appear: answer it with your login password. If
              it keeps rejecting that password, reset the keyring.
            {:else if keyringState === "unavailable"}
              No keyring is running. gnome-keyring must be installed and started for this session.
            {:else}
              The keyring refused the last sign-in. Reset it, sign out of the desktop and back in, then sign in
              here again.
            {/if}
            {#snippet actions()}
              {#if !resetDone}
                <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" onclick={resetKeyring}>Reset the keyring</button>
              {:else}
                <button
                  class="ewe-btn ewe-btn--sm ewe-btn--secondary"
                  onclick={() => api.sessionLogout().catch((e) => toast(String(e), "error"))}>Sign out now</button
                >
              {/if}
            {/snippet}
          </Alert>
        {/if}
        <label class="ewe-field">
          <span class="ewe-field__label">Server address</span>
          <span class="signin-line">
            <input
              class="ewe-input"
              class:is-error={!!error}
              placeholder="https://cloud.example.org"
              bind:value={server}
              disabled={$busy === "signin"}
              onkeydown={(e) => e.key === "Enter" && signIn()}
            />
            {#if $busy === "signin"}
              <button class="ewe-btn ewe-btn--ghost" onclick={() => (busy.set(""), loginUrl.set(""))}>Cancel</button>
            {:else}
              <button class="ewe-btn ewe-btn--primary" onclick={signIn}>Sign in</button>
            {/if}
          </span>
          {#if error}
            <span class="ewe-field__helper ewe-field__helper--error"><Icon name="alert" />{error}</span>
          {/if}
        </label>
        {#if $busy === "signin"}
          <div class="waiting">
            <span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>
            <span>Waiting for the browser… Finish signing in on your server, then come back here.</span>
          </div>
          {#if $loginUrl}
            <div class="link-row">
              <button class="ewe-link" onclick={() => openUrl($loginUrl)}>Open the sign-in page <Icon name="external" /></button>
              <button class="ewe-link" onclick={() => navigator.clipboard.writeText($loginUrl)}>Copy the link</button>
            </div>
          {/if}
        {/if}
      </div>
    </Group>

    <Group
      title="Create an account"
      desc="ewe-sync can’t create accounts; only a server’s own sign-up page can. Pick a provider, sign up in the browser, then come back and sign in. Examples, not endorsements."
    >
      {#each providers as p}
        <button class="ewe-row ewe-row--interactive row-button" onclick={() => openUrl(p.url)}>
          <span class="ewe-row__text">
            <span class="ewe-row__title">{p.name}</span>
            <span class="ewe-row__desc">{p.note}</span>
          </span>
          <span class="ewe-row__trail"><Icon name="external" /></span>
        </button>
      {/each}
    </Group>
  {/if}
</Page>
