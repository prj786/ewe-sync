<script>
  // Settings & apps — this computer's desktop as one file (ewe.conf): the
  // look, the bar, displays, keyboard and the apps you use, kept in your
  // account. When two computers changed it, the decision comes first, under
  // the title, as two equal choices: take the other computer's settings, or
  // keep this computer's (which replaces the account's copy). The page
  // action steps aside while that question is open.
  import { onMount } from "svelte";
  import * as api from "../api";
  import { route, cloud, sync, busy, toast, fmtTime, conflictOf, settingsError } from "../stores";
  import Toggle from "./ui/Toggle.svelte";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Row from "./ui/Row.svelte";
  import KV from "./ui/KV.svelte";
  import Alert from "./ui/Alert.svelte";
  import Empty from "./ui/Empty.svelte";
  import Dialog from "./ui/Dialog.svelte";
  import SkelRows from "./ui/SkelRows.svelte";
  import Icon from "./ui/Icon.svelte";

  let { refresh, syncNow } = $props();
  let me = $state({ name: "", ewe_version: "" });
  let confirmRestore = $state(false);
  let showDetails = $state(false);

  onMount(async () => {
    try {
      me = await api.thisMachine();
    } catch {}
  });

  const conflictKind = $derived(conflictOf($sync));
  const conflict = $derived(!!conflictKind);
  const neverSynced = $derived($sync && !$sync.local_synced_at);
  // true/false from the machine id in the backup's stamp; null when the stamp
  // is older than machine ids and only a hostname (which two computers may
  // share) says who saved it
  const mine = $derived($sync?.remote_is_this_machine);
  const other = $derived($sync?.remote_machine || "");
  const hasRemote = $derived(!!($sync?.remote_machine || $sync?.remote_modified));
  const savedBy = $derived(mine === true ? `this computer (${other || me.name || "?"})` : other || "another computer");
  // whose settings the "use" choice brings: the other computer's, by name
  const useLabel = $derived(mine === true || !other ? "Use the account’s copy…" : `Use ${other}’s settings…`);
  const syncError = $derived($sync?.error && $sync.error !== "nothing-synced" && $sync.error !== "not-signed-in" ? $sync.error : "");
  const syncErr = $derived(settingsError(syncError));

  async function backUp(force = false) {
    if ($busy) return;
    busy.set("sync");
    try {
      const r = await api.push(force);
      if (r.ok) toast(force ? "Kept this computer’s settings. They are in your account now." : "Settings backed up", "success");
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
    <button class="ewe-btn ewe-btn--primary" aria-busy={$busy === "sync"} aria-disabled={!!$busy} onclick={() => backUp(false)}>
      {#if $busy === "sync"}<span class="ewe-spinner ewe-spinner--sm ewe-spinner--on-accent" aria-hidden="true"></span>Backing up…{:else}<Icon name="arrowUp" />Back up this computer{/if}
    </button>
  {:else}
    <button class="ewe-btn ewe-btn--primary" aria-busy={$busy === "sync"} aria-disabled={!!$busy} onclick={syncNow}>
      {#if $busy === "sync"}<span class="ewe-spinner ewe-spinner--sm ewe-spinner--on-accent" aria-hidden="true"></span>Syncing…{:else}<Icon name="refresh" />Sync now{/if}
    </button>
  {/if}
{/snippet}

<Page
  title="Settings & apps"
  desc="Your look, bar, displays, keyboard and the apps you use, kept in your account."
  actions={$cloud?.signed_in && $sync && !conflict ? syncAction : undefined}
>
  {#if $cloud?.signed_in && $sync}
    {#if conflict}
      <Alert
        tone="warning"
        title={conflictKind === "remote-exists"
          ? "Your account already has settings"
          : mine === true
            ? "Your account has a different copy of these settings"
            : "Two computers changed your settings"}
      >
        {#if conflictKind === "remote-exists"}
          Saved by {savedBy}{$sync.remote_modified ? ` ${fmtTime($sync.remote_modified)}` : ""}. This computer has never
          synced. Choose which settings to keep; nothing syncs until you do.
        {:else if mine === true}
          This computer saved it{$sync.remote_modified ? ` ${fmtTime($sync.remote_modified)}` : ""}, but it isn’t the copy
          this computer last synced. Choose which one to keep.
        {:else if mine === null && other && other === me.name}
          “{other}” saved newer settings{$sync.remote_modified ? ` ${fmtTime($sync.remote_modified)}` : ""}. That is this
          computer’s name, but another computer may share it. Choose which settings to keep.
        {:else}
          {other || "Another computer"} saved newer settings{$sync.remote_modified ? ` ${fmtTime($sync.remote_modified)}` : ""}.
          Choose which ones this computer keeps; nothing syncs until you do.
        {/if}
        {#snippet actions()}
          {#if hasRemote}
            <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" aria-disabled={!!$busy} onclick={() => !$busy && (confirmRestore = true)}>{useLabel}</button>
          {/if}
          <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" aria-disabled={!!$busy} onclick={() => backUp(true)}>Keep this computer’s settings</button>
        {/snippet}
      </Alert>
    {:else if syncError}
      <Alert tone="danger" title={`Your settings didn’t sync: ${syncErr.reason}`}>
        {syncErr.hint}
        {#if showDetails}<pre class="error-log">{syncError}</pre>{/if}
        {#snippet actions()}
          <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" aria-disabled={!!$busy} onclick={syncNow}>Try again</button>
          {#if syncError === "no-provider"}
            <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" onclick={() => route.set("account")}>Open Account</button>
          {:else}
            <button class="ewe-btn ewe-btn--sm ewe-btn--ghost" aria-expanded={showDetails} onclick={() => (showDetails = !showDetails)}>
              {showDetails ? "Hide details" : "Details"}
            </button>
          {/if}
        {/snippet}
      </Alert>
    {/if}
    {#if $busy === "restore"}
      <Alert tone="info" title="Restoring…">The desktop re-themes and reloads as the settings land.</Alert>
    {/if}
  {/if}

  <Group title="This computer">
    <KV k="Name" v={me.name || "—"} />
    <KV k="ewe version" v={me.ewe_version || "—"} mono />
  </Group>

  {#if !$cloud?.signed_in}
    <Group title="In your account">
      <Empty compact icon="cloudOff" title="Not signed in" desc="Sign in to keep your settings and apps in your account and bring them back on any computer.">
        <button class="ewe-btn ewe-btn--secondary ewe-btn--sm" onclick={() => route.set("account")}>Sign in</button>
      </Empty>
    </Group>
  {:else if !$sync}
    <Group title="In your account">
      <SkelRows rows={3} tile={false} label="Reading your settings" />
    </Group>
  {:else}
    <Group title="In your account">
      <KV k="Backup">
        {#if hasRemote}
          Saved by {savedBy}{$sync.remote_modified ? ` · ${fmtTime($sync.remote_modified)}` : ""}
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
      <Row title="Auto-sync" sub="Back up whenever something changes; bring the account’s copy back at sign-in when it’s newer.">
        <Toggle label="Auto-sync" on={!!$sync.enabled} toggled={(v) => setAuto(v)} />
      </Row>
      {#if !conflict && hasRemote}
        <Row title="Restore from your account" sub="Replace this computer’s settings with the copy in your account.">
          <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" aria-disabled={!!$busy} onclick={() => !$busy && (confirmRestore = true)}>Restore…</button>
        </Row>
      {/if}
    </Group>

    <p class="note">Apps in your settings appear in Komble → For you after a restore. Nothing installs by itself.</p>
  {/if}
</Page>

{#if confirmRestore}
  <Dialog
    title={mine === true || !other ? "Restore settings from your account?" : `Restore settings from ${other}?`}
    icon="arrowDown"
    tone="accent"
    confirm="Restore settings"
    onConfirm={restore}
    onCancel={() => (confirmRestore = false)}
  >
    This computer’s settings are replaced with the copy in your account. The current ones are kept as a backup next to
    it.
  </Dialog>
{/if}
