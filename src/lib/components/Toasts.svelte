<script>
  import { toasts, dismissToast } from "../stores";
  // each toast sits on its status's own ground — no edge, the fill is the signal
  const styles = {
    success: "is-success",
    error: "is-danger",
    info: ""
  };
</script>

<div class="fixed bottom-4 right-4 z-50 flex w-96 max-w-[90vw] flex-col gap-2">
  {#each $toasts as t (t.id)}
    <button
      class="toast {styles[t.type] || ''} px-4 py-2.5 text-left text-sm"
      onclick={() => dismissToast(t.id)}
    >
      {t.message}
    </button>
  {/each}
</div>

<style>
  .toast {
    border-radius: var(--radius-card);
    background: var(--card);
    color: var(--fg-1);
    box-shadow: var(--elevation);
    transition: background-color 130ms ease;
  }
  .toast:hover {
    background: var(--card-hover);
  }
  .toast.is-success {
    background: var(--success-bg);
    color: var(--success);
  }
  .toast.is-danger {
    background: var(--danger-bg);
    color: var(--danger);
  }
</style>
