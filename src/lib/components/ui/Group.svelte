<script>
  /**
   * A group on a page: an overline Section header right above a list well of
   * rows. `action` is the header's optional button (bottom right). `well =
   * false` leaves the content bare (a card, a form).
   */
  let { title = "", desc = "", well = true, class: className = "", action, children, after } = $props();
</script>

<section class="group {className}">
  {#if title || desc || action}
    <div class="ewe-section">
      <div class="ewe-section__text">
        {#if title}<h2 class="ewe-section__title ewe-section__title--overline">{title}</h2>{/if}
        {#if desc}<p class="ewe-section__desc">{desc}</p>{/if}
      </div>
      {@render action?.()}
    </div>
  {/if}
  {#if well}
    <div class="ewe-list">{@render children?.()}</div>
  {:else}
    {@render children?.()}
  {/if}
  {@render after?.()}
</section>
