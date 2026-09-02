<script>
  /** One glyph of the vendored Phosphor Fill font — the DE's icon language. */
  let { code, size = 14, class: className = "" } = $props();
</script>

<span
  class="ph-i shrink-0 {className}"
  style="font-size: {size}px; width: {size}px; height: {size}px"
  aria-hidden="true">{String.fromCodePoint(code)}</span
>
