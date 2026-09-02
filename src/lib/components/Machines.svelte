<script>
  import { onMount } from "svelte";
  import * as api from "../api";
  import { cloud, sync, machines, toast, fmtTime } from "../stores";

  let loading = $state(false);
  let me = $state("");

  async function load() {
    if (!$cloud?.signed_in) return;
    loading = true;
    try {
      machines.set(await api.machinesList());
    } catch (e) {
      if (String(e) !== "not-signed-in") toast(String(e), "error");
    } finally {
      loading = false;
    }
  }
  onMount(async () => {
    try {
      me = (await api.thisMachine()).name;
    } catch {}
    load();
  });
  $effect(() => {
    if ($cloud?.signed_in) load();
  });
</script>

<div class="mx-auto max-w-2xl">
  <div class="section-title">Your flock</div>
  {#if !$cloud?.signed_in}
    <div class="card px-4 py-4 text-sm text-zinc-500 dark:text-zinc-400">Sign in to see the machines that share your account.</div>
  {:else}
    <div class="card divide-y divide-zinc-200 dark:divide-zinc-700/60">
      {#if loading && !$machines.length}
        <div class="px-4 py-4 text-sm text-zinc-500 dark:text-zinc-400">Reading…</div>
      {:else if !$machines.length}
        <div class="px-4 py-4 text-sm text-zinc-500 dark:text-zinc-400">No machine has backed up yet. This one joins the flock at its first backup.</div>
      {:else}
        {#each $machines as m (m.name)}
          <div class="kv">
            <div class="min-w-0">
              <div class="font-medium">
                {m.name}
                {#if m.name === me}<span class="ml-1 text-xs text-zinc-500 dark:text-zinc-400">this machine</span>{/if}
                {#if $sync?.remote_machine === m.name}<span class="ml-1 rounded px-1.5 py-0.5 text-[11px] text-white" style="background: var(--accent)">saved the backup</span>{/if}
              </div>
              <div class="text-xs text-zinc-500 dark:text-zinc-400">
                ewe {m.ewe_version || "?"} · {m.apps_count ?? 0} apps · seen {fmtTime(m.last_seen)}
              </div>
            </div>
          </div>
        {/each}
      {/if}
    </div>
    <div class="mt-2 flex items-center justify-between text-xs text-zinc-500 dark:text-zinc-400">
      <span>One file per account: the newest backup wins, whichever machine saved it. Per-machine backups are a later option.</span>
      <button class="underline" onclick={load}>Refresh</button>
    </div>
  {/if}
</div>
