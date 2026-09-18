<script>
  /**
   * One glyph of the vendored Lucide font inside an .ewe-icon box. The box
   * takes its size from the component it sits in (an .ewe-btn sets 16px, an
   * .ewe-empty__icon 24px …) or from `size` (xs, sm, md, lg, xl, 2xl); the
   * glyph fills the box (app.css, .ewe-glyph). `code` is a codepoint or a
   * name from ./icons.js.
   */
  import { ICONS } from "./icons.js";
  let { code, name = "", size = "", tone = "", class: className = "", label = "" } = $props();
  const cp = $derived(typeof code === "number" ? code : ICONS[name || code] ?? 0x3f);
  const sizeClass = $derived(size && size !== "md" ? `ewe-icon--${size}` : "");
  const toneClass = $derived(tone ? `ewe-icon-tone--${tone}` : "");
</script>

<span
  class="ewe-icon ewe-glyph {sizeClass} {toneClass} {className}"
  role={label ? "img" : undefined}
  aria-label={label || undefined}
  aria-hidden={label ? undefined : "true"}><i>{String.fromCodePoint(cp)}</i></span
>
