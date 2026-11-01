<script>
  /**
   * A sheet (design/system/components/Sheet): a 400px panel at the window's
   * right edge for a task beside the page — adding or changing a folder, a
   * mail account — so the list it changes stays put. Header: title, an
   * optional back arrow (a second step inside the sheet) and Close. Body
   * scrolls on its own; `foot` holds the buttons, right-aligned.
   * Non-modal (the card: it doesn't dim the page). Esc closes it, or steps
   * back when there is a back step. Focus moves to the first field and
   * returns to the opener on close.
   */
  import { onMount, tick } from "svelte";
  import Icon from "./Icon.svelte";

  let { title = "", back = null, onClose = () => {}, children, foot } = $props();

  const id = `sheet-${Math.random().toString(36).slice(2, 8)}`;
  let el = $state(null);
  let opener = null;

  async function focusFirst() {
    await tick();
    const f = el?.querySelector(".ewe-sheet__body input:not(:disabled), .ewe-sheet__body textarea, .ewe-sheet__body button:not(:disabled), .ewe-sheet__body [tabindex='0']");
    (f || el)?.focus();
  }
  onMount(() => {
    opener = document.activeElement;
    focusFirst();
    return () => {
      if (opener && document.contains(opener)) opener.focus();
    };
  });
  // a new step (back appears or goes) moves focus into it
  $effect(() => {
    void back;
    focusFirst();
  });

  function onKey(e) {
    if (e.key !== "Escape" || e.defaultPrevented) return;
    // a dialog above the sheet answers its own Esc
    if (document.querySelector(".ewe-dialog.is-floating")) return;
    e.preventDefault();
    if (back) back();
    else onClose();
  }
</script>

<svelte:window onkeydown={onKey} />

<div bind:this={el} class="ewe-sheet is-floating" role="dialog" aria-modal="false" aria-labelledby={id} tabindex="-1" data-state="open">
  <div class="ewe-sheet__head">
    {#if back}
      <button type="button" class="ewe-iconbtn ewe-iconbtn--ghost ewe-iconbtn--sm sheet-back" aria-label="Back" title="Back" onclick={back}>
        <Icon name="arrowLeft" />
      </button>
    {/if}
    <h2 class="ewe-sheet__title" {id}>{title}</h2>
    <button type="button" class="ewe-iconbtn ewe-iconbtn--ghost ewe-iconbtn--sm" aria-label="Close" title="Close" onclick={onClose}>
      <Icon name="x" />
    </button>
  </div>
  <div class="ewe-sheet__body">
    {@render children?.()}
  </div>
  {#if foot}
    <div class="ewe-sheet__foot">{@render foot()}</div>
  {/if}
</div>
