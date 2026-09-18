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
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Row from "./ui/Row.svelte";
  import Alert from "./ui/Alert.svelte";
  import Empty from "./ui/Empty.svelte";
  import Check from "./ui/Check.svelte";
  import Icon from "./ui/Icon.svelte";

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
      error = "Enter the server, the user and the password.";
      return;
    }
    working = true;
    try {
      const r = await api.mailLogin(host.trim(), Number(port) || 993, user.trim(), password, starttls);
      if (r?.ok) {
        password = "";
        open = false;
        toast(`Added **${user.trim()}**`, "success");
        await refresh();
      } else {
        // ewe-mail tests the login before storing, so this is the server talking
        error = r?.message || r?.error || "Couldn’t sign in to the mail server. Check the details and try again.";
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
      else toast("Removed the mail account", "info");
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

{#snippet addAction()}
  <button class="ewe-btn ewe-btn--primary" disabled={working} onclick={edit}><Icon name="plus" />Add account</button>
{/snippet}

<Page
  title="Mail"
  desc="Any IMAP mailbox: the inbox your Nextcloud provider gives you, one you host, or a work account."
  actions={!configured && !open ? addAction : undefined}
>
  <Group title="Mailbox · IMAP">
    {#if configured}
      <Row icon="mail" title={$mail.user} sub={`${$mail.host}:${$mail.port}`}>
        <button class="ewe-btn ewe-btn--sm ewe-btn--ghost" disabled={working} onclick={check}>
          <Icon name="refresh" />Check mail
        </button>
        <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" disabled={working} onclick={() => (open ? (open = false) : edit())}>
          {open ? "Cancel" : "Change…"}
        </button>
        <button class="ewe-btn ewe-btn--sm ewe-btn--ghost danger-text" disabled={working} onclick={remove}>Remove</button>
      </Row>
    {:else}
      <Empty
        compact
        icon="inbox"
        title="No mail account"
        desc="Add the inbox your Nextcloud provider gives you, or any other IMAP server."
      />
    {/if}
  </Group>

  {#if configured && $mail?.keyring_state && $mail.keyring_state !== "ok"}
    <Alert tone="warning" title="The keyring is {$mail.keyring_state}">
      The password is in the keyring. Mail stays quiet until the keyring opens.
    </Alert>
  {/if}

  {#if unseen}
    {#if unseen.ok === false}
      <Alert tone="danger" title="Couldn’t check mail">{unseen.message || unseen.error}</Alert>
    {:else}
      <Group title={`Unread · ${unseen.unread || 0}`}>
        {#each unseen.list || [] as m}
          <Row dense title={m.subject || "(no subject)"} sub={m.from || ""} />
        {:else}
          <Empty compact icon="inbox" title="No unread mail" />
        {/each}
      </Group>
    {/if}
  {/if}

  {#if open}
    <Group title={configured ? "Change account" : "Add account"} well={false}>
      <div class="ewe-card">
        <div class="form-grid">
          <label class="ewe-field form-grid__wide">
            <span class="ewe-field__label">Server</span>
            <input class="ewe-input" placeholder="imap.example.org" bind:value={host} disabled={working} />
          </label>
          <label class="ewe-field">
            <span class="ewe-field__label">Port</span>
            <input class="ewe-input mono-input" type="number" min="1" max="65535" bind:value={port} disabled={working} />
          </label>
        </div>
        <label class="ewe-field">
          <span class="ewe-field__label">User</span>
          <input class="ewe-input" placeholder="you@example.org" bind:value={user} disabled={working} autocomplete="username" />
        </label>
        <label class="ewe-field">
          <span class="ewe-field__label">Password</span>
          <input
            class="ewe-input"
            class:is-error={!!error}
            type="password"
            bind:value={password}
            disabled={working}
            autocomplete="current-password"
            onkeydown={(e) => e.key === "Enter" && save()}
          />
          {#if error}
            <span class="ewe-field__helper ewe-field__helper--error"><Icon name="alert" />{error}</span>
          {:else}
            <span class="ewe-field__helper">
              The password goes into the system keyring. Only the server, user and port are written to ewe.conf.
            </span>
          {/if}
        </label>
        <Check
          checked={starttls}
          disabled={working}
          label="Use STARTTLS"
          desc="For servers on port 143, instead of TLS on 993."
          toggled={toggleStarttls}
        />
        <div class="ewe-card__foot">
          <button class="ewe-btn ewe-btn--ghost" disabled={working} onclick={() => (open = false)}>Cancel</button>
          <button class="ewe-btn ewe-btn--primary" disabled={working} onclick={save}>
            {#if working}<span class="ewe-spinner ewe-spinner--sm ewe-spinner--on-accent" aria-hidden="true"></span>Signing in…{:else}Sign in{/if}
          </button>
        </div>
      </div>
    </Group>
  {/if}

  <p class="note">
    The unread badge lives in Quick settings. Whether new mail also raises a notification is a per-machine
    setting, in Settings → User.
  </p>
</Page>
