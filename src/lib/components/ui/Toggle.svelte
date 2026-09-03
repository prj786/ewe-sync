<script>
  /** Controlled switch: `on` is the truth, `toggled()` reports intent. */
  let { on = false, disabled = false, toggled = () => {} } = $props();
</script>

<button
  type="button"
  role="switch"
  aria-checked={on}
  {disabled}
  class="relative h-5 w-9 shrink-0 transition-colors disabled:opacity-50"
  style="border-radius: var(--radius-pill); border: var(--border-thin) solid {on ? 'var(--accent)' : 'var(--stroke)'}; background: {on ? 'var(--accent)' : 'var(--elevated)'}"
  onclick={() => toggled()}
>
  <span
    class="absolute top-0.5 h-4 w-4 transition-all"
    style="left: {on ? '18px' : '2px'}; border-radius: var(--radius-pill); background: {on ? 'var(--accent-fg)' : 'var(--fg-dim)'}"
  ></span>
</button>
