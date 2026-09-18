<script>
  /**
   * A list row (List row card): the item's name and what it is on the left,
   * its control or current value on the right. `icon` adds the 32px icon
   * tile; `lead` replaces it with anything (an avatar); `block` stacks the
   * trailing content under the text.
   */
  import Icon from "./Icon.svelte";
  let {
    title = "",
    sub = "",
    icon = "",
    block = false,
    dense = false,
    tall = false,
    class: className = "",
    lead,
    text,
    children
  } = $props();
</script>

<div class="ewe-row {block ? 'ewe-row--block' : ''} {dense ? 'ewe-row--dense' : ''} {tall ? 'ewe-row--tall' : ''} {className}">
  {#if lead}
    <span class="ewe-row__lead">{@render lead()}</span>
  {:else if icon}
    <span class="ewe-row__lead ewe-row__lead--tile"><Icon name={icon} /></span>
  {/if}
  {#if title || sub || text}
    <div class="ewe-row__text">
      {#if title}<div class="ewe-row__title">{title}</div>{/if}
      {@render text?.()}
      {#if sub}<div class="ewe-row__desc">{sub}</div>{/if}
    </div>
  {/if}
  {#if children}
    <div class="ewe-row__trail">{@render children()}</div>
  {/if}
</div>
