<script>
  /**
   * One glyph of the vendored Lucide font (the .icon face in app.css).
   * shadcn-svelte's components normally pull icons from an npm icon package;
   * they import this instead, so nothing but the font ships and the app keeps
   * a single icon language. Codepoints live in ./icons.js.
   */
  let { code, size = 14, class: className = "" } = $props();
</script>

<span
  class="icon shrink-0 {className}"
  style="font-size: {size}px; width: {size}px; height: {size}px"
  aria-hidden="true">{String.fromCodePoint(code)}</span
>
