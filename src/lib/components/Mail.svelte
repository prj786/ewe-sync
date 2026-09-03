<script>
  // Mail — any IMAP mailbox (RFC-005). Not tied to the ewe account: the inbox
  // a Nextcloud provider gives you, one you host, or a work account.
  //
  // This pane owns adding, changing and removing the account (2026-09-03, when
  // Settings became read-only). The new-mail NOTIFICATION switch deliberately
  // stays in Settings → User: it is a per-machine preference about this
  // desktop's behaviour, not part of the account.
  import * as api from "../api";
  import { mail, toast } from "../stores";

  let { refresh } = $props();

  let open = $state(false);
  let host = $state("");
  let port = $state(993);
  let user = $state("");
  let password = $state("");
  let starttls = $state(false);
  let working = $state(false);
  let error = $state("");
  let unseen = $state(null);

  const configured = $derived(!!$mail?.configured);

  /** STARTTLS servers are the 143 ones; keep the port honest as it is ticked. */
  function toggleStarttls() {
    starttls = !starttls;
    if (starttls && Number(port) === 993) port = 143;
    if (!starttls && Number(port) === 143) port = 993;
  }

  function edit() {
    host = $mail?.host || "";
    user = $mail?.user || "";
    port = Number($mail?.port) || 993;
    starttls = Number(port) === 143;
    password = "";
    error = "";
    open = true;
  }

  async function save() {
    error = "";
    if (!host.trim() || !user.trim() || !password) {
      error = "Server, user and password are all needed.";
      return;
    }
    working = true;
    try {
      const r = await api.mailLogin(host.trim(), Number(port) || 993, user.trim(), password, starttls);
      if (r?.ok) {
        password = "";
        open = false;
        toast("Mail account added", "success");
        await refresh();
      } else {
        // ewe-mail tests the login before storing, so this is the server talking
        error = r?.message || r?.error || "Could not sign in to the mail server.";
      }
    } catch (e) {
      error = String(e);
    }
    working = false;
  }

  async function remove() {
    working = true;
    try {
      const r = await api.mailLogout();
      if (r && r.ok === false) toast(r.message || r.error, "error");
      else toast("Mail account removed", "info");
      unseen = null;
      await refresh();
    } catch (e) {
      toast(String(e), "error");
    }
    working = false;
  }

  async function check() {
    working = true;
    try {
      const r = await api.mailUnseen();
      unseen = r;
      if (r?.ok === false) toast(r.message || r.error, "error");
    } catch (e) {
      toast(String(e), "error");
    }
    working = false;
  }
</script>

<div class="mx-auto max-w-2xl">
  <div class="section-title">Mail · IMAP</div>

  <div class="card divide-y divide-hairline">
    <div class="flex items-center gap-3 px-4 py-4">
      <div class="min-w-0 flex-1">
        <div class="truncate text-base font-semibold">
          {configured ? $mail.user : "No mail account"}
        </div>
        <div class="truncate text-xs text-dim">
          {configured
            ? `${$mail.host}:${$mail.port}`
            : "Add the inbox your Nextcloud provider gives you, or any other IMAP server."}
        </div>
      </div>
      {#if configured}
        <button class="btn-ghost" disabled={working} onclick={check}>Check</button>
        <button class="btn-ghost" disabled={working} onclick={remove}>Remove</button>
      {/if}
      <button class="btn-primary" disabled={working} onclick={() => (open ? (open = false) : edit())}>
        {open ? "Cancel" : configured ? "Change…" : "Add account"}
      </button>
    </div>

    {#if configured && $mail?.keyring_state && $mail.keyring_state !== "ok"}
      <div class="px-4 py-2.5 text-xs text-warning">
        The password is in the keyring, but the keyring is {$mail.keyring_state}. Mail stays quiet until it opens.
      </div>
    {/if}

    {#if unseen}
      <div class="px-4 py-3">
        <div class="text-xs text-dim">
          {unseen.ok === false ? unseen.message || unseen.error : `${unseen.unread || 0} unread`}
        </div>
        {#each unseen.list || [] as m}
          <div class="mt-2 min-w-0">
            <div class="truncate text-sm">{m.subject || "(no subject)"}</div>
            <div class="truncate text-xs text-dim">{m.from || ""}</div>
          </div>
        {/each}
      </div>
    {/if}

    {#if open}
      <div class="px-4 py-4">
        <div class="flex gap-2">
          <input class="input flex-1" placeholder="imap.example.org" bind:value={host} disabled={working} />
          <input class="input w-24" type="number" min="1" max="65535" bind:value={port} disabled={working} />
        </div>
        <input class="input mt-2 w-full" placeholder="you@example.org" bind:value={user} disabled={working} autocomplete="username" />
        <input
          class="input mt-2 w-full"
          type="password"
          placeholder="Password"
          bind:value={password}
          disabled={working}
          autocomplete="current-password"
          onkeydown={(e) => e.key === "Enter" && save()}
        />
        <label class="mt-3 flex items-center gap-2 text-xs text-dim">
          <input type="checkbox" checked={starttls} disabled={working} onchange={toggleStarttls} />
          STARTTLS (port 143 servers) instead of TLS
        </label>
        {#if error}<div class="mt-2 text-xs text-danger">{error}</div>{/if}
        <div class="mt-3 flex items-center justify-between gap-3">
          <div class="text-xs text-dim">
            The password goes into the system keyring. Only the server, user and port are written to
            <span class="font-mono">ewe.conf</span> — never the password.
          </div>
          <button class="btn-primary shrink-0" disabled={working} onclick={save}>
            {working ? "Signing in…" : "Sign in"}
          </button>
        </div>
      </div>
    {/if}
  </div>

  <div class="px-1 py-3 text-xs text-dim">
    The unread badge lives in the Control Center. Whether new mail also raises a notification is a
    per-machine setting, in Settings → User.
  </div>
</div>
