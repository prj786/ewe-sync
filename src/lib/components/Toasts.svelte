<script>
  import { toasts, dismissToast } from "../stores";
  const styles = {
    success: "border-[var(--success)] bg-[color-mix(in_srgb,var(--success)_15%,transparent)] text-[var(--success)]",
    error: "border-[var(--danger)] bg-[color-mix(in_srgb,var(--danger)_15%,transparent)] text-[var(--danger)]",
    info: "border-hairline bg-white text-muted  dark:bg-elevated text-fg"
  };
</script>

<div class="fixed bottom-4 right-4 z-50 flex w-96 max-w-[90vw] flex-col gap-2">
  {#each $toasts as t (t.id)}
    <button
      class="border px-4 py-2.5 text-left text-sm {styles[t.type] || styles.info}" style="border-radius: var(--radius-panel); border-width: var(--stroke-width); box-shadow: var(--elevation)"
      onclick={() => dismissToast(t.id)}
    >
      {t.message}
    </button>
  {/each}
</div>
