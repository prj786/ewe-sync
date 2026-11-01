<script>
  // Folders — every folder connected to the account, on this computer: the
  // ones ewe Sync keeps in step, the ones the Nextcloud app still syncs (with
  // a way to move them here), and the whole account as ~/Nextcloud. Adding
  // one is a click for the usual folders (Suggested), Add folder for any
  // other — a sheet beside the list, so the list stays put — or "Keep in
  // your account…" in the file manager (`ewe-sync --add-folder`).
  // Each row says its state and keeps its actions in one ⋯ menu. A folder
  // that changed in two places, or couldn't sync, carries an alert right
  // under it in plain words, with the choices side by side and the engine's
  // own words behind Details.
  // The list lives in ewe.conf, so every computer you sign in to gets it;
  // pausing, and a different local path, are this computer's own.
  import { onMount, tick } from "svelte";
  import * as api from "../api";
  import {
    route, cloud, folders, busy, toast, fmtTime, fmtBytes, pendingAdd, tildePath, focusFolder, folderError, conflictInfo
  } from "../stores";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Row from "./ui/Row.svelte";
  import Alert from "./ui/Alert.svelte";
  import Empty from "./ui/Empty.svelte";
  import Menu from "./ui/Menu.svelte";
  import Dialog from "./ui/Dialog.svelte";
  import Icon from "./ui/Icon.svelte";
  import FolderForm from "./FolderForm.svelte";

  let editing = $state(null); // null | { index: -1 (new) | n, pair }
  let formError = $state("");
  let suggestions = $state([]);
  let sizes = $state({}); // local path → { bytes, files, more }
  let confirmRemove = $state(null); // an item of the list
  let confirmTakeover = $state(false);
  let details = $state({}); // key → the engine's words shown
  let allFiles = $state({}); // key → every conflicted file shown
  let attach = {}; // key → the alert under that row (focus target)

  function loadSuggestions() {
    api.foldersSuggest()
      .then((r) => {
        suggestions = r.items || [];
        for (const s of suggestions) {
          if (sizes[s.local]) continue;
          api.folderSize(s.local).then((z) => (sizes = { ...sizes, [s.local]: z })).catch(() => {});
        }
      })
      .catch(() => (suggestions = []));
  }
  async function load() {
    try {
      folders.set(await api.foldersList());
    } catch {
      /* outside the desktop */
    }
    loadSuggestions();
  }
  onMount(load);

  const items = $derived($folders?.pairs || []);
  const appFolders = $derived($folders?.app?.folders || []);
  const engines = $derived($folders?.engines || {});
  const af = $derived($folders?.account_folder || null);
  const count = $derived(items.length + appFolders.length);

  // `ewe-sync --add-folder <path>` (the file manager): the sheet, pre-filled
  $effect(() => {
    const p = $pendingAdd;
    if (!p) return;
    pendingAdd.set("");
    const local = tildePath(p);
    const name = local.replace(/\/+$/, "").split("/").pop() || "";
    const existing = items.findIndex((it) => it.local_effective === p);
    if (existing >= 0) {
      toast(`**${name}** is already in your account`, "info");
      return;
    }
    editing = { index: -1, pair: { local, remote: name, mode: "two-way", trigger: "change", interval: 10, exclude: [] } };
    formError = "";
  });

  // Overview's "Choose a copy" / "See details": bring that folder's alert
  // into view and put the keyboard on its first choice
  $effect(() => {
    const key = $focusFolder;
    if (!key || !items.length) return;
    focusFolder.set("");
    details = { ...details, [key]: true };
    tick().then(() => {
      const el = attach[key];
      if (!el) return;
      el.scrollIntoView({ block: "center" });
      el.querySelector("button")?.focus({ preventScroll: true });
    });
  });

  function startAdd() {
    if (editing?.index === -1) return;
    editing = { index: -1, pair: null };
    formError = "";
  }
  function startEdit(i) {
    editing = { index: i, pair: { ...items[i].pair } };
    formError = "";
  }

  async function savePairs(pairs, done) {
    if ($busy) return false;
    busy.set("folders");
    try {
      folders.set(await api.foldersSet(pairs));
      formError = "";
      if (done) toast(done, "success");
      loadSuggestions();
      return true;
    } catch (e) {
      formError = String(e);
      if (!editing) toast(String(e), "error", 8000);
      return false;
    } finally {
      busy.set("");
    }
  }
  async function save(pair) {
    const pairs = items.map((x) => x.pair);
    const isNew = editing.index < 0;
    if (isNew) pairs.push(pair);
    else pairs[editing.index] = pair;
    const name = String(pair.local).replace(/\/+$/, "").split("/").pop();
    if (await savePairs(pairs, isNew ? `**${name}** now syncs with your account` : `Saved **${name}**`)) editing = null;
  }
  function quickAdd(s) {
    const pairs = items.map((x) => x.pair);
    pairs.push({ remote: s.remote, local: s.local, mode: "two-way", trigger: "change", interval: 10, exclude: s.exclude || [] });
    savePairs(pairs, `**${s.name}** now syncs with your account`);
  }
  async function remove() {
    const it = confirmRemove;
    confirmRemove = null;
    if (!it) return;
    if (editing && editing.index >= 0 && items[editing.index]?.key === it.key) editing = null;
    await savePairs(items.filter((x) => x.key !== it.key).map((x) => x.pair), `**${it.name}** stopped syncing. Its files stay where they are.`);
  }

  async function runOne(it) {
    try {
      const r = await api.foldersRun(it.key);
      if (r.ok) toast(`**${it.name}** synced`, "success");
    } catch (e) {
      toast(`Couldn’t sync **${it.name}**: ${folderError(e).reason}`, "danger", 8000);
    }
  }
  async function pause(it) {
    try {
      folders.set(await api.foldersPause(it.key, !it.paused));
      toast(it.paused ? `**${it.name}** syncs again` : `**${it.name}** is paused on this computer`, "info");
    } catch (e) {
      toast(String(e), "error");
    }
  }
  async function resolve(it, path, keep) {
    const file = conflictInfo(path).file;
    try {
      folders.set(await api.foldersResolve(it.key, path, keep));
      toast(
        keep === "mine" ? `Kept this computer’s **${file}**. It goes up on the next sync.` : `Kept the account’s **${file}**`,
        "success"
      );
    } catch (e) {
      toast(String(e), "error", 8000);
    }
  }
  async function takeover() {
    confirmTakeover = false;
    if ($busy) return;
    busy.set("takeover");
    try {
      folders.set(await api.foldersTakeover());
      toast("ewe Sync keeps these folders in step now. The Nextcloud app is closed and won’t start at login.", "success", 9000);
    } catch (e) {
      toast(String(e), "error", 9000);
    } finally {
      busy.set("");
    }
  }
  function open(path) {
    api.folderOpen(path).catch((e) => toast(String(e), "error"));
  }

  const STATE = {
    synced: ["Up to date", "ewe-badge--success"],
    syncing: ["Syncing…", "ewe-badge--accent"],
    pending: ["Waiting", ""],
    paused: ["Paused", ""],
    offline: ["Offline", ""],
    error: ["Couldn’t sync", "ewe-badge--danger"],
    conflict: ["Needs a decision", "ewe-badge--warning"]
  };
  const ARROW = { "two-way": "↔", upload: "→", download: "←" };
  const MODE_WORD = { "two-way": "", upload: " · upload only", download: " · download only" };
  function subline(it) {
    const r = String(it.pair.remote || "").replace(/^\/+|\/+$/g, "");
    const where = r || "your whole account";
    let when = "";
    if (it.state === "paused" && !it.paused) when = " · auto-sync is off";
    else if (!it.running && it.last_ok && it.last_run) when = ` · ${fmtTime(it.last_run)}`;
    return `${tildePath(it.local_effective)} ${ARROW[it.pair.mode] || "↔"} ${where}${MODE_WORD[it.pair.mode] || ""}${when}`;
  }
  function menuFor(it, i) {
    return [
      { label: "Open folder", icon: "folderOpen", run: () => open(it.local_effective) },
      { label: "Sync now", icon: "refresh", disabled: it.running || it.paused || !!$busy, run: () => runOne(it) },
      it.paused
        ? { label: "Resume on this computer", icon: "play", disabled: !!$busy, run: () => pause(it) }
        : { label: "Pause on this computer", icon: "pause", disabled: !!$busy, run: () => pause(it) },
      { label: "Change…", icon: "sliders", disabled: !!$busy, run: () => startEdit(i) },
      { divider: true },
      { label: "Stop syncing…", icon: "x", danger: true, disabled: !!$busy, run: () => (confirmRemove = it) }
    ];
  }
  function appMenu(f) {
    return [
      { label: "Open folder", icon: "folderOpen", run: () => open(f.local) },
      { divider: true },
      { label: "Let ewe Sync take over…", icon: "folderSync", disabled: !!$busy, run: () => (confirmTakeover = true) }
    ];
  }
  const KIND_ICON = { desktop: "monitor", documents: "fileText", pictures: "image", music: "music", videos: "video", downloads: "download" };
  const accountDesc = $derived(
    !af ? "" : af.state === "mounted" ? "Files download when opened" : af.state === "synced" ? "Synced to this computer" : "Not opened on this computer"
  );
