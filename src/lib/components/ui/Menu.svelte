<script>
  /**
   * An overflow menu (design/system/components/Menu) behind a ghost ⋯ icon
   * button: one tab stop per row instead of a cluster of icon buttons.
   * `items`: [{ label, icon, run, danger, disabled }] or { divider: true }.
   * Opens below its trigger (above when there is no room), right-aligned;
   * Up/Down/Home/End move, Enter runs, Esc or a click outside closes and
   * focus returns to the trigger. Destructive items go last, after a divider.
   */
  import { tick } from "svelte";
  import Icon from "./Icon.svelte";

  let { label = "More", items = [], disabled = false } = $props();

  let open = $state(false);
  let trigger = $state(null);
  let menu = $state(null);
  let pos = $state({ top: 0, left: 0 });

  // disabled items stay focusable so their name can be read (Menu card)
  const enabled = () => [...(menu?.querySelectorAll(".ewe-menu__item") || [])];
  const token = (name) => parseFloat(getComputedStyle(document.documentElement).getPropertyValue(name)) || 0;

  async function show() {
    open = true;
    await tick();
    place();
    enabled()[0]?.focus();
  }
  function close(refocus = true) {
    if (!open) return;
    open = false;
    if (refocus) trigger?.focus();
  }
  function place() {
    if (!trigger || !menu) return;
    const r = trigger.getBoundingClientRect();
    const m = menu.getBoundingClientRect();
    const gap = token("--space-xs"); // between trigger and menu (Menu card)
    let top = r.bottom + gap;
    if (top + m.height > window.innerHeight - gap) top = Math.max(gap, r.top - gap - m.height);
    const left = Math.min(Math.max(gap, r.right - m.width), window.innerWidth - m.width - gap);
    pos = { top, left };
  }
  function run(it) {
    if (it.disabled) return;
    close();
    it.run?.();
  }
  function onKey(e) {
    const list = enabled();
    const i = list.indexOf(document.activeElement);
    if (e.key === "ArrowDown") list[(i + 1) % list.length]?.focus();
    else if (e.key === "ArrowUp") list[(i - 1 + list.length) % list.length]?.focus();
    else if (e.key === "Home") list[0]?.focus();
    else if (e.key === "End") list[list.length - 1]?.focus();
    else if (e.key === "Escape") close();
    else if (e.key === "Tab") return close(false);
    else return;
    e.preventDefault();
    e.stopPropagation();
  }
  function outside(e) {
    if (open && !menu?.contains(e.target) && !trigger?.contains(e.target)) close(false);
  }
</script>

<svelte:window onpointerdown={outside} onresize={() => close(false)} onblur={() => close(false)} />

<button
  bind:this={trigger}
  type="button"
  class="ewe-iconbtn ewe-iconbtn--ghost ewe-iconbtn--sm"
  class:is-hover={open}
  aria-label={label}
  aria-haspopup="menu"
  aria-expanded={open}
  {disabled}
  onclick={() => (open ? close() : show())}
  onkeydown={(e) => e.key === "ArrowDown" && !open && (e.preventDefault(), show())}
>
  <Icon name="more" />
</button>

{#if open}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    bind:this={menu}
    class="ewe-menu menu-floating"
    role="menu"
    tabindex="-1"
    aria-label={label}
    data-state="open"
    style="top: {pos.top}px; left: {pos.left}px"
    onkeydown={onKey}
  >
    {#each items as it, i (i)}
      {#if it.divider}
        <div class="ewe-menu__divider" role="separator"></div>
      {:else}
        <button
          type="button"
          role="menuitem"
          class="ewe-menu__item menu-button"
          class:ewe-menu__item--danger={it.danger}
          class:is-disabled={it.disabled}
          aria-disabled={it.disabled ? "true" : undefined}
          onclick={() => run(it)}
        >
          {#if it.icon}<Icon name={it.icon} />{:else}<span class="ewe-menu__spacer"></span>{/if}
          <span class="ewe-menu__label">{it.label}</span>
        </button>
      {/if}
    {/each}
  </div>
{/if}
