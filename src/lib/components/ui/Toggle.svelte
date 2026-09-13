<script>
  /** Controlled switch: `on` is the truth, `toggled()` reports intent. */
  let { on = false, disabled = false, toggled = () => {} } = $props();
</script>

<!-- 38×22 track, 18px thumb; the track is a recessed well (--bg-2) until it
     fills with the brand. No edge — the layer step separates it. -->
<button
  type="button"
  role="switch"
  aria-checked={on}
  {disabled}
  class="toggle {on ? 'is-on' : ''}"
  onclick={() => toggled()}
>
  <span class="toggle-thumb"></span>
</button>

<style>
  .toggle {
    position: relative;
    display: inline-block;
    width: 38px;
    height: 22px;
    flex-shrink: 0;
    border-radius: var(--radius-pill);
    background: var(--bg-2);
    transition: background-color 150ms ease;
  }
  .toggle:hover {
    background: var(--bg-2-hover);
  }
  .toggle.is-on,
  .toggle.is-on:hover {
    background: var(--brand-bg);
  }
  .toggle.is-on:hover {
    background: var(--brand-bg-hover);
  }
  .toggle:disabled {
    opacity: 0.5;
    pointer-events: none;
  }
  .toggle:focus-visible {
    outline: var(--focus-width) solid var(--stroke-focus-2);
    outline-offset: 1px;
  }
  .toggle-thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 18px;
    height: 18px;
    border-radius: var(--radius-pill);
    background: var(--fg-3);
    transition: background-color 150ms ease;
  }
  .toggle.is-on .toggle-thumb {
    left: 18px;
    background: var(--fg-on-brand);
  }
</style>
