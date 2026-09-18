<script>
  /**
   * Checkbox (design/system/components/Checkbox): a button with the checkbox
   * role inside its label, so the whole row is the click target and Space
   * toggles it. Controlled: `checked` is the truth, `toggled(next)` reports.
   */
  import Icon from "./Icon.svelte";
  let { checked = false, disabled = false, label = "", desc = "", toggled = () => {} } = $props();
</script>

<label class="ewe-check" class:is-checked={checked} class:is-disabled={disabled}>
  <button
    type="button"
    role="checkbox"
    aria-checked={checked}
    {disabled}
    class="ewe-check__box"
    onclick={() => toggled(!checked)}
  >
    {#if checked}<Icon name="check" size="xs" />{/if}
  </button>
  <span class="ewe-check__text">
    <span class="ewe-check__label">{label}</span>
    {#if desc}<span class="ewe-check__desc">{desc}</span>{/if}
  </span>
</label>
