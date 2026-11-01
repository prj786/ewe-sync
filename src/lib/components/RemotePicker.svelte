<script>
  // A folder in your account, picked by walking it: a breadcrumb from the
  // account's top and the folders inside with their sizes. It is the second
  // step of the folder sheet: `path`, `loading` and `free` bind up to the
  // sheet, whose footer holds "Use “Work”" (and whose back arrow is the way
  // out — one Cancel, not two). Files are listed greyed so you can tell
  // where you are; only folders open. New folders need no button — type a
  // new name in the form and the first sync creates it.
  import * as api from "../api";
  import { fmtBytes } from "../stores";
  import Icon from "./ui/Icon.svelte";

  let { start = "", path = $bindable(""), loading = $bindable(false), free = $bindable(null) } = $props();

  let listing = $state(null); // { path, size, free, items: [{ name, path, dir, size }] }
  let error = $state("");

  async function go(p) {
    loading = true;
    error = "";
    try {
      listing = await api.remoteList(p);
      path = listing.path || "";
      if (listing.free != null) free = listing.free;
    } catch (e) {
      error = String(e);
      // a folder that is not there yet: show its parent
      if (p && /isn’t in your account/.test(error)) {
        const up = p.split("/").slice(0, -1).join("/");
        if (up !== p) return go(up);
      }
    } finally {
      loading = false;
    }
  }
  // once, from where the form points (not again as `path` moves)
  $effect.pre(() => {
    const from = String(start || "").replace(/^\/+|\/+$/g, "");
    queueMicrotask(() => go(from));
  });

  const crumbs = $derived(path ? path.split("/") : []);
  const dirs = $derived((listing?.items || []).filter((i) => i.dir));
  const files = $derived((listing?.items || []).filter((i) => !i.dir));
</script>

<div class="picker" role="group" aria-label="Folders in your account">
  <div class="picker__head">
    <nav class="ewe-breadcrumb" aria-label="Where you are">
      <button class="ewe-breadcrumb__item" class:is-current={!crumbs.length} onclick={() => go("")}>
        <Icon name="cloud" />Your account
      </button>
      {#each crumbs as c, i}
        <Icon name="caretRight" />
        <button
          class="ewe-breadcrumb__item"
          class:is-current={i === crumbs.length - 1}
          onclick={() => go(crumbs.slice(0, i + 1).join("/"))}>{c}</button
        >
      {/each}
    </nav>
    {#if loading}<span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>{/if}
  </div>
  {#if error}
    <p class="ewe-field__helper ewe-field__helper--error"><Icon name="alert" />{error}</p>
  {/if}
  <div class="picker__list ewe-list">
    {#each dirs as d (d.path)}
      <button class="ewe-row ewe-row--dense ewe-row--interactive row-button" onclick={() => go(d.path)}>
        <span class="ewe-row__lead ewe-row__lead--tile"><Icon name="folder" /></span>
        <span class="ewe-row__text"><span class="ewe-row__title">{d.name}</span></span>
        <span class="ewe-row__trail picker__size">{fmtBytes(d.size)}<Icon name="caretRight" /></span>
      </button>
    {/each}
    {#each files.slice(0, 6) as f (f.path)}
      <div class="ewe-row ewe-row--dense is-dim">
        <span class="ewe-row__lead ewe-row__lead--tile"><Icon name="fileText" /></span>
        <span class="ewe-row__text"><span class="ewe-row__title">{f.name}</span></span>
        <span class="ewe-row__trail picker__size">{fmtBytes(f.size)}</span>
      </div>
    {/each}
    {#if files.length > 6}
      <div class="ewe-row ewe-row--dense is-dim"><span class="ewe-row__text"><span class="ewe-row__desc">and {files.length - 6} more files</span></span></div>
    {/if}
    {#if listing && !dirs.length && !files.length}
      <div class="ewe-row ewe-row--dense is-dim"><span class="ewe-row__text"><span class="ewe-row__desc">This folder is empty.</span></span></div>
    {/if}
  </div>
</div>
