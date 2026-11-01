<script>
  // Side navigation (design/system/components/SideNav): the sheep mark and
  // the app name, the sections — each with a count Badge when something
  // there needs you — and in the footer who is signed in with the state
  // written under the name ("Up to date", "1 needs a decision", "Offline"),
  // then the version in Geist Mono. Up and Down move between sections;
  // Ctrl+1 … Ctrl+7 jump (App.svelte). Below 720px the rail is icons only
  // and the labels become tooltips.
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import sheep from "../../assets/ewe-mark.svg?raw";
  import Icon from "./ui/Icon.svelte";
  import { route, cloud, avatar, navCounts, statusLine } from "../stores";

  let { items = [] } = $props();

  let version = $state("");
  let narrow = $state(false);
  onMount(() => {
    getVersion().then((v) => (version = v)).catch(() => {}); // dev server outside Tauri
    const mq = window.matchMedia("(max-width: 720px)");
    const set = () => (narrow = mq.matches);
    set();
    mq.addEventListener("change", set);
    return () => mq.removeEventListener("change", set);
  });

  let navItems = $state([]);
  function navKey(e, i) {
    const d = e.key === "ArrowDown" ? 1 : e.key === "ArrowUp" ? -1 : 0;
    if (!d) return;
    e.preventDefault();
    const n = (i + d + items.length) % items.length;
    navItems[n]?.focus();
    route.set(items[n].id);
  }

  const name = $derived($cloud?.signed_in ? $cloud.display_name || $cloud.user : "");
  const toneClass = { warning: "state-line--warning", danger: "state-line--danger", muted: "" };
</script>

<nav class="ewe-sidenav" aria-label="ewe Sync sections">
  <div class="ewe-sidenav__brand">
    <span class="ewe-sidenav__logo" aria-hidden="true">{@html sheep}</span>
    <span class="ewe-sidenav__name">ewe Sync</span>
  </div>
  <div class="ewe-sidenav__group">
    {#each items as it, i (it.id)}
      {@const badge = $navCounts[it.id]}
      <button
        bind:this={navItems[i]}
        class="ewe-navitem"
        class:is-selected={$route === it.id}
        aria-current={$route === it.id ? "page" : undefined}
        data-route={it.id}
        title={narrow ? it.label : undefined}
        onclick={() => route.set(it.id)}
        onkeydown={(e) => navKey(e, i)}
      >
        <Icon name={it.icon} />
        <span class="ewe-navitem__label">{it.label}</span>
        {#if badge}
          <span
            class="ewe-badge ewe-badge--solid ewe-badge--{badge.tone} nav-count"
            aria-label={`${badge.count} need${badge.count === 1 ? "s" : ""} you`}
          ><span class="ewe-badge__label">{badge.count}</span></span>
        {/if}
      </button>
    {/each}
  </div>

  <div class="ewe-sidenav__foot">
    <button
      type="button"
      class="who"
      title={narrow ? `${name || "Not signed in"} · ${$statusLine.word}` : undefined}
      onclick={() => route.set($cloud?.signed_in ? "overview" : "account")}
    >
      <span class="ewe-avatar ewe-avatar--sm who__avatar" aria-hidden="true">
        {#if $avatar && name}
          <img class="who__img" src={$avatar} alt="" />
        {:else if name}
          {name.slice(0, 1).toUpperCase()}
        {:else}
          <Icon name="user" />
        {/if}
        {#if $statusLine.dot !== null}
          <span class="ewe-avatar__status {$statusLine.dot ? `ewe-avatar__status--${$statusLine.dot}` : ''}"></span>
        {/if}
      </span>
      <span class="who__text">
        <span class="who__name">{name || "Not signed in"}</span>
        {#if name}<span class="who__state {toneClass[$statusLine.tone]}">{$statusLine.word}</span>{/if}
      </span>
    </button>
    {#if version}<div class="ewe-sidenav__version">ewe Sync {version}</div>{/if}
  </div>
</nav>
