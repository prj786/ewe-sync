<script>
  // Mail — any number of IMAP/SMTP mailboxes (RFC-005, ewe Mail). Not tied to
  // the ewe account: the inbox a Nextcloud provider gives you, one you host,
  // a work account. Gmail is better through the Google page.
  //
  // This pane owns adding, changing and removing accounts (ewe Mail and the
  // shell plugin only read them). The new-mail NOTIFICATION switch is the
  // Mail plugin's own setting (its bell, Komble → Plugins → Mail → Options):
  // a preference about this desktop, not part of an account.
  // Adding and changing happen in a sheet (the same grammar as a folder),
  // email first: the address fills in the servers, which stay editable under
  // "Servers". Removing asks first, naming the account.
  import { onMount } from "svelte";
  import * as api from "../api";
  import { mailAccounts, route, toast } from "../stores";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Row from "./ui/Row.svelte";
  import Alert from "./ui/Alert.svelte";
  import Empty from "./ui/Empty.svelte";
  import Menu from "./ui/Menu.svelte";
  import Sheet from "./ui/Sheet.svelte";
  import Dialog from "./ui/Dialog.svelte";
  import Icon from "./ui/Icon.svelte";

  let { refresh } = $props();

  const accounts = $derived($mailAccounts || []);

  // the sheet
  let open = $state(false);
  let editing = $state(null); // the account being changed, or null when adding
  let email = $state("");
  let password = $state("");
  let name = $state("");
  let host = $state("");
  let port = $state(993);
  let security = $state("tls");
  let smtpHost = $state("");
  let smtpPort = $state(465);
  let smtpSecurity = $state("tls");
  let smtpUser = $state("");
  let smtpPassword = $state("");
  let userName = $state(""); // the sign-in name on the incoming server
  let showServers = $state(false);
  let touched = $state(false); // the servers were edited by hand: autoconfig stops overwriting
  let note = $state("");
  let finding = $state(false);
  let working = $state(false);
  let error = $state("");

  let loading = $state(true);
  let confirmRemove = $state(null);
  let checking = $state(false);
  let unseen = $state(null);

  const isGmail = $derived(/@(gmail|googlemail)\.com$/i.test(email.trim()));
  const validEmail = $derived(/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email.trim()));

  async function load() {
    try {
      const r = await api.mailAccounts();
      mailAccounts.set(r?.ok === false ? [] : r?.accounts || []);
    } catch {
      mailAccounts.set([]);
    }
    loading = false;
  }
  onMount(load);

  async function reload() {
    await load();
    await refresh?.();
  }

  const label = (a) => a.email || a.user;
  const servers = (a) => (a.host === a.smtp_host ? a.host : `${a.host} · ${a.smtp_host}`);
  function sub(a) {
    const bits = [];
    if (a.name) bits.push(a.name);
    bits.push(servers(a));
    return bits.join(" · ");
  }

  function reset() {
    password = "";
    smtpPassword = "";
    error = "";
    note = "";
    touched = false;
    showServers = false;
  }

  function add() {
    editing = null;
    email = "";
    name = "";
    host = "";
    port = 993;
    security = "tls";
    smtpHost = "";
    smtpPort = 465;
    smtpSecurity = "tls";
    smtpUser = "";
    userName = "";
    reset();
    open = true;
  }

  function edit(a) {
    editing = a;
    email = a.email || "";
    name = a.name || "";
    host = a.host;
    port = Number(a.port) || 993;
    security = a.starttls ? "starttls" : "tls";
    smtpHost = a.smtp_host;
    smtpPort = Number(a.smtp_port) || 465;
    smtpSecurity = a.smtp_security || "tls";
    smtpUser = a.smtp_user && a.smtp_user !== a.user ? a.smtp_user : "";
    userName = a.user;
    reset();
    touched = true;
    open = true;
  }

  // the incoming port follows the security choice, unless it was typed
  function pickSecurity(which) {
    security = which;
    touched = true;
    if (which === "starttls" && Number(port) === 993) port = 143;
    if (which === "tls" && Number(port) === 143) port = 993;
  }
  function pickSmtpSecurity(which) {
    smtpSecurity = which;
    touched = true;
    if (which === "starttls" && Number(smtpPort) === 465) smtpPort = 587;
    if (which === "tls" && Number(smtpPort) === 587) smtpPort = 465;
  }

  // ask autoconfig for the servers once the address looks complete
  let timer;
  let asked = "";
  function onEmail() {
    clearTimeout(timer);
    if (editing || !validEmail || touched) return;
    const addr = email.trim();
    timer = setTimeout(() => find(addr), 450);
  }
  async function find(addr) {
    if (addr === asked) return;
    asked = addr;
    finding = true;
    try {
      const r = await api.mailAutoconfig(addr);
      if (addr !== email.trim() || touched) return;
      if (r?.ok) {
        note = r.note || "";
        host = r.imap?.host || "";
        port = r.imap?.port || 993;
        security = r.imap?.security || "tls";
        smtpHost = r.smtp?.host || "";
        smtpPort = r.smtp?.port || 465;
        smtpSecurity = r.smtp?.security || "tls";
        userName = r.user === "local" ? addr.split("@")[0] : addr;
        if (!host) showServers = true; // nothing to fill in: they are typed by hand
      }
    } catch {
      /* the fields stay empty; they can be typed */
    }
    finding = false;
  }

  async function save() {
    error = "";
    if (!validEmail || !password) {
      error = "Enter the email address and the password.";
      return;
    }
    if (!host.trim() || !smtpHost.trim()) {
      showServers = true;
      error = "Enter the incoming and the outgoing server.";
      return;
    }
    working = true;
    try {
      const addr = email.trim();
      const extra = {
        email: addr,
        name: name.trim(),
        smtp_host: smtpHost.trim(),
        smtp_port: Number(smtpPort) || 465,
        smtp_security: smtpSecurity,
        smtp_user: smtpUser.trim() || (userName.trim() || addr),
        smtp_password: smtpPassword || ""
      };
      const r = await api.mailLogin(
        host.trim(),
        Number(port) || 993,
        userName.trim() || addr,
        password,
        security === "starttls",
        extra
      );
      if (r?.ok) {
        const was = editing;
        reset();
        open = false;
        toast(was ? `Updated **${addr}**` : `Added **${addr}**`, "success");
        await reload();
      } else {
        // ewe-mail tests both servers before storing, so this is a server talking
        error = r?.message || r?.error || "Couldn’t sign in to the mail server. Check the details and try again.";
        if (/smtp|outgoing/i.test(`${r?.error} ${r?.message}`)) showServers = true;
      }
    } catch (e) {
      error = String(e);
    }
    working = false;
  }

  async function remove() {
    const a = confirmRemove;
    confirmRemove = null;
    working = true;
    try {
      const r = await api.mailLogout(a.id);
      if (r && r.ok === false) toast(r.message || r.error, "error");
      else toast(`Removed **${label(a)}**`, "info");
      unseen = null;
      await reload();
    } catch (e) {
      toast(String(e), "error");
    }
    working = false;
  }

  async function check() {
    checking = true;
    try {
      const r = await api.mailUnseen();
      unseen = r;
      if (r?.ok === false) toast(r.message || r.error, "error");
    } catch (e) {
      toast(String(e), "error");
    }
    checking = false;
  }