</script>

{#snippet addAction()}
  <button class="ewe-btn ewe-btn--primary" aria-disabled={!!$busy} onclick={() => !$busy && startAdd()}>
    <Icon name="folderPlus" />Add folder
  </button>
{/snippet}

{#snippet conflictFile(it, path)}
  <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" onclick={() => resolve(it, path, "mine")}>Keep this computer’s</button>
  <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" onclick={() => resolve(it, path, "theirs")}>Keep the account’s</button>
{/snippet}

<Page
  title="Folders"
  desc="Folders on this computer, kept in your account."
  actions={$cloud?.signed_in ? addAction : undefined}
>
  {#if !$cloud?.signed_in}
    <Group title="Your folders">
      <Empty compact icon="folder" title="Not signed in" desc="Sign in to keep folders in your account and on every computer you use.">
        <button class="ewe-btn ewe-btn--secondary ewe-btn--sm" onclick={() => route.set("account")}>Sign in</button>
      </Empty>
    </Group>
  {:else}
    {#if appFolders.length}
      <Alert tone="info" title={`The Nextcloud app syncs ${appFolders.length === 1 ? "a folder" : `${appFolders.length} folders`} here`}>
        Two sync apps on one folder get in each other’s way. ewe Sync can take {appFolders.length === 1 ? "it" : "them"} over:
        the same folders, nothing moves, one place to see them. The Nextcloud app closes and stops starting at login.
        {#snippet actions()}
          <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" aria-disabled={!!$busy} onclick={() => !$busy && (confirmTakeover = true)}>
            {$busy === "takeover" ? "Taking over…" : "Let ewe Sync take over"}
          </button>
        {/snippet}
      </Alert>
    {/if}

    <Group title={count ? `Your folders · ${count}` : "Your folders"}>
      {#if !count}
        <Empty
          compact
          icon="folderSync"
          title="No folders yet"
          desc="Add any folder. It stays where it is on this computer and appears on every computer you sign in to."
        >
          <button class="ewe-btn ewe-btn--secondary ewe-btn--sm" aria-disabled={!!$busy} onclick={() => !$busy && startAdd()}>
            <Icon name="folderPlus" />Add folder
          </button>
        </Empty>
      {/if}
      {#each items as it, i (it.key)}
        {#if i > 0}<div class="ewe-list__divider"></div>{/if}
        <Row title={it.name} sub={subline(it)} class={editing?.index === i ? "is-selected" : ""}>
          {#snippet lead()}
            <span class="ewe-row__lead ewe-row__lead--tile"><Icon name={it.pair.mode === "upload" ? "cloudUpload" : it.pair.mode === "download" ? "cloudDownload" : "folder"} /></span>
          {/snippet}
          <span class="ewe-badge {STATE[it.state]?.[1] || ''}"><span class="ewe-badge__label">{STATE[it.state]?.[0] || it.state}</span></span>
          <Menu label={`${it.name} actions`} items={menuFor(it, i)} />
        </Row>

        {#if it.conflicts?.length}
          {@const first = conflictInfo(it.conflicts[0])}
          <div class="row-attach" bind:this={attach[it.key]}>
            {#if it.conflicts.length === 1}
              <Alert tone="warning" title={`${first.file} changed in two places`}>
                {first.dir ? `In ${first.dir}. ` : ""}Edited here and in your account{first.when ? ` (${fmtTime(first.when)})` : ""}.
                Both copies are kept until you choose.
                {#snippet actions()}{@render conflictFile(it, it.conflicts[0])}{/snippet}
              </Alert>
            {:else}
              <Alert tone="warning" title={`${it.conflicts.length} files changed in two places`}>
                Each was edited here and in your account. Both copies of each are kept until you choose.
                <ul class="conflict-list">
                  {#each allFiles[it.key] ? it.conflicts : it.conflicts.slice(0, 3) as path (path)}
                    {@const c = conflictInfo(path)}
                    <li class="conflict-list__item">
                      <span class="conflict-list__text">
                        <span class="conflict-list__name" title={path}>{c.file}</span>
                        <span class="conflict-list__meta">{[c.dir, c.when ? fmtTime(c.when) : ""].filter(Boolean).join(" · ")}</span>
                      </span>
                      <span class="conflict-list__actions">{@render conflictFile(it, path)}</span>
                    </li>
                  {/each}
                </ul>
                {#snippet actions()}
                  {#if it.conflicts.length > 3}
                    <button class="ewe-btn ewe-btn--sm ewe-btn--ghost" aria-expanded={!!allFiles[it.key]} onclick={() => (allFiles = { ...allFiles, [it.key]: !allFiles[it.key] })}>
                      {allFiles[it.key] ? "Show fewer" : `Show all ${it.conflicts.length}`}
                    </button>
                  {/if}
                {/snippet}
              </Alert>
            {/if}
          </div>
        {:else if it.state === "error"}
          {@const e = folderError(it.last_error)}
          <div class="row-attach" bind:this={attach[it.key]}>
            <Alert tone="danger" title={`Couldn’t sync: ${e.reason}`}>
              {e.hint}
              {#if details[it.key]}<pre class="error-log">{it.last_error}</pre>{/if}
              {#snippet actions()}
                <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" aria-disabled={!!$busy} onclick={() => runOne(it)}>Try again</button>
                {#if e.kind === "app" && appFolders.length}
                  <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" onclick={() => (confirmTakeover = true)}>Let ewe Sync take over…</button>
                {:else if e.kind === "signin"}
                  <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" onclick={() => route.set("account")}>Open Account</button>
                {/if}
                <button class="ewe-btn ewe-btn--sm ewe-btn--ghost" aria-expanded={!!details[it.key]} onclick={() => (details = { ...details, [it.key]: !details[it.key] })}>
                  {details[it.key] ? "Hide details" : "Details"}
                </button>
              {/snippet}
            </Alert>
          </div>
        {/if}
      {/each}
      {#each appFolders as f, j (f.local)}
        {#if items.length || j > 0}<div class="ewe-list__divider"></div>{/if}
        <Row title={f.name} sub={`${tildePath(f.local)} ↔ ${f.remote || "your whole account"} · the Nextcloud app`}>
          {#snippet lead()}
            <span class="ewe-row__lead ewe-row__lead--tile"><Icon name="folder" /></span>
          {/snippet}
          <span class="ewe-badge"><span class="ewe-badge__label">{f.paused ? "Paused" : $folders?.app?.running ? "Nextcloud app" : "App not running"}</span></span>
          <Menu label={`${f.name} actions`} items={appMenu(f)} />
        </Row>
      {/each}
    </Group>

    {#if suggestions.length || af}
      <Group title={suggestions.length ? "Suggested" : "Your whole account"} well={false}>
        <div class="suggest">
          {#each suggestions as s (s.local)}
            {@const z = sizes[s.local]}
            <div class="suggest__item">
              <span class="ewe-row__lead ewe-row__lead--tile"><Icon name={KIND_ICON[s.kind] || "folder"} /></span>
              <span class="suggest__text">
                <span class="suggest__name">{s.name}</span>
                <span class="suggest__desc" title={s.local}>
                  {s.local}{s.in_account ? " · in your account" : z ? ` · ${z.more ? "over " : ""}${fmtBytes(z.bytes)}` : ""}
                </span>
              </span>
              <button class="ewe-btn ewe-btn--secondary ewe-btn--sm" aria-disabled={!!$busy} aria-label={`Add ${s.name}`} onclick={() => !$busy && quickAdd(s)}>Add</button>
            </div>
          {/each}
          {#if af}
            <div class="suggest__item">
              <span class="ewe-row__lead ewe-row__lead--tile"><Icon name="cloud" /></span>
              <span class="suggest__text">
                <span class="suggest__name">Whole account</span>
                <span class="suggest__desc" title={af.state !== "off" ? tildePath(af.path) : undefined}>{accountDesc}</span>
              </span>
              {#if af.state !== "off"}
                <button class="ewe-btn ewe-btn--secondary ewe-btn--sm" aria-label="Open your whole account" onclick={() => open(af.path)}>Open</button>
              {/if}
            </div>
          {/if}
        </div>
      </Group>
    {/if}

    <p class="note">
      Upload only and download only never delete anything. Files that changed in two places keep both copies until you
      choose.
    </p>
  {/if}
</Page>

{#if editing}
  <FolderForm
    initial={editing.pair}
    isNew={editing.index < 0}
    name={editing.index < 0 ? "" : items[editing.index]?.name || ""}
    {engines}
    busy={!!$busy}
    error={formError}
    onSave={save}
    onCancel={() => ((editing = null), (formError = ""))}
  />
{/if}

{#if confirmRemove}
  <Dialog
    title={`Stop syncing ${confirmRemove.name}?`}
    icon="x"
    tone="danger"
    confirm="Stop syncing"
    onConfirm={remove}
    onCancel={() => (confirmRemove = null)}
  >
    Nothing is deleted: the files stay on this computer and in your account. Your other computers stop syncing it too.
  </Dialog>
{/if}

{#if confirmTakeover}
  <Dialog
    title="Let ewe Sync take over?"
    icon="folderSync"
    confirm="Take over"
    onConfirm={takeover}
    onCancel={() => (confirmTakeover = false)}
  >
    The Nextcloud app closes and no longer starts at login. {appFolders.map((f) => tildePath(f.local)).join(", ")}
    {appFolders.length === 1 ? "keeps" : "keep"} syncing through ewe Sync. Your files stay where they are; the first sync
    only compares them.
  </Dialog>
{/if}
