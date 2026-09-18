<script>
  /**
   * A page (App shell, Header bar "Page title"): the 22px title with a
   * one-line description and at most one page action, then groups.
   */
  let { title = "", desc = "", actions, children } = $props();
</script>

<div class="page">
  <header class="ewe-headerbar ewe-headerbar--page">
    <div class="ewe-headerbar__titles">
      <h1 class="ewe-headerbar__title">{title}</h1>
      {#if desc}<p class="ewe-headerbar__subtitle">{desc}</p>{/if}
    </div>
    {#if actions}
      <div class="ewe-headerbar__actions">{@render actions()}</div>
    {/if}
  </header>
  {@render children?.()}
</div>
