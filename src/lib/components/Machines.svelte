<script>
  import { onMount } from "svelte";
  import * as api from "../api";
  import { cloud, sync, machines, toast, fmtTime } from "../stores";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Row from "./ui/Row.svelte";
  import Empty from "./ui/Empty.svelte";
  import IconBtn from "./ui/IconBtn.svelte";

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

{#snippet refreshAction()}
  <IconBtn name="refresh" title="Refresh" variant="secondary" size="md" disabled={loading} go={load} />
{/snippet}

<Page
  title="Machines"
  desc="The machines that share your account. One file per account: the newest backup wins, whichever machine saved it."
  actions={$cloud?.signed_in ? refreshAction : undefined}
>
  <Group title="Your machines">
    {#if !$cloud?.signed_in}
      <Empty compact icon="machines" title="Not signed in" desc="Sign in to see the machines that share your account." />
    {:else if loading && !$machines.length}
      <div class="ewe-row" aria-busy="true">
        <span class="ewe-spinner" aria-hidden="true"></span><span class="ewe-row__title">Reading…</span>
      </div>
    {:else if !$machines.length}
      <Empty
        compact
        icon="machines"
        title="No machines yet"
        desc="No machine has backed up yet. This one joins the list at its first backup."
      />
    {:else}
      {#each $machines as m, i (m.name)}
        {#if i > 0}<div class="ewe-list__divider"></div>{/if}
        <Row
          tall
          class={m.name === me ? "is-active" : ""}
          title={m.name}
          sub={`ewe ${m.ewe_version || "?"} · ${m.apps_count ?? 0} apps · seen ${fmtTime(m.last_seen)}`}
        >
          {#snippet lead()}
            <span class="ewe-avatar ewe-avatar--square ewe-avatar--neutral" aria-hidden="true">
              {m.name.slice(0, 1).toUpperCase()}
            </span>
          {/snippet}
          {#if $sync?.remote_machine === m.name}
            <span class="ewe-badge ewe-badge--accent"><span class="ewe-badge__label">Saved the backup</span></span>
          {/if}
          {#if m.name === me}
            <span class="ewe-badge"><span class="ewe-badge__label">This machine</span></span>
          {/if}
        </Row>
      {/each}
    {/if}
  </Group>

  {#if $cloud?.signed_in}
    <p class="note">Per-machine backups are a later option.</p>
  {/if}
</Page>
