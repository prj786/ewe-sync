<script>
  // Empty state (design/system/components/EmptyState), compact inside a well
  // or a pane: what's missing, what to do, and at most one action.
  import Icon from "./Icon.svelte";
  let { icon = "info", title = "", desc = "", compact = false, children } = $props();
</script>

<div class="ewe-empty {compact ? 'ewe-empty--compact' : ''}">
  <span class="ewe-empty__icon"><Icon name={icon} /></span>
  {#if title}<div class="ewe-empty__title">{title}</div>{/if}
  {#if desc}<div class="ewe-empty__desc">{desc}</div>{/if}
  {#if children}<div class="ewe-empty__actions">{@render children()}</div>{/if}
</div>
