<script>
  // Add or change one connected folder, in a Sheet beside the list (the
  // list stays put): the folder on this computer (typed, or Choose… — the
  // system's folder chooser), the folder in your account (typed, or Browse…
  // your account — a second step inside the sheet with its own back arrow;
  // a new name is created on the first sync), and how they keep in step.
  // The rest — when it runs, what it skips, hidden files — waits under
  // "More options".
  import { open as chooseFolder } from "@tauri-apps/plugin-dialog";
  import * as api from "../api";
  import { fmtBytes } from "../stores";
  import Alert from "./ui/Alert.svelte";
  import Check from "./ui/Check.svelte";
  import Icon from "./ui/Icon.svelte";
  import Sheet from "./ui/Sheet.svelte";
  import RemotePicker from "./RemotePicker.svelte";

  let {
    initial,
    name = "",
    engines = {},
    isNew = true,
    busy = false,
    error = "",
    onSave = () => {},
    onCancel = () => {}
  } = $props();

  let pair = $state({ remote: "", local: "", mode: "two-way", trigger: "change", interval: 10, exclude: [], hidden: false });
  let excludeText = $state("");
  let more = $state(false);
  let browsing = $state(false);
  let remoteTouched = $state(false);
  let localSize = $state(null); // { bytes, files, more }
  // the picker's place, for the sheet's footer
  let pickPath = $state("");
  let pickLoading = $state(false);
  let pickFree = $state(null);

  $effect.pre(() => {
    const p = { remote: "", local: "", mode: "two-way", trigger: "change", interval: 10, exclude: [], hidden: false, ...(initial || {}) };
    pair = p;
    excludeText = (p.exclude || []).join("\n");
    remoteTouched = !isNew || !!p.remote;
    more = p.trigger !== "change" || (p.exclude || []).length > 0 || !!p.hidden;
    browsing = false;
  });

  // the account folder follows the local folder's name until it is typed
  function setLocal(v) {
    pair.local = v;
    if (!remoteTouched) {
      const n = String(v).replace(/\/+$/, "").split("/").pop() || "";
      pair.remote = n === "~" ? "" : n;
    }
  }

  async function choose() {
    try {
      const home = String(pair.local || "~").startsWith("/") ? pair.local : undefined;
      const picked = window.__EWE_SYNC_MOCK__ ? "/home/scubba/Work" : await chooseFolder({ directory: true, defaultPath: home, title: "Choose a folder to keep in your account" });
      if (picked) setLocal(String(picked).replace(/^\/home\/[^/]+(?=\/|$)/, "~"));
    } catch {
      /* the chooser was closed */
    }
  }

  $effect(() => {
    const l = String(pair.local || "").trim();
    localSize = null;
    if (!l || l.includes("..")) return;
    const t = setTimeout(() => {
      api.folderSize(l).then((z) => (localSize = z)).catch(() => {});
    }, 400);
    return () => clearTimeout(t);
  });

  const MODES = [
    ["two-way", "Both ways"],
    ["upload", "Upload only"],
    ["download", "Download only"]
  ];
  const MODE_HELP = {
    "two-way": "Changes on this computer and in your account meet: edit, add, rename or delete on either side.",
    upload: "Copies this computer’s files to your account. Nothing is ever deleted, and changes in the account stay there.",
    download: "Copies your account’s files here. Nothing is ever deleted, and changes here stay here."
  };
  const TRIGGERS = [
    ["change", "As soon as something changes"],
    ["interval", "Every few minutes"],
    ["login", "Once, when you sign in"]
  ];

  function save() {
    if (busy || !String(pair.local || "").trim()) return;
    const out = {
      ...pair,
      remote: String(pair.remote || "").trim(),
      local: String(pair.local || "").trim(),
      interval: Number(pair.interval) || 10,
      exclude: excludeText.split("\n").map((s) => s.trim()).filter(Boolean)
    };
    if (!out.hidden) delete out.hidden;
    onSave(out);
  }
  function usePick() {
    pair.remote = pickPath;
    remoteTouched = true;
    browsing = false;
  }
  const pickName = $derived(pickPath ? pickPath.split("/").pop() : "");
</script>

<Sheet
  title={browsing ? "Choose a folder in your account" : isNew ? "Add a folder" : `Change ${name || "folder"}`}
  back={browsing ? () => (browsing = false) : null}
  onClose={onCancel}
