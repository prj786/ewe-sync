<script>
  /**
   * Switch (design/system/components/Switch): a button with the switch role;
   * the look is .ewe-switch, `is-on` when checked. The thumb carries a small
   * x or check so the state never rests on colour alone. Controlled: `on` is
   * the truth, `toggled(next)` reports intent.
   */
  import Icon from "./Icon.svelte";
  let { on = false, disabled = false, label = "", toggled = () => {} } = $props();
</script>

<button
  type="button"
  role="switch"
  aria-checked={on}
  aria-label={label || undefined}
  {disabled}
  class="ewe-switch"
  class:is-on={on}
  class:is-disabled={disabled}
  onclick={() => toggled(!on)}
>
  <span class="ewe-switch__thumb"><Icon name={on ? "check" : "x"} /></span>
</button>
