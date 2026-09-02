<script>
  /** Controlled switch: `on` is the truth, `toggled()` reports intent. */
  let { on = false, disabled = false, toggled = () => {} } = $props();
</script>

<button
  type="button"
  role="switch"
  aria-checked={on}
  {disabled}
  class="relative h-5 w-9 shrink-0 rounded-full transition-colors disabled:opacity-50"
  style={on ? "background: var(--accent)" : ""}
  class:bg-zinc-400={!on}
  class:dark:bg-zinc-600={!on}
  onclick={() => toggled()}
>
  <span
    class="absolute top-0.5 h-4 w-4 rounded-full bg-white shadow transition-all"
    style="left: {on ? '18px' : '2px'}"
  ></span>
</button>
