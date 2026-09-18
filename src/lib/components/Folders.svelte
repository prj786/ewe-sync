<script>
  import { onMount } from "svelte";
  import * as api from "../api";
  import { cloud, folders, busy, toast, fmtTime } from "../stores";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Row from "./ui/Row.svelte";
  import Alert from "./ui/Alert.svelte";
  import Empty from "./ui/Empty.svelte";
  import IconBtn from "./ui/IconBtn.svelte";
  import Icon from "./ui/Icon.svelte";

  // the list comes from the runner: pairs (from ewe.conf) + this machine's state
  let editing = $state(null); // null | { index: -1 (new) | n, pair: {...}, excludeText }
  let openConflicts = $state({}); // key → bool
  let error = $state("");

  const MODES = [
    ["two-way", "Two-way: the Nextcloud sync engine"],
    ["upload", "Upload only: copy local files to the account"],
    ["download", "Download only: copy the account’s files here"]
  ];
  const TRIGGERS = [
    ["change", "When something changes"],
    ["interval", "Every N minutes"],
    ["login", "Once at login"]
  ];

  async function load() {
    try {
      folders.set(await api.foldersList());
    } catch (e) {
      error = String(e);
    }
  }
  onMount(load);

  const items = $derived($folders?.pairs || []);
  const engines = $derived($folders?.engines || {});

  function blank(remote = "/", local = "~/Nextcloud") {
    return { remote, local, mode: "two-way", trigger: "change", interval: 10, exclude: [] };
  }
  function startAdd(remote, local) {
    editing = { index: -1, pair: blank(remote, local), excludeText: "" };
    error = "";
  }
  function startEdit(i) {
    const p = { ...items[i].pair };
    editing = { index: i, pair: p, excludeText: (p.exclude || []).join("\n") };
    error = "";
  }
  function cancel() {
    editing = null;
    error = "";
  }

  async function savePairs(pairs) {
    if ($busy) return;
    busy.set("folders");
    try {
      folders.set(await api.foldersSet(pairs));
      editing = null;
      error = "";
    } catch (e) {
      error = String(e);
    } finally {
      busy.set("");
    }
  }
  function save() {
    const p = { ...editing.pair, interval: Number(editing.pair.interval) || 10 };
    p.exclude = editing.excludeText.split("\n").map((s) => s.trim()).filter(Boolean);
    const pairs = items.map((x) => x.pair);
    if (editing.index < 0) pairs.push(p);
    else pairs[editing.index] = p;
    savePairs(pairs);
  }
  function remove(i) {
    const pairs = items.map((x) => x.pair);
    pairs.splice(i, 1);
    savePairs(pairs);
  }

  async function runOne(key) {
    try {
      const r = await api.foldersRun(key);
      if (r.ok) toast("Folder synced", "success");
    } catch (e) {
      toast(String(e), "error", 8000);
    }
    load();
  }
  async function resolve(key, path, keep) {
    try {
      folders.set(await api.foldersResolve(key, path, keep));
      toast(keep === "mine" ? "Kept your copy. It uploads on the next run." : "Kept the account’s copy", "success");
    } catch (e) {
      toast(String(e), "error", 8000);
    }
  }

  function statusText(it) {
    if (it.running) return "Syncing…";
    if (it.conflicts?.length) return `${it.conflicts.length} conflict${it.conflicts.length === 1 ? "" : "s"}`;
    if (it.last_error) return "Failed";
    if (it.last_run) return "Up to date";
    return "Never run";
  }
  function statusTone(it) {
    if (it.running) return "ewe-badge--accent";
    if (it.conflicts?.length) return "ewe-badge--warning";
    if (it.last_error) return "ewe-badge--danger";
    if (it.last_run) return "ewe-badge--success";
    return "";
  }
  const MODE_WORD = { "two-way": "Two-way", upload: "Upload only", download: "Download only" };
  const MODE_ICON = { "two-way": "arrowBoth", upload: "arrowUp", download: "arrowDown" };
  function triggerText(p) {
    if (p.trigger === "interval") return `every ${p.interval} min`;
    if (p.trigger === "login") return "at login";
    return "on change";
  }
</script>

