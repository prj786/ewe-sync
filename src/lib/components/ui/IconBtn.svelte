<script>
  /**
   * Icon button (design/system/components/IconButton), ghost by default. The
   * tooltip the card asks for is the native title; the accessible name is the
   * same words. `danger` tints the hover like a danger menu row.
   */
  import Icon from "./Icon.svelte";
  let {
    name = "",
    title = "",
    disabled = false,
    danger = false,
    variant = "ghost",
    size = "sm",
    go = () => {}
  } = $props();
</script>

<button
  type="button"
  {title}
  aria-label={title}
  {disabled}
  class="ewe-iconbtn ewe-iconbtn--{variant} {size !== 'md' ? `ewe-iconbtn--${size}` : ''} {danger ? 'is-danger' : ''}"
  onclick={() => go()}
>
  <Icon {name} />
</button>
