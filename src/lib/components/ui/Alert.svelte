<script>
  /**
   * Inline alert (design/system/components/InlineAlert): something on this
   * page needs attention or explanation. tone: info | success | warning |
   * danger | accent | "" (neutral).
   */
  import Icon from "./Icon.svelte";
  let { tone = "info", title = "", dismiss = null, children, actions } = $props();
  const glyph = { info: "info", accent: "info", success: "success", warning: "warning", danger: "alert" };
</script>

<div
  class="ewe-alert {tone ? `ewe-alert--${tone}` : ''}"
  role={tone === "danger" || tone === "warning" ? "alert" : "status"}
>
  <Icon name={glyph[tone] || "info"} />
  <div class="ewe-alert__body">
    {#if title}<div class="ewe-alert__title">{title}</div>{/if}
    {#if children}<div class="ewe-alert__desc">{@render children()}</div>{/if}
    {#if actions}<div class="ewe-alert__actions">{@render actions()}</div>{/if}
  </div>
  {#if dismiss}
    <button class="ewe-iconbtn ewe-iconbtn--ghost ewe-iconbtn--sm" aria-label="Dismiss" title="Dismiss" onclick={dismiss}>
      <Icon name="x" />
    </button>
  {/if}
</div>
