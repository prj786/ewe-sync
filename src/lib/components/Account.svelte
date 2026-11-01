<script>
  // Account — your own Nextcloud. Signed in: who, where and how much room,
  // in one well, and at the bottom the rare, disruptive Sign out (confirmed,
  // like Stop syncing). Signed out: the sign-in (your browser opens your
  // server's page; ewe never sees the password), what blocks it (access
  // revoked, a locked or missing keyring) right under the title, and where
  // to get an account.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import * as api from "../api";
  import { cloud, busy, loginUrl, avatar, toast, fmtBytes, route } from "../stores";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Row from "./ui/Row.svelte";
  import KV from "./ui/KV.svelte";
  import Alert from "./ui/Alert.svelte";
  import Dialog from "./ui/Dialog.svelte";
  import Icon from "./ui/Icon.svelte";

  let { refresh } = $props();

  let server = $state(localStorage.getItem("ewe-sync.lastServer") || "");
  let error = $state("");
  let resetDone = $state(false);
  let confirmSignOut = $state(false);

  const providers = [
    { name: "I run my own", url: "https://nextcloud.com/install/", note: "Any Nextcloud you administer", icon: "server" },
    { name: "Murena", url: "https://murena.io/signup", note: "The de-Googled cloud, free tier", icon: "cloud" },
    { name: "Disroot", url: "https://user.disroot.org/pages/signup", note: "Privacy-first, community run", icon: "cloud" },
    { name: "tab.digital", url: "https://tab.digital/", note: "Hosted Nextcloud, free tier", icon: "cloud" },
    { name: "Infomaniak", url: "https://www.infomaniak.com/en/ksuite", note: "Swiss, free kSuite", icon: "cloud" }
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
          if (a?.ok && a.path) avatar.set(api.fileSrc(a.path) + (String(a.path).startsWith("data:") ? "" : "?t=" + Date.now()));
        } catch {}
        // the same next steps as the shell's sign-in: your settings learn
        // their account, auto-sync is on, ~/Nextcloud appears; then Overview
        // offers to bring everything back (or to choose the folders to keep)
        await api.afterSignIn().catch(() => {});
        await refresh();
        route.set("overview");
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
    confirmSignOut = false;
    if ($busy) return;
    busy.set("logout");
    try {
      await api.cloudLogout();
      await api.afterSignOut().catch(() => {});
      avatar.set("");
      toast("Signed out. Your files stay on this computer.", "info");
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

  const quotaPct = $derived(Math.max(0, Math.min(100, Math.round($cloud?.quota?.relative ?? 0))));
  const host = $derived(String($cloud?.server || "").replace(/^https?:\/\//, "").replace(/\/+$/, ""));
</script>

<Page title="Account" desc="Your own Nextcloud. It keeps your settings, folders, browser and passwords.">
  {#if $cloud?.signed_in}
    {#if $cloud.offline}
      <Alert tone="" icon="cloudOff" title="Your server didn’t answer">
        Showing what was known at the last check.{$cloud.reason ? ` ${$cloud.reason}` : ""}
      </Alert>
    {/if}

    <Group title="Your account">
      <Row tall title={$cloud.display_name || $cloud.user} sub={$cloud.email || $cloud.user}>
        {#snippet lead()}
          {#if $avatar}
            <img class="ewe-avatar ewe-avatar--lg avatar-img" src={$avatar} alt="" />
          {:else}
            <span class="ewe-avatar ewe-avatar--lg" aria-hidden="true">
              {($cloud.display_name || $cloud.user || "?").slice(0, 1).toUpperCase()}
            </span>
          {/if}
        {/snippet}
      </Row>
      <div class="ewe-list__divider"></div>
      <KV k="Server" v={host} mono />
      <KV k="Signed in as" v={$cloud.user} />
      {#if $cloud.quota && $cloud.quota.total > 0}
        <div class="ewe-list__divider"></div>
        <div class="ewe-row ewe-row--block">
          <div class="ewe-meter {quotaPct >= 95 ? 'ewe-meter--danger' : quotaPct >= 80 ? 'ewe-meter--warning' : ''}">
            <div class="ewe-meter__head">
              <Icon name="cloud" />
              <span>{fmtBytes($cloud.quota.used)} of {fmtBytes($cloud.quota.total)} used · {fmtBytes($cloud.quota.total - $cloud.quota.used)} free</span>
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
      {/if}
    </Group>

    <Group title="This computer">
      <Row title="Sign out of this computer" sub="Syncing stops here. Your files and settings stay on this computer and in your account.">
        <button class="ewe-btn ewe-btn--sm ewe-btn--ghost danger-text" aria-disabled={!!$busy} onclick={() => !$busy && (confirmSignOut = true)}>
          {#if $busy === "logout"}<span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>Signing out…{:else}<Icon name="logOut" />Sign out…{/if}
        </button>
      </Row>
    </Group>

    <p class="note">
      This computer appears in your Nextcloud under Settings → Security as “ewe”. Revoking it there signs this computer
      out.
    </p>
  {:else}
    {#if $cloud?.reason === "revoked"}
      <Alert tone="warning" title="Access was revoked">
        This computer’s access was revoked on the server. Sign in again to continue.
      </Alert>
    {/if}
    {#if keyringTrouble}
      <Alert tone="warning" title="The keyring needs attention">
        {#if keyringState === "locked"}
          Your keyring is locked. An “Unlock keyring” prompt will appear: answer it with your login password. If it keeps
          rejecting that password, reset the keyring.
        {:else if keyringState === "unavailable"}
          No keyring is running. gnome-keyring must be installed and started for this session.
        {:else}
          The keyring refused the last sign-in. Reset it, sign out of the desktop and back in, then sign in here again.
        {/if}
        {#snippet actions()}
          {#if !resetDone}
            <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" onclick={resetKeyring}>Reset the keyring</button>
          {:else}
            <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" onclick={() => api.sessionLogout().catch((e) => toast(String(e), "error"))}
              >Sign out of the desktop now</button
            >
          {/if}
        {/snippet}
      </Alert>
    {/if}

    <Group well={false}>
      <div class="ewe-card signin-card">
        <div class="ewe-card__head">
          <span class="ewe-card__icon ewe-card__icon--accent"><Icon name="cloud" /></span>
          <div class="ewe-card__titles">
            <div class="ewe-card__title">Sign in to your Nextcloud</div>
            <div class="ewe-card__desc">
              Self-hosted or from a provider. Your browser opens your server’s sign-in page; ewe never sees your password.
            </div>
          </div>
        </div>
        <label class="ewe-field">
          <span class="ewe-field__label">Server address</span>
          <span class="signin-line">
            <input
              class="ewe-input"
              class:is-error={!!error}
              placeholder="cloud.example.org"
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
      title="No account yet?"
      variant="title"
      desc="ewe Sync can’t create one: only a server’s own sign-up page can. Pick one, sign up in the browser, then come back and sign in."
    >
      {#each providers as p, i}
        {#if i > 0}<div class="ewe-list__divider"></div>{/if}
        <button class="ewe-row ewe-row--interactive row-button" onclick={() => openUrl(p.url)}>
          <span class="ewe-row__lead ewe-row__lead--tile"><Icon name={p.icon} /></span>
          <span class="ewe-row__text">
            <span class="ewe-row__title">{p.name}</span>
            <span class="ewe-row__desc">{p.note}</span>
          </span>
          <span class="ewe-row__trail"><Icon name="external" /></span>
        </button>
      {/each}
    </Group>
    <p class="note">Examples, not endorsements.</p>
  {/if}
</Page>

{#if confirmSignOut}
  <Dialog
    title={`Sign out of ${host || "your account"}?`}
    icon="logOut"
    tone="danger"
    confirm="Sign out"
    onConfirm={signOut}
    onCancel={() => (confirmSignOut = false)}
  >
    Folders, settings, ewe Browser and ewe Safe stop syncing on this computer. Nothing is deleted: your files stay here
    and in your account, and everything picks up again when you sign in.
  </Dialog>
{/if}
