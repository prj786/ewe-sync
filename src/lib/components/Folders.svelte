<script>
  import { onMount } from "svelte";
  import * as api from "../api";
  import { cloud, folders, busy, toast, fmtTime } from "../stores";

  // the list comes from the runner: pairs (from ewe.conf) + this machine's state
  let editing = $state(null); // null | { index: -1 (new) | n, pair: {...}, excludeText }
  let openConflicts = $state({}); // key → bool
  let error = $state("");

  const MODES = [
    ["two-way", "Two-way — the Nextcloud sync engine"],
    ["upload", "Upload only — copy local files to the account"],
    ["download", "Download only — copy the account's files here"]
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
      toast(keep === "mine" ? "Kept your copy — it uploads on the next run" : "Kept the account's copy", "success");
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
  function statusClass(it) {
    if (it.running) return "text-sky-400";
    if (it.conflicts?.length) return "text-amber-500";
    if (it.last_error) return "text-red-400";
    return "text-zinc-500 dark:text-zinc-400";
  }
  function triggerText(p) {
    if (p.trigger === "interval") return `every ${p.interval} min`;
    if (p.trigger === "login") return "at login";
    return "on change";
  }
</script>

<div class="mx-auto max-w-2xl">
  <div class="section-title">Folders</div>

  {#if !$cloud?.signed_in}
    <div class="card px-4 py-4 text-sm text-zinc-500 dark:text-zinc-400">Sign in first — folders sync with your account.</div>
  {:else}
    {#if error}
      <div class="mb-3 rounded-lg border border-red-500/40 bg-red-500/10 px-3 py-2 text-xs text-red-400">{error}</div>
    {/if}

    {#if !items.length && !editing}
      <div class="card px-4 py-4 text-sm">
        <div class="font-medium">Sync ~/Nextcloud with your account</div>
        <div class="mt-1 text-xs text-zinc-500 dark:text-zinc-400">
          Two-way, whenever something changes — the whole account into one folder on this machine. Opt in below, or add any folder pair you like.
        </div>
        <div class="mt-3 flex gap-2">
          <button class="btn-primary" onclick={() => startAdd("/", "~/Nextcloud")}>Set up</button>
          <button class="btn-ghost" onclick={() => startAdd("", "")}>Add another folder</button>
        </div>
      </div>
    {/if}

    {#each items as it, i (it.key)}
      <div class="card mb-3 divide-y divide-zinc-200 dark:divide-zinc-700/60">
        <div class="flex items-center gap-3 px-4 py-3">
          <span class="ph-i text-[18px] text-zinc-500">{String.fromCodePoint(0xe256)}</span>
          <div class="min-w-0 flex-1">
            <div class="truncate font-mono text-xs">{it.local_effective}</div>
            <div class="truncate text-xs text-zinc-500 dark:text-zinc-400">
              {it.pair.mode === "download" ? "←" : it.pair.mode === "upload" ? "→" : "↔"}
              {it.pair.remote || "/"} · {it.pair.mode} · {triggerText(it.pair)}
              {#if it.last_run}· last run {fmtTime(it.last_run)}{/if}
            </div>
          </div>
          <span class="shrink-0 text-xs {statusClass(it)}">{statusText(it)}</span>
          <button class="btn-ghost !py-1 text-xs" disabled={it.running || !!$busy} onclick={() => runOne(it.key)}>Sync now</button>
          <button class="btn-ghost !py-1 text-xs" disabled={!!$busy} onclick={() => startEdit(i)}>Edit</button>
          <button class="btn-ghost !py-1 text-xs text-red-400" disabled={!!$busy} onclick={() => remove(i)}>Remove</button>
        </div>
        {#if it.last_error}
          <div class="px-4 py-2 font-mono text-[11px] whitespace-pre-wrap text-red-400">{it.last_error}</div>
        {/if}
        {#if it.conflicts?.length}
          <div class="px-4 py-2">
            <button class="text-xs text-amber-500" onclick={() => (openConflicts[it.key] = !openConflicts[it.key])}>
              {openConflicts[it.key] ? "Hide" : "Show"} {it.conflicts.length} conflict{it.conflicts.length === 1 ? "" : "s"}
            </button>
            {#if openConflicts[it.key]}
              <div class="mt-2 text-xs text-zinc-500 dark:text-zinc-400">
                Both sides changed the same file. Keep yours (it replaces the account's copy on the next run) or keep the account's.
              </div>
              {#each it.conflicts as c (c)}
                <div class="mt-1 flex items-center gap-2">
                  <span class="min-w-0 flex-1 truncate font-mono text-[11px]">{c}</span>
                  <button class="btn-ghost !py-0.5 text-xs" onclick={() => resolve(it.key, c, "mine")}>Keep mine</button>
                  <button class="btn-ghost !py-0.5 text-xs" onclick={() => resolve(it.key, c, "theirs")}>Keep theirs</button>
                </div>
              {/each}
            {/if}
          </div>
        {/if}
      </div>
    {/each}

    {#if items.length && !editing}
      <button class="btn-ghost text-xs" onclick={() => startAdd("", "")}>+ Add folder</button>
    {/if}

    {#if editing}
      <div class="card mt-3 px-4 py-4">
        <div class="mb-3 text-sm font-medium">{editing.index < 0 ? "Add folder" : "Edit folder"}</div>
        <div class="grid gap-3">
          <label class="grid gap-1 text-xs">
            <span class="text-zinc-500 dark:text-zinc-400">Local folder on this machine</span>
            <input class="input font-mono" placeholder="~/Documents" bind:value={editing.pair.local} />
          </label>
          <label class="grid gap-1 text-xs">
            <span class="text-zinc-500 dark:text-zinc-400">Folder in your account</span>
            <input class="input font-mono" placeholder="/ (everything) or /Documents" bind:value={editing.pair.remote} />
          </label>
          <label class="grid gap-1 text-xs">
            <span class="text-zinc-500 dark:text-zinc-400">Mode</span>
            <select class="input" bind:value={editing.pair.mode}>
              {#each MODES as [v, l]}<option value={v}>{l}</option>{/each}
            </select>
          </label>
          <div class="grid grid-cols-2 gap-3">
            <label class="grid gap-1 text-xs">
              <span class="text-zinc-500 dark:text-zinc-400">Run</span>
              <select class="input" bind:value={editing.pair.trigger}>
                {#each TRIGGERS as [v, l]}<option value={v}>{l}</option>{/each}
              </select>
            </label>
            <label class="grid gap-1 text-xs">
              <span class="text-zinc-500 dark:text-zinc-400">Minutes</span>
              <input class="input" type="number" min="1" max="1440" disabled={editing.pair.trigger !== "interval"} bind:value={editing.pair.interval} />
            </label>
          </div>
          <label class="grid gap-1 text-xs">
            <span class="text-zinc-500 dark:text-zinc-400">Exclude (one pattern per line)</span>
            <textarea class="input font-mono" rows="3" placeholder=".git&#10;node_modules" bind:value={editing.excludeText}></textarea>
          </label>
          {#if editing.pair.mode === "two-way" && engines.nextcloudcmd === false}
            <div class="text-xs text-amber-500">nextcloudcmd is not installed — two-way sync needs the nextcloud-client package.</div>
          {:else if editing.pair.mode !== "two-way" && engines.rclone === false}
            <div class="text-xs text-amber-500">rclone is not installed — one-way folders need it.</div>
          {/if}
          <div class="flex gap-2">
            <button class="btn-primary" disabled={!!$busy} onclick={save}>Save</button>
            <button class="btn-ghost" onclick={cancel}>Cancel</button>
          </div>
        </div>
      </div>
    {/if}

    <div class="mt-4 text-xs text-zinc-500 dark:text-zinc-400">
      Two-way folders run the Nextcloud sync engine (<code>nextcloudcmd</code>); one-way folders copy with rclone and never delete. Folder definitions live in <code>ewe.conf</code> and follow you to every machine; the local path can differ per machine.
    </div>
  {/if}
</div>