</script>

<Page
  title="Mail"
  desc="Any mailbox: the inbox your Nextcloud provider gives you, one you host, or a work account. Add as many as you like."
>
  <Group title="Mail accounts">
    {#snippet action()}
      {#if accounts.length}
        <button class="ewe-btn ewe-btn--secondary ewe-btn--sm" onclick={add}><Icon name="plus" />Add account</button>
      {/if}
    {/snippet}
    {#if loading}
      <Row icon="mail" title="Looking…" />
    {:else if accounts.length}
      {#each accounts as a, i (a.id)}
        {#if i > 0}<div class="ewe-list__divider"></div>{/if}
        <Row icon="mail" title={label(a)} sub={sub(a)}>
          {#if !a.keyring}
            <span class="ewe-badge ewe-badge--warning"><span class="ewe-badge__label">Password missing</span></span>
          {/if}
          <Menu
            label={`${label(a)} actions`}
            items={[
              { label: "Change…", icon: "sliders", disabled: working, run: () => edit(a) },
              { divider: true },
              { label: "Remove…", icon: "x", danger: true, disabled: working, run: () => (confirmRemove = a) }
            ]}
          />
        </Row>
      {/each}
    {:else}
      <Empty
        compact
        icon="mail"
        title="No mail account"
        desc="Add the inbox your Nextcloud provider gives you, or any other mail server."
      >
        <button class="ewe-btn ewe-btn--secondary ewe-btn--sm" onclick={add}><Icon name="plus" />Add account</button>
      </Empty>
    {/if}
  </Group>

  {#if accounts.length}
    <Group title="Unread">
      {#if unseen && unseen.ok !== false}
        {#each unseen.list || [] as m, i}
          {#if i > 0}<div class="ewe-list__divider"></div>{/if}
          <Row dense title={m.subject || "(no subject)"} sub={m.from || ""} />
        {:else}
          <Empty compact icon="success" title="No unread mail" desc="Everything has been read." />
        {/each}
      {:else if unseen}
        <Alert tone="danger" title="Couldn’t check mail">{unseen.message || unseen.error}</Alert>
      {:else}
        <Row title="Check that your accounts answer" sub="Counts the unread mail in every account.">
          <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" aria-busy={checking} aria-disabled={checking} onclick={() => !checking && check()}>
            {#if checking}<span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>Checking…{:else}Check mail{/if}
          </button>
        </Row>
      {/if}
      {#snippet action()}
        {#if unseen}
          <button class="ewe-btn ewe-btn--ghost ewe-btn--sm" onclick={check}>Check again</button>
        {/if}
      {/snippet}
    </Group>
  {/if}

  <p class="note">
    The unread count of every account shows in Quick settings, and ewe Mail reads and sends from them. Whether new mail
    also notifies you is the bell on the Inbox page in Quick settings.
  </p>
</Page>

{#if open}
  <Sheet title={editing ? "Change mail account" : "Add a mail account"} onClose={() => (open = false)}>
    <label class="ewe-field">
      <span class="ewe-field__label">Email</span>
      <input
        class="ewe-input"
        type="email"
        placeholder="you@example.org"
        bind:value={email}
        oninput={onEmail}
        disabled={working || !!editing}
        autocomplete="email"
      />
    </label>
    {#if isGmail}
      <Alert tone="info" title="Gmail works best through the Google page">
        No app password needed there, and ewe Mail can send.
        {#snippet actions()}
          <button class="ewe-btn ewe-btn--secondary ewe-btn--sm" onclick={() => { open = false; route.set("google"); }}>
            Open the Google page
          </button>
        {/snippet}
      </Alert>
    {/if}
    <label class="ewe-field">
      <span class="ewe-field__label">Password</span>
      <input
        class="ewe-input"
        class:is-error={!!error}
        type="password"
        bind:value={password}
        disabled={working}
        autocomplete="current-password"
        onkeydown={(e) => e.key === "Enter" && !working && save()}
      />
      {#if error}
        <span class="ewe-field__helper ewe-field__helper--error"><Icon name="alert" />{error}</span>
      {:else}
        <span class="ewe-field__helper">
          The password goes into the system keyring. Only the servers and the user sync with your settings.
        </span>
      {/if}
    </label>
    {#if note}
      <Alert tone="info">{note}</Alert>
    {/if}
    <label class="ewe-field">
      <span class="ewe-field__label">Name</span>
      <input class="ewe-input" placeholder="Shown on mail you send" bind:value={name} disabled={working} autocomplete="name" />
    </label>

    <button
      type="button"
      class="ewe-link servers-toggle"
      aria-expanded={showServers}
      onclick={() => (showServers = !showServers)}
    >
      <Icon name={showServers ? "caretDown" : "caretRight"} />Servers{#if finding}<span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>{:else if host && !showServers}<span class="servers-sum">{host}</span>{/if}
    </button>
    {#if showServers}
      <div class="form-grid">
        <label class="ewe-field form-grid__wide">
          <span class="ewe-field__label">Incoming server</span>
          <input class="ewe-input" placeholder="imap.example.org" bind:value={host} oninput={() => (touched = true)} disabled={working} />
        </label>
        <label class="ewe-field">
          <span class="ewe-field__label">Port</span>
          <input class="ewe-input mono-input" type="number" min="1" max="65535" bind:value={port} oninput={() => (touched = true)} disabled={working} />
        </label>
      </div>
      <label class="ewe-field">
        <span class="ewe-field__label">Incoming security</span>
        <select class="ewe-input" value={security} onchange={(e) => pickSecurity(e.currentTarget.value)} disabled={working}>
          <option value="tls">TLS</option>
          <option value="starttls">STARTTLS</option>
        </select>
      </label>
      <label class="ewe-field">
        <span class="ewe-field__label">User</span>
        <input class="ewe-input" placeholder={email || "you@example.org"} bind:value={userName} oninput={() => (touched = true)} disabled={working} autocomplete="username" />
      </label>
      <div class="form-grid">
        <label class="ewe-field form-grid__wide">
          <span class="ewe-field__label">Outgoing server</span>
          <input class="ewe-input" placeholder="smtp.example.org" bind:value={smtpHost} oninput={() => (touched = true)} disabled={working} />
        </label>
        <label class="ewe-field">
          <span class="ewe-field__label">Port</span>
          <input class="ewe-input mono-input" type="number" min="1" max="65535" bind:value={smtpPort} oninput={() => (touched = true)} disabled={working} />
        </label>
      </div>
      <label class="ewe-field">
        <span class="ewe-field__label">Outgoing security</span>
        <select class="ewe-input" value={smtpSecurity} onchange={(e) => pickSmtpSecurity(e.currentTarget.value)} disabled={working}>
          <option value="tls">TLS</option>
          <option value="starttls">STARTTLS</option>
        </select>
      </label>
      <label class="ewe-field">
        <span class="ewe-field__label">Outgoing user</span>
        <input class="ewe-input" placeholder="Same as the incoming user" bind:value={smtpUser} disabled={working} />
      </label>
      <label class="ewe-field">
        <span class="ewe-field__label">Outgoing password</span>
        <input class="ewe-input" type="password" placeholder="Same as above" bind:value={smtpPassword} disabled={working} autocomplete="off" />
        <span class="ewe-field__helper">Only if the outgoing server wants a different one.</span>
      </label>
    {/if}
    {#snippet foot()}
      <button class="ewe-btn ewe-btn--ghost" disabled={working} onclick={() => (open = false)}>Cancel</button>
      <button class="ewe-btn ewe-btn--primary" aria-busy={working} aria-disabled={working} onclick={() => !working && save()}>
        {#if working}<span class="ewe-spinner ewe-spinner--sm ewe-spinner--on-accent" aria-hidden="true"></span>Signing in…{:else}{editing ? "Save" : "Add"}{/if}
      </button>
    {/snippet}
  </Sheet>
{/if}

{#if confirmRemove}
  <Dialog
    title={`Remove ${label(confirmRemove)}?`}
    icon="x"
    tone="danger"
    confirm="Remove account"
    onConfirm={remove}
    onCancel={() => (confirmRemove = null)}
  >
    The account leaves ewe Mail and Quick settings, here and on your other computers once your settings sync, and its
    password leaves this computer’s keyring. The mailbox itself stays on the server.
  </Dialog>
{/if}

<style>
  .servers-toggle {
    display: inline-flex;
    align-items: center;
    gap: var(--space-s);
    align-self: flex-start;
  }
  .servers-sum {
    color: var(--text-muted);
    font-weight: normal;
  }
</style>
