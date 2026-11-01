<script>
  // Google — the OPTIONAL extra beside the ewe account (RFC-005): Gmail in
  // Quick settings and Drive as a folder. Never settings sync; that is the
  // Nextcloud account's job and only its job.
  //
  // ewe ships NO Google client, on purpose — nothing about the project's owner
  // lives in the package. So this pane leads with the client file, as two
  // steps: create one in Google's console, save it where ewe looks (and
  // whether the one you saved there is usable). There is nothing to connect
  // until that file is valid, and saying so plainly is most of this pane's
  // work.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import * as api from "../api";
  import { google, gclient, consentUrl, toast, tildePath } from "../stores";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Row from "./ui/Row.svelte";
  import KV from "./ui/KV.svelte";
  import Alert from "./ui/Alert.svelte";
  import Empty from "./ui/Empty.svelte";
  import IconBtn from "./ui/IconBtn.svelte";
  import Icon from "./ui/Icon.svelte";

  let { refresh } = $props();

  let working = $state(false);
  let error = $state("");

  const CONSOLE = "https://console.cloud.google.com/apis/credentials";
  const fullPath = $derived($gclient?.path || "~/.config/ewe/oauth-client.json");
  const path = $derived(tildePath(fullPath));
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

  function copyPath() {
    navigator.clipboard
      .writeText(fullPath)
      .then(() => toast("Copied the path", "success"))
      .catch((e) => toast(String(e), "error"));
  }
</script>

<Page
  title="Google"
  desc="Optional: Gmail in Quick settings and Drive as a folder. Settings sync never uses Google."
>
  {#if clientState === "invalid"}
    <Alert tone="warning" title="This file isn’t a Desktop client">
      Download the client again from the console: pick Desktop app, then “Download JSON”, and save it as it is.
    </Alert>
  {/if}

  <!-- the client file, first: without it there is nothing to connect -->
  <Group title="Your Google client">
    <button class="ewe-row ewe-row--interactive row-button" onclick={() => openUrl(CONSOLE)}>
      <span class="ewe-step" class:is-done={clientState !== "missing"} class:is-current={clientState === "missing"}>
        <span class="ewe-step__dot">{#if clientState !== "missing"}<Icon name="check" />{:else}1{/if}</span>
      </span>
      <span class="ewe-row__text">
        <span class="ewe-row__title">Create a Desktop client</span>
        <span class="ewe-row__desc">In the Google Cloud console, with the Gmail and Drive APIs turned on.</span>
      </span>
      <span class="ewe-row__trail">Open the console<Icon name="external" /></span>
    </button>
    <div class="ewe-list__divider"></div>
    <div class="ewe-row">
      <span class="ewe-step" class:is-done={clientState === "valid"} class:is-current={clientState === "invalid"}>
        <span class="ewe-step__dot">{#if clientState === "valid"}<Icon name="check" />{:else}2{/if}</span>
      </span>
      <span class="ewe-row__text">
        <span class="ewe-row__title">Save its file here</span>
        <span class="ewe-row__desc ewe-mono path-line" title={fullPath}>{path}</span>
      </span>
      <span class="ewe-row__trail">
        {#if clientState === "valid"}
          <span class="ewe-badge ewe-badge--success"><span class="ewe-badge__label">Found</span></span>
        {:else if clientState === "invalid"}
          <span class="ewe-badge ewe-badge--warning"><span class="ewe-badge__label">Not usable</span></span>
        {:else}
          <span class="ewe-badge ewe-badge--warning"><span class="ewe-badge__label">Missing</span></span>
        {/if}
        <IconBtn name="copy" title="Copy the path" go={copyPath} />
      </span>
    </div>
  </Group>

  <!-- the connection itself -->
  <Group title="Connection">
    {#if clientState !== "valid"}
      <Empty
        compact
        icon="globe"
        title="Nothing to connect yet"
        desc="Save your client file first. ewe looks for it at the path above."
      >
        <button class="ewe-btn ewe-btn--secondary ewe-btn--sm" onclick={() => openUrl(CONSOLE)}>
          Open the Google Cloud console<Icon name="external" />
        </button>
      </Empty>
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

  <p class="note">
    ewe ships no Google client of its own: you make one for yourself. A personal client needs no review; Google shows a
    warning once that you click through.{#if $google?.signed_in && clientState === "valid"}
      A mail account set up on the Mail page comes before Gmail in Quick settings.{/if}
  </p>
</Page>