>
  {#if browsing}
    <RemotePicker start={pair.remote} bind:path={pickPath} bind:loading={pickLoading} bind:free={pickFree} />
  {:else}
    <label class="ewe-field">
      <span class="ewe-field__label">Folder on this computer</span>
      <span class="signin-line">
        <input
          class="ewe-input ewe-mono"
          placeholder="A folder, like ~/Documents"
          value={pair.local}
          oninput={(e) => setLocal(e.currentTarget.value)}
        />
        <button class="ewe-btn ewe-btn--secondary" type="button" onclick={choose}><Icon name="folderOpen" />Choose…</button>
      </span>
      {#if localSize}
        <span class="ewe-field__helper">{localSize.more ? "More than " : ""}{fmtBytes(localSize.bytes)} in {localSize.files} file{localSize.files === 1 ? "" : "s"}</span>
      {/if}
    </label>

    <label class="ewe-field">
      <span class="ewe-field__label">Folder in your account</span>
      <span class="signin-line">
        <input
          class="ewe-input ewe-mono"
          placeholder="Empty: your whole account"
          value={pair.remote}
          oninput={(e) => {
            remoteTouched = true;
            pair.remote = e.currentTarget.value;
          }}
        />
        <button class="ewe-btn ewe-btn--secondary" type="button" onclick={() => (browsing = true)}>
          <Icon name="cloud" />Browse…
        </button>
      </span>
      <span class="ewe-field__helper">A name that isn’t in your account yet is created on the first sync.</span>
    </label>

    <div class="ewe-field">
      <span class="ewe-field__label">Keep in step</span>
      <div class="ewe-seg ewe-seg--block ewe-seg--accent" role="radiogroup" aria-label="Keep in step">
        {#each MODES as [v, l]}
          <button
            type="button"
            role="radio"
            aria-checked={pair.mode === v}
            class="ewe-seg__item"
            class:is-selected={pair.mode === v}
            onclick={() => (pair.mode = v)}>{l}</button
          >
        {/each}
      </div>
      <span class="ewe-field__helper">{MODE_HELP[pair.mode]}</span>
    </div>

    <button class="ewe-link more-toggle" type="button" aria-expanded={more} onclick={() => (more = !more)}>
      <Icon name={more ? "caretDown" : "caretRight"} />More options
    </button>
    {#if more}
      <div class="form-grid" class:form-grid--one={pair.trigger !== "interval"}>
        <label class="ewe-field form-grid__wide">
          <span class="ewe-field__label">When to sync</span>
          <select class="ewe-input" bind:value={pair.trigger}>
            {#each TRIGGERS as [v, l]}<option value={v}>{l}</option>{/each}
          </select>
        </label>
        {#if pair.trigger === "interval"}
          <label class="ewe-field">
            <span class="ewe-field__label">Minutes</span>
            <input class="ewe-input mono-input" type="number" min="1" max="1440" bind:value={pair.interval} />
          </label>
        {/if}
      </div>
      <label class="ewe-field">
        <span class="ewe-field__label">Skip <span class="ewe-field__optional">optional</span></span>
        <textarea class="ewe-input ewe-input--multiline ewe-mono" rows="3" placeholder={"node_modules\n*.tmp"} bind:value={excludeText}></textarea>
        <span class="ewe-field__helper">One name or pattern per line. These never sync.</span>
      </label>
      {#if pair.mode === "two-way"}
        <Check
          checked={!!pair.hidden}
          label="Include hidden files"
          desc="Files and folders whose name starts with a dot."
          toggled={(v) => (pair.hidden = v)}
        />
      {/if}
    {/if}

    {#if pair.mode === "two-way" && engines.nextcloudcmd === false}
      <Alert tone="warning" title="A sync tool is missing">Both ways needs the nextcloud-client package. Install it with Komble.</Alert>
    {:else if pair.mode !== "two-way" && engines.rclone === false}
      <Alert tone="warning" title="A sync tool is missing">Upload only and download only need rclone. Install it with Komble.</Alert>
    {/if}
    {#if error}
      <Alert tone="danger" title={isNew ? "Couldn’t add the folder" : "Couldn’t save the folder"}>{error}</Alert>
    {/if}
  {/if}

  {#snippet foot()}
    {#if browsing}
      {#if pickFree != null && pickFree >= 0 && !pickPath}
        <span class="sheet-foot__note">{fmtBytes(pickFree)} free in your account</span>
      {/if}
      <button class="ewe-btn ewe-btn--primary" type="button" aria-disabled={pickLoading} onclick={() => !pickLoading && usePick()}>
        Use {pickPath ? `“${pickName}”` : "the whole account"}
      </button>
    {:else}
      <button class="ewe-btn ewe-btn--ghost" type="button" onclick={onCancel}>Cancel</button>
      <button
        class="ewe-btn ewe-btn--primary"
        type="button"
        aria-busy={busy}
        disabled={!String(pair.local || "").trim()}
        aria-disabled={busy}
        onclick={save}
      >
        {#if busy}<span class="ewe-spinner ewe-spinner--sm ewe-spinner--on-accent" aria-hidden="true"></span>{/if}{isNew ? "Start syncing" : "Save"}
      </button>
    {/if}
  {/snippet}
</Sheet>
