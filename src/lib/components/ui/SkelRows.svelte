<script>
  /**
   * Loading rows (design/system/components/Skeleton) in the shape of the
   * rows that replace them — a tile or none, a title and a line under it —
   * so the page doesn't jump when the data lands. The well reports busy;
   * the shapes are hidden from assistive technology.
   */
  let { rows = 2, tile = true, label = "Loading" } = $props();
  const widths = ["skel-w1", "skel-w2", "skel-w3"];
</script>

<div class="skel-rows" aria-busy="true" aria-label={label} role="status">
  {#each Array(rows) as _, i}
    {#if i > 0}<div class="ewe-list__divider"></div>{/if}
    <div class="ewe-row" aria-hidden="true">
      {#if tile}<span class="ewe-skel ewe-skel--block skel-tile"></span>{/if}
      <span class="ewe-row__text">
        <span class="ewe-skel ewe-skel--title {widths[i % 3]}"></span>
        <span class="ewe-skel ewe-skel--text {widths[(i + 1) % 3]}"></span>
      </span>
    </div>
  {/each}
</div>
