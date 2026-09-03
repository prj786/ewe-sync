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
        error = r?.message || r?.error || "Could not connect.";
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

<div class="mx-auto max-w-2xl">
  <div class="section-title">Google · optional</div>

  <!-- ── the client file, first: without it there is nothing to connect ── -->
  <div class="card divide-y divide-hairline">
    <div class="px-4 py-3 text-xs text-dim">
      ewe ships no Google client of its own. To use Gmail and Drive, create your own OAuth client of
      type <em>Desktop app</em> (with the Gmail and Drive APIs enabled) and save the file it gives
      you at the path below. A personal client needs no Google verification — its consent screen
      shows a warning you click through once.
    </div>
    <div class="kv">
      <span class="text-dim">Client file</span>
      <span class="font-mono text-xs">{path}</span>
    </div>
    <div class="kv">
      <span class="text-dim">Status</span>
      <span class="text-xs">
        {#if clientState === "valid"}
          <span class="text-success">found — a Desktop-app client</span>
        {:else if clientState === "invalid"}
          <span class="text-warning">found, but it is not a Desktop-app client JSON</span>
        {:else}
          <span class="text-dim">missing</span>
        {/if}
      </span>
    </div>
    {#if clientState === "invalid"}
      <div class="px-4 py-2.5 text-xs text-warning">
        The file is there but carries no <span class="font-mono">client_id</span>. Download the
        client again from the console — pick <em>Desktop app</em>, then “Download JSON”, and save it
        verbatim.
      </div>
    {/if}
    <button class="kv w-full text-left hover:bg-elevated dark:hover:bg-elevated" onclick={() => openUrl(CONSOLE)}>
      <span><span class="font-medium">Open the Google Cloud console</span>
        <span class="ml-2 text-xs text-dim">create the client there</span></span>
      <span class="ph-i text-[14px] text-dim">{String.fromCodePoint(0xe13a)}</span>
    </button>
  </div>

  <!-- ── the connection itself ── -->
  <div class="section-title">Connection</div>
  <div class="card divide-y divide-hairline">
    {#if clientState !== "valid"}
      <div class="px-4 py-4 text-sm text-dim">
        Nothing to connect until the client file is in place.
      </div>
    {:else if $google?.signed_in}
      <div class="flex items-center gap-4 px-4 py-4">
        {#if $google.profile?.picture}
          <img src={$google.profile.picture} alt="" class="h-12 w-12 rounded-full" />
        {/if}
        <div class="min-w-0 flex-1">
          <div class="truncate text-base font-semibold">{$google.profile?.name || "Google"}</div>
          <div class="truncate text-xs text-dim">{$google.profile?.email || ""}</div>
        </div>
        <button class="btn-ghost" disabled={working} onclick={disconnect}>Disconnect</button>
      </div>
      <div class="kv">
        <span class="text-dim">Gmail</span>
        <span class="text-xs">
          {$google.mail_state === "scope"
            ? "no mail permission — disconnect and connect again"
            : $google.mail_state === "ok"
              ? `${$google.mail_unread || 0} unread`
              : $google.mail_state || "—"}
        </span>
      </div>
      <div class="kv">
        <span class="text-dim">Drive folder</span>
        <span class="text-xs font-mono">~/Google Drive</span>
      </div>
      <div class="px-4 py-2.5 text-xs text-dim">
        Settings sync never uses Google — that is your Nextcloud account. An IMAP account, when one
        is set up, takes precedence over Gmail in the Control Center.
      </div>
    {:else}
      <div class="flex items-center justify-between gap-3 px-4 py-4">
        <div class="text-sm text-dim">
          {working ? "Waiting for the browser…" : "Connect your Google account with your own client."}
        </div>
        {#if working}
          <button class="btn-ghost shrink-0" onclick={() => (working = false)}>Cancel</button>
        {:else}
          <button class="btn-primary shrink-0" onclick={connect}>Connect</button>
        {/if}
      </div>
      {#if working && $consentUrl}
        <div class="px-4 py-2.5 text-xs">
          <div class="flex gap-4">
            <button class="underline" onclick={() => openUrl($consentUrl)}>Open the sign-in page</button>
            <button class="underline" onclick={() => navigator.clipboard.writeText($consentUrl)}>Copy the link</button>
          </div>
        </div>
      {/if}
      {#if error}<div class="px-4 pb-3 text-xs text-danger">{error}</div>{/if}
    {/if}
  </div>
</div>
