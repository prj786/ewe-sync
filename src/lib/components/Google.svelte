<script>
  // Google — the OPTIONAL extra beside the ewe account (RFC-005): Gmail in the
  // Control Center and Drive as a folder. Never settings sync; that is the
  // Nextcloud account's job and only its job.
  //
  // ewe ships NO Google client, on purpose — nothing about the project's owner
  // lives in the package. So this pane leads with the client file: where it
  // goes, whether the one you dropped there parses, and how to get one. There
  // is nothing to connect until that file is valid, and saying so plainly is
  // most of this pane's work.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import * as api from "../api";
  import { google, gclient, consentUrl, toast } from "../stores";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Row from "./ui/Row.svelte";
  import KV from "./ui/KV.svelte";
  import Alert from "./ui/Alert.svelte";
  import Empty from "./ui/Empty.svelte";
  import Icon from "./ui/Icon.svelte";

  let { refresh } = $props();

  let working = $state(false);
  let error = $state("");

  const CONSOLE = "https://console.cloud.google.com/apis/credentials";
  const path = $derived($gclient?.path || "~/.config/ewe/oauth-client.json");
  const clientState = $derived(
    $gclient?.valid ? "valid" : $gclient?.exists ? "invalid" : "missing"
  );

  async function connect() {
    if (working) return;
    error = "";
    consentUrl.set("");
    working = true;
    try {
      const r = await api.googleLogin();
      if (r?.ok) {
        toast("Google connected", "success");
        await refresh();
      } else {
        error = r?.message || r?.error || "Couldn’t connect. Try again.";
      }
    } catch (e) {
      error = String(e);
    }
    working = false;
    consentUrl.set("");
  }

  async function disconnect() {
    working = true;
    try {
      await api.googleLogout();
      toast("Google disconnected", "info");
      await refresh();
    } catch (e) {
      toast(String(e), "error");
    }
    working = false;
  }
</script>

<Page
  title="Google"
  desc="Optional: Gmail in Quick settings and Drive as a folder. Settings sync never uses Google."
>
  <!-- the client file, first: without it there is nothing to connect -->
  <Group
    title="Your OAuth client"
    desc="ewe ships no Google client of its own. Create an OAuth client of type Desktop app, with the Gmail and Drive APIs enabled, and save its file at the path below. A personal client needs no verification; its consent screen shows a warning you click through once."
  >
    <KV k="Client file" v={path} mono />
    <KV k="Status">
      {#if clientState === "valid"}
        <span class="ewe-badge ewe-badge--success"><span class="ewe-badge__label">Found</span></span>
        <span>A Desktop-app client</span>
      {:else if clientState === "invalid"}
        <span class="ewe-badge ewe-badge--warning"><span class="ewe-badge__label">Not usable</span></span>
        <span>Not a Desktop-app client file</span>
      {:else}
        <span class="ewe-badge"><span class="ewe-badge__label">Missing</span></span>
      {/if}
    </KV>
    <div class="ewe-list__divider"></div>
    <button class="ewe-row ewe-row--interactive row-button" onclick={() => openUrl(CONSOLE)}>
      <span class="ewe-row__lead ewe-row__lead--tile"><Icon name="globe" /></span>
      <span class="ewe-row__text">
        <span class="ewe-row__title">Open the Google Cloud console</span>
        <span class="ewe-row__desc">Create the client there</span>
      </span>
      <span class="ewe-row__trail"><Icon name="external" /></span>
    </button>
  </Group>

  {#if clientState === "invalid"}
    <Alert tone="warning" title="The client file has no client_id">
      Download the client again from the console: pick Desktop app, then “Download JSON”, and save it as it is.
    </Alert>
  {/if}

  <!-- the connection itself -->
  <Group title="Connection">
    {#if clientState !== "valid"}
      <Empty
        compact
        icon="globe"
        title="Nothing to connect yet"
        desc="Put your client file in place first."
      />
    {:else if $google?.signed_in}
      <Row tall title={$google.profile?.name || "Google"} sub={$google.profile?.email || ""}>
        {#snippet lead()}
          {#if $google.profile?.picture}
            <img class="ewe-avatar ewe-avatar--lg avatar-img" src={$google.profile.picture} alt="" />
          {:else}
            <span class="ewe-avatar ewe-avatar--lg" aria-hidden="true">
              {($google.profile?.name || "G").slice(0, 1).toUpperCase()}
            </span>
          {/if}
        {/snippet}
        <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" disabled={working} onclick={disconnect}>Disconnect</button>
      </Row>
      <div class="ewe-list__divider"></div>
      <KV
        k="Gmail"
        v={$google.mail_state === "scope"
          ? "No mail permission: disconnect and connect again"
          : $google.mail_state === "ok"
            ? `${$google.mail_unread || 0} unread`
            : $google.mail_state || "—"}
      />
      <KV k="Drive folder" v="~/Google Drive" mono />
    {:else}
      <Row
        icon="globe"
        title={working ? "Waiting for the browser…" : "Not connected"}
        sub={working ? "Finish the consent in your browser, then come back here." : "Connect your Google account with your own client."}
      >
        {#if working}
          <span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>
          <button class="ewe-btn ewe-btn--sm ewe-btn--ghost" onclick={() => (working = false)}>Cancel</button>
        {:else}
          <button class="ewe-btn ewe-btn--sm ewe-btn--primary" onclick={connect}>Connect</button>
        {/if}
      </Row>
      {#if working && $consentUrl}
        <div class="link-row link-row--inset">
          <button class="ewe-link" onclick={() => openUrl($consentUrl)}>Open the sign-in page <Icon name="external" /></button>
          <button class="ewe-link" onclick={() => navigator.clipboard.writeText($consentUrl)}>Copy the link</button>
        </div>
      {/if}
    {/if}
  </Group>

  {#if error}
    <Alert tone="danger" title="Couldn’t connect Google" dismiss={() => (error = "")}>{error}</Alert>
  {/if}

  {#if $google?.signed_in && clientState === "valid"}
    <p class="note">
      Settings sync never uses Google: that is your Nextcloud account. An IMAP account, when one is set up, takes
      precedence over Gmail in Quick settings.
    </p>
  {/if}
</Page>
