<script>
  import { onMount } from "svelte";
  import * as api from "../api";
  import { cloud, sync, busy, toast, fmtTime } from "../stores";
  import Toggle from "./ui/Toggle.svelte";

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
      if (r.ok) toast(force ? "Pushed over the account's copy" : "Settings backed up", "success");
      else toast(r.message || r.error || "Push failed", "error");
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
        toast(`Restored. ${r.apps ? r.apps + " apps are listed for Komble." : ""}`, "success", 8000);
      } else toast(r.message || r.error || r.apply_stderr || "Restore failed", "error");
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

<div class="mx-auto max-w-2xl">
  <div class="section-title">This machine</div>
  <div class="card divide-y divide-hairline">
    <div class="kv"><span class="text-dim">Name</span><span class="font-medium">{me.name || "—"}</span></div>
    <div class="kv"><span class="text-dim">ewe</span><span>{me.ewe_version || "—"}</span></div>
  </div>

  <div class="section-title">Settings sync · the one file</div>
  {#if !$cloud?.signed_in}
    <div class="card px-4 py-4 text-sm text-dim">Sign in to back up <code>ewe.conf</code> to your account and bring it back on any machine.</div>
  {:else if !$sync}
    <div class="card px-4 py-4 text-sm text-dim">Reading…</div>
  {:else}
    <div class="card divide-y divide-hairline">
      {#if conflict}
        <div class="px-4 py-3 text-xs text-warning">
          {#if $sync.error === "remote-exists"}
            A backup already exists in your account and this machine never synced. Restore it first — or push anyway to replace it.
          {:else}
            Another machine ({$sync.remote_machine || "unknown"}) saved newer settings. Restore them first — or push anyway to replace them.
          {/if}
        </div>
      {:else if $sync.error && $sync.error !== "nothing-synced"}
        <div class="px-4 py-3 text-xs text-danger">{$sync.error}</div>
      {/if}
      <div class="kv">
        <span class="text-dim">Backup in your account</span>
        <span class="text-right">
          {#if $sync.remote_machine || $sync.remote_modified}
            saved by <span class="font-medium">{$sync.remote_machine || "?"}</span> · {fmtTime($sync.remote_modified)}
          {:else}
            none yet
          {/if}
        </span>
      </div>
      <div class="kv">
        <span class="text-dim">This machine last synced</span>
        <span class="text-right">
          {#if $sync.local_synced_at}{fmtTime($sync.local_synced_at)}{#if $sync.in_sync} · up to date{/if}{:else}never{/if}
        </span>
      </div>
      <div class="kv">
        <div>
          <div class="font-medium">Auto-sync</div>
          <div class="text-xs text-dim">Back up whenever the file changes; pull at login when the account is newer.</div>
        </div>
        <Toggle on={!!$sync.enabled} toggled={() => setAuto(!$sync.enabled)} />
      </div>
      <div class="flex flex-wrap gap-2 px-4 py-3">
        {#if neverSynced}
          <button class="btn-primary" disabled={!!$busy} onclick={() => backUp(false)}>Back up this machine</button>
        {:else}
          <button class="btn-primary" disabled={!!$busy} onclick={syncNow}>Sync now</button>
        {/if}
        {#if conflict}
          <button class="btn-ghost" disabled={!!$busy} onclick={() => backUp(true)}>Push anyway</button>
        {/if}
        {#if $sync.remote_machine || $sync.remote_modified}
          <button class="btn-ghost" disabled={!!$busy} onclick={() => (confirmRestore = true)}>Restore…</button>
        {/if}
      </div>
      {#if confirmRestore}
        <div class="px-4 py-3">
          <div class="text-sm">Replace this machine's settings with the copy saved by <b>{$sync.remote_machine || "another machine"}</b>? Your current file is kept as a backup next to it.</div>
          <div class="mt-2 flex gap-2">
            <button class="btn-danger" onclick={restore}>Restore</button>
            <button class="btn-ghost" onclick={() => (confirmRestore = false)}>Cancel</button>
          </div>
        </div>
      {/if}
      {#if $busy === "restore"}
        <div class="px-4 py-3 text-xs text-dim">Restoring — the desktop re-themes and reloads as the file lands…</div>
      {/if}
    </div>
    <div class="mt-2 text-xs text-dim">
      Apps recorded in the file appear in Komble → For you after a restore; nothing installs by itself.
    </div>
  {/if}
</div>
