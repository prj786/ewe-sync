<script>
  /**
   * The Toast (design/system/components/Toast): one at a time, centered at
   * the bottom of the window. The 2px accent timer shrinks while the toast
   * waits and pauses while it is hovered or focused; when it runs out the
   * toast closes. role=status: announced politely, never takes focus.
   */
  import { currentToast, dismissToast } from "../stores";
  import Icon from "./ui/Icon.svelte";

  const glyph = { success: "success", warning: "warning", danger: "alert", info: "info" };
  // **name** in a message is the thing it names, set in weight 600
  const parts = (m) => String(m).split(/\*\*(.+?)\*\*/g).map((t, i) => ({ t, b: i % 2 === 1 }));

  async function act(t) {
    dismissToast(t.id);
    try {
      await t.action.run();
    } catch (e) {
      console.error(e);
    }
  }
</script>

<div class="toasts" role="status" aria-live="polite">
  {#if $currentToast}
    {#key $currentToast.id}
      {@const t = $currentToast}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="ewe-toast ewe-toast--{t.tone}" onkeydown={(e) => e.key === "Escape" && dismissToast(t.id)}>
        <Icon name={glyph[t.tone] || "info"} />
        <span class="ewe-toast__text">
          {#each parts(t.message) as p}{#if p.b}<b>{p.t}</b>{:else}{p.t}{/if}{/each}
        </span>
        {#if t.action}
          <button class="ewe-toast__action" onclick={() => act(t)}>{t.action.label}</button>
        {/if}
        <span class="ewe-toast__sep"></span>
        <button class="ewe-iconbtn ewe-iconbtn--ghost ewe-iconbtn--sm" aria-label="Close" title="Close" onclick={() => dismissToast(t.id)}>
          <Icon name="x" />
        </button>
        <span
          class="ewe-toast__timer"
          style="animation-duration: {t.timeout}ms"
          onanimationend={() => dismissToast(t.id)}
        ></span>
      </div>
    {/key}
  {/if}
</div>
