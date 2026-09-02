<script>
  import { toasts, dismissToast } from "../stores";
  const styles = {
    success: "border-green-500/40 bg-green-50 text-green-800 dark:bg-green-900/40 dark:text-green-200",
    error: "border-red-500/40 bg-red-50 text-red-800 dark:bg-red-900/40 dark:text-red-200",
    info: "border-zinc-300 bg-white text-zinc-700 dark:border-zinc-600 dark:bg-zinc-800 dark:text-zinc-200"
  };
</script>

<div class="fixed bottom-4 right-4 z-50 flex w-96 max-w-[90vw] flex-col gap-2">
  {#each $toasts as t (t.id)}
    <button
      class="rounded-xl border px-4 py-2.5 text-left text-sm shadow-lg backdrop-blur {styles[t.type] || styles.info}"
      onclick={() => dismissToast(t.id)}
    >
      {t.message}
    </button>
  {/each}
</div>
