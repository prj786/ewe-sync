<script>
  // Side navigation (design/system/components/SideNav): the sheep mark and
  // the app name, the sections, and in the footer the account's state and
  // the version in Geist Mono. Up and Down move between sections; Ctrl+1 …
  // Ctrl+6 jump (App.svelte).
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import sheep from "../../assets/sheep.svg?raw";
  import Icon from "./ui/Icon.svelte";
  import { route, cloud, trayState } from "../stores";

  let { items = [] } = $props();

  let version = $state("");
  onMount(async () => {
    try {
      version = await getVersion();
    } catch {
      // dev server outside Tauri
    }
  });

  // the footer dot: one status role per tray state; offline and signed out
  // fall back to the neutral dot (a Badge dot, Badge card)
  const dotTone = { idle: "ewe-badge--success", syncing: "ewe-badge--warning", conflict: "ewe-badge--warning" };
  const stateWord = {
    idle: "Up to date",
    syncing: "Syncing…",
    conflict: "Needs a decision",
    offline: "Offline",
    "signed-out": "Not signed in"
  };

  let navItems = $state([]);
  function navKey(e, i) {
    const d = e.key === "ArrowDown" ? 1 : e.key === "ArrowUp" ? -1 : 0;
    if (!d) return;
    e.preventDefault();
    const n = (i + d + items.length) % items.length;
    navItems[n]?.focus();
    route.set(items[n].id);
  }
</script>

<nav class="ewe-sidenav" aria-label="ewe-sync sections">
  <div class="ewe-sidenav__brand">
    <span class="ewe-sidenav__logo" aria-hidden="true">{@html sheep}</span>
    <span class="ewe-sidenav__name">ewe-sync</span>
  </div>
  <div class="ewe-sidenav__group">
    {#each items as it, i (it.id)}
      <button
        bind:this={navItems[i]}
        class="ewe-navitem"
        class:is-selected={$route === it.id}
        aria-current={$route === it.id ? "page" : undefined}
        title={it.label}
        onclick={() => route.set(it.id)}
        onkeydown={(e) => navKey(e, i)}
      >
        <Icon name={it.icon} />
        <span class="ewe-navitem__label">{it.label}</span>
      </button>
    {/each}
  </div>

  <div class="ewe-sidenav__foot">
    <div class="sync-state" title={stateWord[$trayState] || ""}>
      <span class="ewe-badge ewe-badge--dot ewe-badge--solid {dotTone[$trayState] || 'ewe-badge--neutral'}"></span>
      <span class="sync-state__text ewe-navitem__label">
        {#if $cloud?.signed_in}{$cloud.display_name || $cloud.user}{:else}Not signed in{/if}
      </span>
    </div>
    {#if version}<div class="ewe-sidenav__version">ewe-sync {version}</div>{/if}
  </div>
</nav>