{#snippet addAction()}
  <button class="ewe-btn ewe-btn--secondary" disabled={!!$busy} onclick={() => startAdd("", "")}>
    <Icon name="plus" />Add folder
  </button>
{/snippet}

<Page
  title="Folders"
  desc="Folders on this machine kept in step with your account."
  actions={$cloud?.signed_in && items.length && !editing ? addAction : undefined}
>
  {#if !$cloud?.signed_in}
    <Group title="Folders">
      <Empty compact icon="folder" title="Not signed in" desc="Sign in first: folders sync with your account." />
    </Group>
  {:else}
    {#if error}
      <Alert tone="danger" title="Couldn’t save the folders" dismiss={() => (error = "")}>{error}</Alert>
    {/if}

    {#if !items.length && !editing}
      <Group title="Get started" well={false}>
        <div class="ewe-card">
          <div class="ewe-card__head">
            <span class="ewe-card__icon ewe-card__icon--accent"><Icon name="folderSync" /></span>
            <div class="ewe-card__titles">
              <div class="ewe-card__title">Sync ~/Nextcloud with your account</div>
              <div class="ewe-card__desc">
                Two-way, whenever something changes: the whole account in one folder on this machine. Set it up, or
                add any folder pair you like.
              </div>
            </div>
          </div>
          <div class="ewe-card__foot">
            <button class="ewe-btn ewe-btn--ghost" onclick={() => startAdd("", "")}>Add another folder</button>
            <button class="ewe-btn ewe-btn--primary" onclick={() => startAdd("/", "~/Nextcloud")}>Set up</button>
          </div>
        </div>
      </Group>
    {/if}

    {#if items.length}
      <Group title={`Folders · ${items.length}`}>
        {#each items as it, i (it.key)}
          {#if i > 0}<div class="ewe-list__divider"></div>{/if}
          <Row
            class="folder-row"
            sub={`${it.pair.remote || "/"} · ${MODE_WORD[it.pair.mode] || it.pair.mode} · ${triggerText(it.pair)}${it.last_run ? ` · last run ${fmtTime(it.last_run)}` : ""}`}
          >
            {#snippet lead()}
              <span class="ewe-row__lead ewe-row__lead--tile"><Icon name={MODE_ICON[it.pair.mode] || "folder"} /></span>
            {/snippet}
            {#snippet text()}
              <div class="ewe-row__title ewe-mono folder-path" title={it.local_effective}>{it.local_effective}</div>
            {/snippet}
            <span class="ewe-badge {statusTone(it)}"><span class="ewe-badge__label">{statusText(it)}</span></span>
            <span class="iconbtn-row">
              <IconBtn name="refresh" title="Sync now" disabled={it.running || !!$busy} go={() => runOne(it.key)} />
              <IconBtn name="pencil" title="Edit" disabled={!!$busy} go={() => startEdit(i)} />
              <IconBtn name="trash" title="Remove" danger disabled={!!$busy} go={() => remove(i)} />
            </span>
          </Row>
          {#if it.last_error}
            <div class="ewe-row ewe-row--block"><pre class="error-log">{it.last_error}</pre></div>
          {/if}
          {#if it.conflicts?.length}
            <Row
              dense
              title="Both sides changed the same file"
              sub="Keep yours (it replaces the account’s copy on the next run) or keep the account’s."
            >
              <button
                class="ewe-btn ewe-btn--sm ewe-btn--ghost"
                aria-expanded={!!openConflicts[it.key]}
                onclick={() => (openConflicts[it.key] = !openConflicts[it.key])}
              >
                {openConflicts[it.key] ? "Hide" : "Show"}
                {it.conflicts.length} conflict{it.conflicts.length === 1 ? "" : "s"}
              </button>
            </Row>
            {#if openConflicts[it.key]}
              {#each it.conflicts as c (c)}
                <Row dense>
                  {#snippet text()}
                    <div class="ewe-row__title ewe-mono" title={c}>{c}</div>
                  {/snippet}
                  <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" onclick={() => resolve(it.key, c, "mine")}>Keep mine</button>
                  <button class="ewe-btn ewe-btn--sm ewe-btn--ghost" onclick={() => resolve(it.key, c, "theirs")}>Keep theirs</button>
                </Row>
              {/each}
            {/if}
          {/if}
        {/each}
      </Group>
    {/if}

    {#if editing}
      <Group title={editing.index < 0 ? "Add folder" : "Edit folder"} well={false}>
        <div class="ewe-card">
          <label class="ewe-field">
            <span class="ewe-field__label">Local folder on this machine</span>
            <input class="ewe-input ewe-mono" placeholder="~/Documents" bind:value={editing.pair.local} />
          </label>
          <label class="ewe-field">
            <span class="ewe-field__label">Folder in your account</span>
            <input class="ewe-input ewe-mono" placeholder="/ (everything) or /Documents" bind:value={editing.pair.remote} />
          </label>
          <label class="ewe-field">
            <span class="ewe-field__label">Mode</span>
            <select class="ewe-input" bind:value={editing.pair.mode}>
              {#each MODES as [v, l]}<option value={v}>{l}</option>{/each}
            </select>
          </label>
          <div class="form-grid">
            <label class="ewe-field form-grid__wide">
              <span class="ewe-field__label">Run</span>
              <select class="ewe-input" bind:value={editing.pair.trigger}>
                {#each TRIGGERS as [v, l]}<option value={v}>{l}</option>{/each}
              </select>
            </label>
            <label class="ewe-field">
              <span class="ewe-field__label">Minutes</span>
              <input
                class="ewe-input mono-input"
                type="number"
                min="1"
                max="1440"
                disabled={editing.pair.trigger !== "interval"}
                bind:value={editing.pair.interval}
              />
            </label>
          </div>
          <label class="ewe-field">
            <span class="ewe-field__label">Exclude <span class="ewe-field__optional">optional</span></span>
            <textarea
              class="ewe-input ewe-input--multiline ewe-mono"
              rows="3"
              placeholder={".git\nnode_modules"}
              bind:value={editing.excludeText}
            ></textarea>
            <span class="ewe-field__helper">One pattern per line.</span>
          </label>
          {#if editing.pair.mode === "two-way" && engines.nextcloudcmd === false}
            <Alert tone="warning" title="nextcloudcmd isn’t installed">Two-way sync needs the nextcloud-client package.</Alert>
          {:else if editing.pair.mode !== "two-way" && engines.rclone === false}
            <Alert tone="warning" title="rclone isn’t installed">One-way folders need it.</Alert>
          {/if}
          <div class="ewe-card__foot">
            <button class="ewe-btn ewe-btn--ghost" onclick={cancel}>Cancel</button>
            <button class="ewe-btn ewe-btn--primary" disabled={!!$busy} onclick={save}>Save</button>
          </div>
        </div>
      </Group>
    {/if}

    <p class="note">
      Two-way folders run the Nextcloud sync engine (nextcloudcmd); one-way folders copy with rclone and never delete.
      Folder definitions live in ewe.conf and follow you to every machine; the local path can differ per machine.
    </p>
  {/if}
</Page>
