<script>
  // Folders — every folder connected to the account, on this computer: the
  // ones ewe-sync keeps in step, the ones the Nextcloud app still syncs (with
  // a way to move them here), and the whole account as ~/Nextcloud. Adding
  // one is a click for the usual folders (Suggested), Choose… for any other,
  // or "Keep in your account…" in the file manager (`ewe-sync --add-folder`).
  // The list lives in ewe.conf, so every computer you sign in to gets it;
  // pausing, and a different local path, are this computer's own.
  import { onMount } from "svelte";
  import * as api from "../api";
  import { cloud, folders, busy, toast, fmtTime, fmtBytes, pendingAdd, tildePath } from "../stores";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Row from "./ui/Row.svelte";
  import Alert from "./ui/Alert.svelte";
  import Empty from "./ui/Empty.svelte";
  import IconBtn from "./ui/IconBtn.svelte";
  import Icon from "./ui/Icon.svelte";
  import FolderForm from "./FolderForm.svelte";

  let editing = $state(null); // null | { index: -1 (new) | n, pair }
  let formError = $state("");
  let openConflicts = $state({}); // key → bool
  let suggestions = $state([]);
  let confirmRemove = $state(null); // an item of the list
  let confirmTakeover = $state(false);

  async function load() {
    try {
      folders.set(await api.foldersList());
    } catch {
      /* outside the desktop */
    }
    api.foldersSuggest().then((r) => (suggestions = r.items || [])).catch(() => (suggestions = []));
  }
  onMount(load);

  const items = $derived($folders?.pairs || []);
  const appFolders = $derived($folders?.app?.folders || []);
  const engines = $derived($folders?.engines || {});
  const af = $derived($folders?.account_folder || null);

  // `ewe-sync --add-folder <path>` (the file manager): the form, pre-filled
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

  function startAdd() {
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
      api.foldersSuggest().then((r) => (suggestions = r.items || [])).catch(() => {});
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
    if (await savePairs(pairs, isNew ? `**${name}** now syncs with your account` : "Saved")) editing = null;
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
    await savePairs(items.filter((x) => x.key !== it.key).map((x) => x.pair), `**${it.name}** stopped syncing. Its files stay where they are.`);
  }

  async function runOne(key) {
    try {
      const r = await api.foldersRun(key);
      if (r.ok) toast("Folder synced", "success");
    } catch (e) {
      toast(String(e), "error", 8000);
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
  async function resolve(key, path, keep) {
    try {
      folders.set(await api.foldersResolve(key, path, keep));
      toast(keep === "mine" ? "Kept your copy. It goes up on the next sync." : "Kept the account’s copy", "success");
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
      toast("ewe-sync keeps these folders in step now. The Nextcloud app is closed and won’t start at login.", "success", 9000);
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
  const MODE_WORD = { "two-way": "", upload: " · upload only", download: " · download only" };
  function subline(it) {
    const r = String(it.pair.remote || "").replace(/^\/+|\/+$/g, "");
    const where = r ? `${r} in your account` : "your whole account";
    const when = it.state === "paused" && !it.paused ? " · auto-sync is off" : it.last_ok && it.last_run ? ` · ${fmtTime(it.last_run)}` : "";
    return `${tildePath(it.local_effective)} ↔ ${where}${MODE_WORD[it.pair.mode] || ""}${when}`;
  }
</script>

{#snippet addAction()}
  <button class="ewe-btn ewe-btn--primary" disabled={!!$busy || !!editing} onclick={startAdd}>
    <Icon name="folderPlus" />Add folder
  </button>
{/snippet}

<Page
  title="Folders"
  desc="Folders on this computer kept in your account. Every computer you sign in to gets the same folders."
  actions={$cloud?.signed_in ? addAction : undefined}
>
  {#if !$cloud?.signed_in}
    <Group title="Folders">
      <Empty compact icon="folder" title="Not signed in" desc="Sign in first: folders sync with your account." />
    </Group>
  {:else}
    {#if appFolders.length}
      <Alert tone="info" title={`The Nextcloud app syncs ${appFolders.length === 1 ? "a folder" : `${appFolders.length} folders`} here`}>
        Two sync apps on one folder get in each other’s way. ewe-sync can take {appFolders.length === 1 ? "it" : "them"} over:
        the same folders, nothing moves, one place to see them. The Nextcloud app closes and stops starting at login.
        {#snippet actions()}
          <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" disabled={!!$busy} onclick={() => (confirmTakeover = true)}>
            {$busy === "takeover" ? "Taking over…" : "Let ewe-sync take over"}
          </button>
        {/snippet}
      </Alert>
    {/if}

    {#if editing}
      <Group title={editing.index < 0 ? "Add a folder" : `Change ${items[editing.index]?.name || "folder"}`} well={false}>
        <FolderForm
          initial={editing.pair}
          isNew={editing.index < 0}
          {engines}
          busy={!!$busy}
          error={formError}
          onSave={save}
          onCancel={() => ((editing = null), (formError = ""))}
        />
      </Group>
    {:else if suggestions.length}
      <Group title="Suggested" desc="One click keeps a folder in your account, right where it is." well={false}>
        <div class="suggest">
          {#each suggestions as s (s.local)}
            <button class="ewe-card ewe-card--interactive suggest__item" disabled={!!$busy} onclick={() => quickAdd(s)}>
              <span class="ewe-card__icon"><Icon name="folder" /></span>
              <span class="suggest__text">
                <span class="suggest__name">{s.name}</span>
                <span class="suggest__desc">{s.in_account ? `In your account · ${fmtBytes(s.remote_size || 0)}` : s.local}</span>
              </span>
              <Icon name="plus" />
            </button>
          {/each}
        </div>
      </Group>
    {/if}

    {#if items.length || appFolders.length}
      <Group title={`Connected · ${items.length + appFolders.length}`}>
        {#each items as it, i (it.key)}
          {#if i > 0}<div class="ewe-list__divider"></div>{/if}
          <Row class="folder-row" title={it.name} sub={subline(it)}>
            {#snippet lead()}
              <span class="ewe-row__lead ewe-row__lead--tile"><Icon name={it.pair.mode === "upload" ? "cloudUpload" : it.pair.mode === "download" ? "cloudDownload" : "folder"} /></span>
            {/snippet}
            <span class="ewe-badge {STATE[it.state]?.[1] || ''}"><span class="ewe-badge__label">{STATE[it.state]?.[0] || it.state}</span></span>
            <span class="iconbtn-row">
              <IconBtn name="folderOpen" title="Open" go={() => open(it.local_effective)} />
              <IconBtn name="refresh" title="Sync now" disabled={it.running || it.paused || !!$busy} go={() => runOne(it.key)} />
              <IconBtn name={it.paused ? "play" : "pause"} title={it.paused ? "Resume on this computer" : "Pause on this computer"} disabled={!!$busy} go={() => pause(it)} />
              <IconBtn name="pencil" title="Change" disabled={!!$busy || !!editing} go={() => startEdit(i)} />
              <IconBtn name="trash" title="Stop syncing" danger disabled={!!$busy} go={() => (confirmRemove = it)} />
            </span>
          </Row>
          {#if it.last_error && it.state !== "paused"}
            <div class="ewe-row ewe-row--block"><pre class="error-log">{it.last_error}</pre></div>
          {/if}
          {#if it.conflicts?.length}
            <Row dense title="Changed in two places" sub="Both copies are kept. Keep yours (it replaces the account’s on the next sync) or the account’s.">
              <button
                class="ewe-btn ewe-btn--sm ewe-btn--ghost"
                aria-expanded={!!openConflicts[it.key]}
                onclick={() => (openConflicts[it.key] = !openConflicts[it.key])}
              >
                {openConflicts[it.key] ? "Hide" : "Show"}
                {it.conflicts.length} file{it.conflicts.length === 1 ? "" : "s"}
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
        {#each appFolders as f, j (f.local)}
          {#if items.length || j > 0}<div class="ewe-list__divider"></div>{/if}
          <Row title={f.name} sub={`${tildePath(f.local)} ↔ ${f.remote ? `${f.remote} in your account` : "your whole account"} · the Nextcloud app`}>
            {#snippet lead()}
              <span class="ewe-row__lead ewe-row__lead--tile"><Icon name="folder" /></span>
            {/snippet}
            <span class="ewe-badge"><span class="ewe-badge__label">{f.paused ? "Paused" : $folders?.app?.running ? "Nextcloud app" : "App not running"}</span></span>
            <span class="iconbtn-row">
              <IconBtn name="folderOpen" title="Open" go={() => open(f.local)} />
            </span>
          </Row>
        {/each}
      </Group>
    {:else if !editing}
      <Group title="Connected">
        <Empty
          compact
          icon="folderSync"
          title="No folders yet"
          desc="Pick a suggested folder above, or add any folder. It stays where it is on this computer."
        />
      </Group>
    {/if}

    {#if af}
      <Group title="Your whole account">
        <Row icon="cloud" title="Everything in your account" sub={af.state === "mounted"
          ? `${tildePath(af.path)} · opens your whole account as a folder; files download when you open them`
          : af.state === "synced"
            ? `${tildePath(af.path)} · synced to this computer`
            : "Not opened as a folder on this computer"}>
          {#if af.state !== "off"}
            <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" onclick={() => open(af.path)}>Open</button>
          {/if}
        </Row>
      </Group>
    {/if}

    <p class="note">
      Both ways uses the Nextcloud sync engine; upload only and download only never delete anything. Files that changed
      in two places keep both copies until you choose.
    </p>
  {/if}
</Page>

{#if confirmRemove}
  <div class="scrim"></div>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="ewe-dialog is-floating"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="rm-title"
    aria-describedby="rm-desc"
    tabindex="-1"
    onkeydown={(e) => e.key === "Escape" && (confirmRemove = null)}
  >
    <div class="ewe-dialog__head">
      <span class="ewe-dialog__icon ewe-dialog__icon--danger"><Icon name="trash" /></span>
      <div class="ewe-dialog__titles">
        <h2 class="ewe-dialog__title" id="rm-title">Stop syncing {confirmRemove.name}?</h2>
        <p class="ewe-dialog__desc" id="rm-desc">
          Nothing is deleted: the files stay on this computer and in your account. Your other computers stop syncing it
          too.
        </p>
      </div>
    </div>
    <div class="ewe-dialog__foot">
      <!-- svelte-ignore a11y_autofocus -->
      <button class="ewe-btn ewe-btn--ghost" autofocus onclick={() => (confirmRemove = null)}>Cancel</button>
      <button class="ewe-btn ewe-btn--danger" onclick={remove}>Stop syncing</button>
    </div>
  </div>
{/if}

{#if confirmTakeover}
  <div class="scrim"></div>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="ewe-dialog is-floating"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="to-title"
    aria-describedby="to-desc"
    tabindex="-1"
    onkeydown={(e) => e.key === "Escape" && (confirmTakeover = false)}
  >
    <div class="ewe-dialog__head">
      <span class="ewe-dialog__icon ewe-dialog__icon--accent"><Icon name="folderSync" /></span>
      <div class="ewe-dialog__titles">
        <h2 class="ewe-dialog__title" id="to-title">Let ewe-sync take over?</h2>
        <p class="ewe-dialog__desc" id="to-desc">
          The Nextcloud app closes and no longer starts at login. {appFolders.map((f) => tildePath(f.local)).join(", ")}
          {appFolders.length === 1 ? "keeps" : "keep"} syncing through ewe-sync. Your files stay where they are; the first
          sync only compares them.
        </p>
      </div>
    </div>
    <div class="ewe-dialog__foot">
      <!-- svelte-ignore a11y_autofocus -->
      <button class="ewe-btn ewe-btn--ghost" autofocus onclick={() => (confirmTakeover = false)}>Cancel</button>
      <button class="ewe-btn ewe-btn--primary" onclick={takeover}>Take over</button>
    </div>
  </div>
{/if}
