<script>
  import { onMount } from "svelte";
  import * as api from "../api";
  import { cloud } from "../stores";

  let folders = $state([]);
  onMount(async () => {
    try {
      const v = await api.confGet("sync.folders");
      folders = Array.isArray(v) ? v : [];
    } catch {
      folders = [];
    }
  });
</script>

<div class="mx-auto max-w-2xl">
  <div class="section-title">Folders</div>
  <div class="card divide-y divide-zinc-200 dark:divide-zinc-700/60">
    <div class="px-4 py-4 text-sm">
      <div class="font-medium">Folder sync is coming in the next release.</div>
      <div class="mt-1 text-xs text-zinc-500 dark:text-zinc-400">
        Pick which folders sync with your account, two-way or one-way, on change or on a timer, and resolve conflicts here. Powered by the Nextcloud sync engine (<code>nextcloudcmd</code>), driven by Flock.
      </div>
    </div>
    {#if !$cloud?.signed_in}
      <div class="px-4 py-3 text-xs text-zinc-500 dark:text-zinc-400">Sign in first.</div>
    {:else if folders.length}
      {#each folders as f}
        <div class="kv">
          <span class="font-mono text-xs">{f.local || "~/Nextcloud"} ↔ {f.remote || "/"}</span>
          <span class="text-xs text-zinc-500 dark:text-zinc-400">{f.mode || "two-way"} · {f.trigger || "change"}</span>
        </div>
      {/each}
    {:else}
      <div class="px-4 py-3 text-xs text-zinc-500 dark:text-zinc-400">No folder pairs are defined in <code>ewe.conf</code> yet.</div>
    {/if}
  </div>
</div>
