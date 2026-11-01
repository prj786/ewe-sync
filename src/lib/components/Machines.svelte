<script>
  // Computers (route id "machines": `ewe-sync --pane machines` is public) —
  // every computer that keeps its settings in this account, from the
  // machine registry: which one this is, which one saved the settings every
  // computer gets, and when each was last seen.
  import { onMount } from "svelte";
  import * as api from "../api";
  import { route, cloud, sync, machines, busy, toast, fmtTime } from "../stores";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Row from "./ui/Row.svelte";
  import Empty from "./ui/Empty.svelte";
  import IconBtn from "./ui/IconBtn.svelte";
  import SkelRows from "./ui/SkelRows.svelte";
  import Icon from "./ui/Icon.svelte";

  let loading = $state(false);
  let loaded = $state(false);
  // this computer: by id (two computers may share a hostname), by name for
  // a legacy record that has no id yet
  let me = $state({ id: "", name: "" });
  const isMe = (m) => (m.id ? m.id === me.id : m.name === me.name);
  // this computer first, then the most recently seen
  const list = $derived(
    [...$machines].sort((a, b) => (isMe(b) ? 1 : 0) - (isMe(a) ? 1 : 0) || String(b.last_seen || "").localeCompare(String(a.last_seen || "")))
  );

  async function load() {
    if (!$cloud?.signed_in) return;
    loading = true;
    try {
      machines.set(await api.machinesList());
      loaded = true;
    } catch (e) {
      if (String(e) !== "not-signed-in") toast(String(e), "error");
    } finally {
      loading = false;
    }
  }
  onMount(async () => {
    try {
      me = await api.thisMachine();
    } catch {}
    load();
  });
  $effect(() => {
    if ($cloud?.signed_in) load();
  });

  // the empty list's next step: this computer joins at its first backup
  async function backUp() {
    if ($busy) return;
    busy.set("sync");
    try {
      const r = await api.push(false);
      if (r.ok) toast("This computer’s settings are backed up", "success");
      else if (r.error === "remote-newer" || r.error === "remote-exists") {
        route.set("machine");
        toast("Your account already has settings. Choose which ones this computer keeps.", "warning");
      } else toast(r.message || r.error || "Couldn’t back up. Try again.", "error");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy.set("");
      load();
    }
  }
</script>

{#snippet refreshAction()}
  <IconBtn name="refresh" title="Refresh" variant="secondary" size="md" disabled={loading} go={load} />
{/snippet}

<Page
  title="Computers"
  desc="The computers signed in to your account. Each one gets the newest settings any of them saved."
  actions={$cloud?.signed_in ? refreshAction : undefined}
>
  <Group title={$cloud?.signed_in && list.length ? `Your computers · ${list.length}` : "Your computers"}>
    {#if !$cloud?.signed_in}
      <Empty compact icon="laptop" title="Not signed in" desc="Sign in to see the computers that share your account.">
        <button class="ewe-btn ewe-btn--secondary ewe-btn--sm" onclick={() => route.set("account")}>Sign in</button>
      </Empty>
    {:else if !loaded && !list.length}
      <SkelRows rows={2} label="Reading your computers" />
    {:else if !list.length}
      <Empty
        compact
        icon="laptop"
        title="No computers yet"
        desc="A computer joins this list when it backs up its settings. This one hasn’t yet."
      >
        <button class="ewe-btn ewe-btn--secondary ewe-btn--sm" aria-busy={$busy === "sync"} aria-disabled={!!$busy} onclick={backUp}>
          <Icon name="arrowUp" />Back up this computer
        </button>
      </Empty>
    {:else}
      {#each list as m, i (m.id || m.name)}
        {#if i > 0}<div class="ewe-list__divider"></div>{/if}
        <Row
          tall
          icon="laptop"
          class={isMe(m) ? "is-active" : ""}
          title={m.name}
          sub={`ewe ${m.ewe_version || "?"} · ${m.apps_count ?? 0} app${m.apps_count === 1 ? "" : "s"} · seen ${fmtTime(m.last_seen)}`}
        >
          {#if $sync?.remote_machine === m.name}
            <span class="ewe-badge"><span class="ewe-badge__label">Saved the latest settings</span></span>
          {/if}
          {#if isMe(m)}
            <span class="ewe-badge ewe-badge--accent"><span class="ewe-badge__label">This computer</span></span>
          {/if}
        </Row>
      {/each}
    {/if}
  </Group>
</Page>
