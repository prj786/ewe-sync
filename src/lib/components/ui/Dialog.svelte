<script>
  /**
   * A confirmation dialog (design/system/components/Dialog), one for the
   * whole app instead of three hand-written copies: a full-window scrim, the
   * decision as a question, the consequence, then [Cancel] [verb]. Modal:
   * Tab stays inside, Esc and Cancel answer no, Cancel has the first focus
   * (nothing runs by a stray Enter). `tone` "danger" is for actions that
   * remove or stop something; "accent" for everything else (a restore keeps
   * the old copy, so it is not danger).
   */
  import { onMount, tick } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    title = "",
    icon = "info",
    tone = "accent",
    confirm = "OK",
    cancel = "Cancel",
    onConfirm = () => {},
    onCancel = () => {},
    children
  } = $props();

  const id = `dlg-${Math.random().toString(36).slice(2, 8)}`;
  let el = $state(null);
  let cancelBtn = $state(null);
  let opener = null;

  onMount(() => {
    opener = document.activeElement;
    tick().then(() => cancelBtn?.focus());
    return () => {
      if (opener && document.contains(opener)) opener.focus();
    };
  });

  function onKey(e) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onCancel();
      return;
    }
    if (e.key !== "Tab") return;
    const f = [...el.querySelectorAll("button:not(:disabled), [href], input:not(:disabled)")];
    if (!f.length) return;
    const first = f[0];
    const last = f[f.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }
</script>

<div class="scrim" aria-hidden="true"></div>
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  bind:this={el}
  class="ewe-dialog is-floating"
  role="alertdialog"
  aria-modal="true"
  aria-labelledby="{id}-t"
  aria-describedby="{id}-d"
  tabindex="-1"
  data-state="open"
  onkeydown={onKey}
>
  <div class="ewe-dialog__head">
    <span class="ewe-dialog__icon {tone === 'danger' ? 'ewe-dialog__icon--danger' : 'ewe-dialog__icon--accent'}"><Icon name={icon} /></span>
    <div class="ewe-dialog__titles">
      <h2 class="ewe-dialog__title" id="{id}-t">{title}</h2>
      <p class="ewe-dialog__desc" id="{id}-d">{@render children?.()}</p>
    </div>
  </div>
  <div class="ewe-dialog__foot">
    <button bind:this={cancelBtn} type="button" class="ewe-btn ewe-btn--ghost" onclick={onCancel}>{cancel}</button>
    <button type="button" class="ewe-btn {tone === 'danger' ? 'ewe-btn--danger' : 'ewe-btn--primary'}" onclick={onConfirm}>{confirm}</button>
  </div>
</div>
