<script>
  import { onMount } from "svelte";
  import * as api from "../api";
  import { cloud, sync, busy, toast, fmtTime } from "../stores";
  import Toggle from "./ui/Toggle.svelte";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Row from "./ui/Row.svelte";
  import KV from "./ui/KV.svelte";
  import Alert from "./ui/Alert.svelte";
  import Empty from "./ui/Empty.svelte";
  import Icon from "./ui/Icon.svelte";

  let { refresh, syncNow } = $props();
  let me = $state({ name: "", ewe_version: "" });
  let confirmRestore = $state(false);

  onMount(async () => {
    try {
      me = await api.thisMachine();
    } catch {}
  });

  const conflict = $derived($sync && ($sync.error === "remote-newer" || $sync.error === "remote-exists"));
  const neverSynced = $derived($sync && !$sync.local_synced_at);

  async function backUp(force = false) {
    if ($busy) return;
    busy.set("sync");
    try {
      const r = await api.push(force);
      if (r.ok) toast(force ? "Replaced the account’s copy" : "Settings backed up", "success");
      else toast(r.message || r.error || "Couldn’t back up. Try again.", "error");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy.set("");
      refresh();
    }
  }

  async function restore() {
    confirmRestore = false;
    if ($busy) return;
    busy.set("restore");
    try {
      const r = await api.restore();
      if (r.ok) {
        toast(`Settings restored.${r.apps ? ` ${r.apps} apps are listed in Komble.` : ""}`, "success", 8000);
      } else toast(r.message || r.error || r.apply_stderr || "Couldn’t restore. Try again.", "error");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy.set("");
      refresh();
    }
  }

  async function setAuto(on) {
    try {
      await api.confSet("sync.enabled", on);
    } catch (e) {
      toast(String(e), "error");
    }
    refresh();
  }
</script>

{#snippet syncAction()}
  {#if neverSynced}
    <button class="ewe-btn ewe-btn--primary" disabled={!!$busy} onclick={() => backUp(false)}>
      <Icon name="arrowUp" />Back up this machine
    </button>
  {:else}
    <button class="ewe-btn ewe-btn--primary" disabled={!!$busy} onclick={syncNow}>
      {#if $busy === "sync"}<span class="ewe-spinner ewe-spinner--sm ewe-spinner--on-accent" aria-hidden="true"></span>Syncing…{:else}<Icon name="refresh" />Sync now{/if}
    </button>
  {/if}
{/snippet}

<Page
  title="This machine"
  desc="Settings sync: ewe.conf, the one file, backed up to your account and brought back on any machine."
  actions={$cloud?.signed_in && $sync ? syncAction : undefined}
>
  <Group title="This machine">
    <KV k="Name" v={me.name || "—"} />
    <KV k="ewe" v={me.ewe_version || "—"} mono />
  </Group>

  {#if !$cloud?.signed_in}
    <Group title="Settings sync">
      <Empty
        compact
        icon="cloudOff"
        title="Not signed in"
        desc="Sign in to back up ewe.conf to your account and bring it back on any machine."
      />
    </Group>
  {:else if !$sync}
    <Group title="Settings sync">
      <div class="ewe-row" aria-busy="true">
        <span class="ewe-spinner" aria-hidden="true"></span><span class="ewe-row__title">Reading…</span>
      </div>
    </Group>
  {:else}
    {#if conflict}
      <Alert tone="warning" title={$sync.error === "remote-exists" ? "A backup already exists" : "Newer settings in your account"}>
        {#if $sync.error === "remote-exists"}
          Your account has a backup and this machine never synced. Restore it first, or push anyway to replace it.
        {:else}
          {$sync.remote_machine || "Another machine"} saved newer settings. Restore them first, or push anyway to replace
          them.
        {/if}
        {#snippet actions()}
          {#if $sync.remote_machine || $sync.remote_modified}
            <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" disabled={!!$busy} onclick={() => (confirmRestore = true)}>Restore…</button>
          {/if}
          <button class="ewe-btn ewe-btn--sm ewe-btn--ghost" disabled={!!$busy} onclick={() => backUp(true)}>Push anyway</button>
        {/snippet}
      </Alert>
    {:else if $sync.error && $sync.error !== "nothing-synced"}
      <Alert tone="danger" title="Settings sync failed">{$sync.error}</Alert>
    {/if}

    <Group title="Settings sync">
      <KV k="Backup in your account">
        {#if $sync.remote_machine || $sync.remote_modified}
          Saved by {$sync.remote_machine || "?"} · {fmtTime($sync.remote_modified)}
        {:else}
          None yet
        {/if}
      </KV>
      <KV k="Last synced here">
        {#if $sync.local_synced_at}
          {fmtTime($sync.local_synced_at)}
          {#if $sync.in_sync}<span class="ewe-badge ewe-badge--success"><span class="ewe-badge__label">Up to date</span></span>{/if}
        {:else}
          Never
        {/if}
      </KV>
      <div class="ewe-list__divider"></div>
      <Row title="Auto-sync" sub="Back up whenever the file changes; bring it back at sign-in when the account’s copy is newer.">
        <Toggle label="Auto-sync" on={!!$sync.enabled} toggled={(v) => setAuto(v)} />
      </Row>
      {#if !conflict && ($sync.remote_machine || $sync.remote_modified)}
        <Row title="Restore from your account" sub="Replace this machine’s settings with the backup.">
          <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" disabled={!!$busy} onclick={() => (confirmRestore = true)}>Restore…</button>
        </Row>
      {/if}
    </Group>

    {#if $busy === "restore"}
      <Alert tone="info" title="Restoring…">The desktop re-themes and reloads as the file lands.</Alert>
    {/if}

    <p class="note">Apps recorded in the file appear in Komble → For you after a restore; nothing installs by itself.</p>
  {/if}
</Page>

{#if confirmRestore}
  <div class="scrim"></div>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="ewe-dialog is-floating"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="restore-title"
    aria-describedby="restore-desc"
    tabindex="-1"
    onkeydown={(e) => e.key === "Escape" && (confirmRestore = false)}
  >
    <div class="ewe-dialog__head">
      <span class="ewe-dialog__icon ewe-dialog__icon--danger"><Icon name="arrowDown" /></span>
      <div class="ewe-dialog__titles">
        <h2 class="ewe-dialog__title" id="restore-title">Restore settings from {$sync?.remote_machine || "another machine"}?</h2>
        <p class="ewe-dialog__desc" id="restore-desc">
          This machine’s settings are replaced with the copy in your account. Your current file is kept as a backup
          next to it.
        </p>
      </div>
    </div>
    <div class="ewe-dialog__foot">
      <!-- svelte-ignore a11y_autofocus -->
      <button class="ewe-btn ewe-btn--ghost" autofocus onclick={() => (confirmRestore = false)}>Cancel</button>
      <button class="ewe-btn ewe-btn--danger" onclick={restore}>Restore settings</button>
    </div>
  </div>
{/if}
