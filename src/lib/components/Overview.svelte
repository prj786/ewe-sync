<script>
  // Overview — everything the account keeps, on one page (0.15). Signed out
  // it says what signing in brings; on a new computer whose account holds a
  // backup it offers to bring everything back in one click; on the first
  // computer it offers the folders worth keeping; and always it lists what
  // syncs on THIS computer — settings & apps, the browser, passwords,
  // folders — each with its own switch and state. Nothing else in ewe has a
  // sync button (rule 6): the browser and Safe sync themselves, this page
  // shows them and its Sync now asks them to go.
  import { onMount } from "svelte";
  import * as api from "../api";
  import {
    route, cloud, sync, busy, folders, browser, safe, preview, toast, fmtTime, fmtBytes, conflictOf, tildePath
  } from "../stores";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Row from "./ui/Row.svelte";
  import Toggle from "./ui/Toggle.svelte";
  import Check from "./ui/Check.svelte";
  import Icon from "./ui/Icon.svelte";

  let { refresh, syncNow } = $props();

  let avatar = $state("");
  let suggestions = $state([]); // [{ name, local, remote, in_account, remote_size, exclude, size?, picked }]
  let restoreHidden = $state(false);
  let startHidden = $state(false);

  const signedIn = $derived(!!$cloud?.signed_in);
  const pairs = $derived($folders?.pairs || []);
  const appFolders = $derived($folders?.app?.folders || []);
  const conflict = $derived(conflictOf($sync));
  // a computer that never synced, and the account holds a backup: the
  // "welcome back" moment
  const newMachine = $derived(signedIn && $sync && !$sync.local_synced_at && conflict === "remote-exists");
  const firstMachine = $derived(
    signedIn && $sync && !$sync.remote && !pairs.length && !appFolders.length
  );

  function dismissedKey(name) {
    return `ewe-sync.${name}Dismissed`;
  }
  onMount(() => {
    startHidden = localStorage.getItem(dismissedKey("start")) === "1";
  });

  // the restore preview, once per backup ("Not now" remembers which one)
  $effect(() => {
    if (!newMachine) return;
    const stamp = $sync?.remote_modified || "";
    restoreHidden = localStorage.getItem(dismissedKey("restore")) === stamp && stamp !== "";
    if ($preview?.modified === stamp) return;
    api.restorePreview().then((p) => preview.set(p)).catch(() => preview.set(null));
  });

  $effect(() => {
    if (!firstMachine || startHidden || suggestions.length) return;
    api.foldersSuggest()
      .then((r) => {
        // Documents and Desktop are what people mean by "my files"
        suggestions = (r.items || []).map((s) => ({ ...s, picked: s.kind === "documents" || s.kind === "desktop", size: null }));
        for (const s of suggestions) {
          api.folderSize(s.local).then((z) => {
            suggestions = suggestions.map((x) => (x.local === s.local ? { ...x, size: z } : x));
          }).catch(() => {});
        }
      })
      .catch(() => {});
  });

  $effect(() => {
    if (signedIn && !avatar) {
      api.cloudAvatar().then((a) => {
        if (a?.ok && a.path) avatar = api.fileSrc(a.path);
      }).catch(() => {});
    }
  });

  // ── the rows ──────────────────────────────────────────────────────────────
  const settingsText = $derived.by(() => {
    const s = $sync;
    if (!s) return "Reading…";
    if (conflict === "remote-exists") return "Your account has a backup from another computer";
    if (conflict) return "Needs a decision: two computers changed settings";
    if (!s.enabled) return "Off on this computer";
    if (!s.local_synced_at) return "Not backed up yet";
    return `Up to date · ${fmtTime(s.local_synced_at)}`;
  });

  const browserText = $derived.by(() => {
    const b = $browser;
    if (!b) return "Bookmarks, history and open tabs";
    if (!b.installed) return "ewe browser isn’t installed";
    if (!b.on) return "Off on this computer";
    if (b.state === "syncing") return "Syncing…";
    if (b.state === "error") return b.error || "Couldn’t sync";
    if (b.last_sync) return `Bookmarks, history and open tabs · ${fmtTime(Math.round(b.last_sync / 1000))}`;
    return "Bookmarks, history and open tabs · syncs when the browser runs";
  });

  const safeText = $derived.by(() => {
    const s = $safe;
    if (!s) return "Your passwords, in Safe";
    if (!s.installed) return "Safe isn’t installed. Find it in Komble.";
    if (s.answered === false) return "Safe didn’t answer";
    if (!s.set_up) return "Set up Safe to keep your passwords in your account";
    if (s.place === "local") return "Only on this computer. Move them to your account in Safe.";
    if (s.state === "in-sync") return `In your account · ${fmtTime(s.last)}`;
    if (s.state) return `In your account · ${String(s.state).replace(/-/g, " ")}`;
    return "In your account";
  });

  const foldersText = $derived.by(() => {
    const n = pairs.length + appFolders.length;
    if (!n) return "None yet. Choose the folders to keep.";
    const bad = pairs.filter((p) => p.state === "error" || p.state === "conflict").length;
    const word = `${n} folder${n === 1 ? "" : "s"}`;
    if (pairs.some((p) => p.running)) return `${word} · syncing…`;
    if (bad) return `${word} · ${bad} need${bad === 1 ? "s" : ""} attention`;
    if (appFolders.length && !pairs.length) return `${word} · synced by the Nextcloud app`;
    if (!$folders?.auto) return `${word} · paused`;
    return `${word} · up to date`;
  });

  async function setConf(key, on, what) {
    try {
      await api.confSet(key, on);
      toast(`${what} ${on ? "syncs on this computer" : "stays on this computer"}`, "info");
    } catch (e) {
      toast(String(e), "error");
    }
    refresh();
  }

  // ── bring everything back ─────────────────────────────────────────────────
  const restoreBytes = $derived(($preview?.folders || []).reduce((a, f) => a + (Number(f.size) || 0), 0));
  async function bringBack() {
    if ($busy) return;
    busy.set("restore");
    try {
      const r = await api.restoreEverything();
      if (r.ok) {
        const bits = [];
        if (r.apps) bits.push(`${r.apps} app${r.apps === 1 ? "" : "s"} wait in Komble`);
        if (r.folders) bits.push(`your folders are coming down`);
        toast(`Welcome back.${bits.length ? " " + bits.join(", and ") + "." : ""}`, "success", 9000);
      } else toast(r.message || r.error || r.apply_stderr || "Couldn’t bring everything back. Try again.", "error");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy.set("");
      preview.set(null);
      refresh();
    }
  }
  function notNow() {
    localStorage.setItem(dismissedKey("restore"), $sync?.remote_modified || "");
    restoreHidden = true;
  }

  // ── the first computer: choose what to keep ───────────────────────────────
  const picked = $derived(suggestions.filter((s) => s.picked));
  // what the picked folders add to the account (a folder already there
  // counts only what is new), against the free space
  const pickedBytes = $derived(
    picked.reduce((a, s) => a + Math.max(0, (Number(s.size?.bytes) || 0) - (Number(s.remote_size) || 0)), 0)
  );
  const freeBytes = $derived(
    $cloud?.quota?.total > 0 ? Math.max(0, $cloud.quota.total - $cloud.quota.used) : null
  );
  async function startSyncing() {
    if ($busy) return;
    busy.set("folders");
    try {
      const add = picked.map((s) => ({
        remote: s.remote, local: s.local, mode: "two-way", trigger: "change", interval: 10, exclude: s.exclude || []
      }));
      if (add.length) folders.set(await api.foldersSet([...pairs.map((p) => p.pair), ...add]));
      await api.syncEverything();
      toast(add.length ? `${add.map((a) => a.remote).join(", ")} ${add.length === 1 ? "is" : "are"} now in your account` : "Your settings are backed up", "success");
      localStorage.setItem(dismissedKey("start"), "1");
      startHidden = true;
    } catch (e) {
      toast(String(e), "error", 8000);
    } finally {
      busy.set("");
      refresh();
    }
  }
  function skipStart() {
    localStorage.setItem(dismissedKey("start"), "1");
    startHidden = true;
  }

  const quotaPct = $derived(Math.max(0, Math.min(100, Math.round($cloud?.quota?.relative ?? 0))));
</script>

{#snippet syncAction()}
  <button class="ewe-btn ewe-btn--primary" disabled={!!$busy} onclick={syncNow}>
    {#if $busy === "sync"}<span class="ewe-spinner ewe-spinner--sm ewe-spinner--on-accent" aria-hidden="true"></span>Syncing…{:else}<Icon name="refresh" />Sync now{/if}
  </button>
{/snippet}

<Page
  title="Overview"
  desc={signedIn ? "Everything your account keeps, and whether it’s up to date." : "One account for everything that’s yours."}
  actions={signedIn && !newMachine ? syncAction : undefined}
>
  {#if !signedIn}
    <Group well={false}>
      <div class="ewe-card promise">
        <div class="ewe-card__head">
          <span class="ewe-card__icon ewe-card__icon--accent"><Icon name="cloud" /></span>
          <div class="ewe-card__titles">
            <div class="ewe-card__title">Sign in once. Everything follows.</div>
            <div class="ewe-card__desc">
              Your settings, your apps, your browser, your passwords and your folders live in your own Nextcloud.
              Sign in on any computer and they come back.
            </div>
          </div>
        </div>
        <ul class="promise__list">
          <li><Icon name="settings" /><span><b>Settings & apps</b> · your look, bar, displays, keyboard and the apps you use</span></li>
          <li><Icon name="globe" /><span><b>Browser</b> · bookmarks, history and the tabs open on your other computers</span></li>
          <li><Icon name="lock" /><span><b>Passwords</b> · Safe keeps them encrypted in your account</span></li>
          <li><Icon name="folderSync" /><span><b>Folders</b> · Documents, Desktop, any folder you choose</span></li>
        </ul>
        <div class="ewe-card__foot">
          <button class="ewe-btn ewe-btn--primary" onclick={() => route.set("account")}>Sign in</button>
        </div>
      </div>
    </Group>
  {:else}
    {#if newMachine && !restoreHidden}
      <Group well={false}>
        <div class="ewe-card welcome" aria-busy={$busy === "restore"}>
          <div class="ewe-card__head">
            <span class="ewe-card__icon ewe-card__icon--accent"><Icon name="cloudDownload" /></span>
            <div class="ewe-card__titles">
              <div class="ewe-card__title">Welcome back</div>
              <div class="ewe-card__desc">
                {#if $preview?.exists}
                  Your account has everything from “{$preview.machine || "your other computer"}”, saved {fmtTime($preview.modified)}.
                  Bring it to this computer?
                {:else}
                  Your account has a backup from another computer. Bring it to this computer?
                {/if}
              </div>
            </div>
          </div>
          {#if $preview?.exists}
            <ul class="promise__list">
              <li><Icon name="check" /><span><b>Settings and look</b> · theme, bar, displays, keyboard, shortcuts</span></li>
              {#if $preview.apps}
                <li><Icon name="check" /><span><b>{$preview.apps} app{$preview.apps === 1 ? "" : "s"}</b> · waiting in Komble; nothing installs by itself</span></li>
              {/if}
              {#if $preview.folders?.length}
                <li>
                  <Icon name="check" />
                  <span
                    ><b>{$preview.folders.length} folder{$preview.folders.length === 1 ? "" : "s"}</b> ·
                    {$preview.folders.map((f) => f.name).join(", ")}{restoreBytes ? ` · ${fmtBytes(restoreBytes)} to download` : ""}</span
                  >
                </li>
              {/if}
              {#if $preview.browser}
                <li><Icon name="check" /><span><b>Browser</b> · bookmarks, history and open tabs</span></li>
              {/if}
              {#if $preview.safe}
                <li><Icon name="check" /><span><b>Passwords</b> · in Safe, after you unlock it</span></li>
              {/if}
            </ul>
          {:else}
            <div class="waiting"><span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>Reading what’s in your account…</div>
          {/if}
          <div class="ewe-card__foot">
            <button class="ewe-btn ewe-btn--ghost" disabled={!!$busy} onclick={notNow}>Not now</button>
            <button class="ewe-btn ewe-btn--primary" disabled={!!$busy} onclick={bringBack}>
              {#if $busy === "restore"}<span class="ewe-spinner ewe-spinner--sm ewe-spinner--on-accent" aria-hidden="true"></span>Bringing it back…{:else}Bring everything back{/if}
            </button>
          </div>
        </div>
      </Group>
    {:else if firstMachine && !startHidden}
      <Group well={false}>
        <div class="ewe-card welcome">
          <div class="ewe-card__head">
            <span class="ewe-card__icon ewe-card__icon--accent"><Icon name="sparkles" /></span>
            <div class="ewe-card__titles">
              <div class="ewe-card__title">Keep your files in your account</div>
              <div class="ewe-card__desc">
                Pick the folders to keep. They stay right where they are on this computer, and appear on every computer
                you sign in to. Your settings are backed up too.
              </div>
            </div>
          </div>
          {#if suggestions.length}
            <div class="pick-list">
              {#each suggestions as s (s.local)}
                <Check
                  checked={s.picked}
                  label={s.name}
                  desc={`${s.local}${s.size ? ` · ${s.size.more ? "more than " : ""}${fmtBytes(s.size.bytes)}` : ""}${s.in_account ? " · already in your account" : ""}`}
                  toggled={(v) => (suggestions = suggestions.map((x) => (x.local === s.local ? { ...x, picked: v } : x)))}
                />
              {/each}
            </div>
            {#if freeBytes != null && pickedBytes > freeBytes}
              <p class="ewe-field__helper ewe-field__helper--error">
                <Icon name="alert" />That’s about {fmtBytes(pickedBytes)}, and your account has {fmtBytes(freeBytes)} free.
                Pick fewer folders, or make room in your account first.
              </p>
            {/if}
          {/if}
          <div class="ewe-card__foot">
            <button class="ewe-btn ewe-btn--ghost" disabled={!!$busy} onclick={skipStart}>Not now</button>
            <button class="ewe-btn ewe-btn--primary" disabled={!!$busy} onclick={startSyncing}>
              {#if $busy === "folders"}<span class="ewe-spinner ewe-spinner--sm ewe-spinner--on-accent" aria-hidden="true"></span>Starting…{:else}Start syncing{/if}
            </button>
          </div>
        </div>
      </Group>
    {/if}

    <Group title="Your account">
      <Row tall title={$cloud.display_name || $cloud.user} sub={`${$cloud.email || $cloud.user} · ${String($cloud.server || "").replace(/^https?:\/\//, "")}`}>
        {#snippet lead()}
          {#if avatar}
            <img class="ewe-avatar ewe-avatar--lg avatar-img" src={avatar} alt="" />
          {:else}
            <span class="ewe-avatar ewe-avatar--lg" aria-hidden="true">
              {($cloud.display_name || $cloud.user || "?").slice(0, 1).toUpperCase()}
            </span>
          {/if}
        {/snippet}
        {#if $cloud.quota && $cloud.quota.total > 0}
          <span class="quota-mini" title={`${fmtBytes($cloud.quota.used)} of ${fmtBytes($cloud.quota.total)} used`}>
            <span class="ewe-meter__track quota-mini__track {quotaPct >= 95 ? 'is-danger' : quotaPct >= 80 ? 'is-warning' : ''}" role="meter" aria-label="Storage used" aria-valuemin="0" aria-valuemax="100" aria-valuenow={quotaPct}>
              <span class="ewe-meter__fill" style="--value: {quotaPct}%"></span>
            </span>
            <span class="quota-mini__text">{fmtBytes($cloud.quota.total - $cloud.quota.used)} free</span>
          </span>
        {/if}
      </Row>
      {#if $cloud.offline}
        <Row dense sub="Your server didn’t answer. Everything syncs again when it does." />
      {/if}
    </Group>

    <Group title="On this computer">
      <Row icon="settings" title="Settings & apps" sub={settingsText}>
        {#if conflict && conflict !== "remote-exists"}
          <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" onclick={() => route.set("machine")}>Decide</button>
        {/if}
        <Toggle label="Sync settings and apps" on={!!$sync?.enabled} disabled={!$sync} toggled={(v) => setConf("sync.enabled", v, "Settings & apps")} />
      </Row>
      <div class="ewe-list__divider"></div>
      <Row icon="globe" title="Browser" sub={browserText}>
        <Toggle
          label="Sync the browser"
          on={$browser?.on !== false}
          disabled={!$browser?.installed}
          toggled={(v) => setConf("sync.browser", v, "The browser")}
        />
      </Row>
      <div class="ewe-list__divider"></div>
      <Row icon="lock" title="Passwords" sub={safeText}>
        {#if $safe?.installed}
          <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" onclick={() => api.openSafe().catch((e) => toast(String(e), "error"))}>Open Safe</button>
        {/if}
      </Row>
      <div class="ewe-list__divider"></div>
      <Row icon="folderSync" title="Folders" sub={foldersText}>
        <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" onclick={() => route.set("folders")}>
          {pairs.length + appFolders.length ? "Manage" : "Choose folders"}
        </button>
      </Row>
    </Group>

    {#if pairs.length}
      <Group title="Folders">
        {#each pairs as it, i (it.key)}
          {#if i > 0}<div class="ewe-list__divider"></div>{/if}
          <Row dense title={it.name} sub={tildePath(it.local_effective)}>
            {#snippet lead()}
              <span class="ewe-row__lead ewe-row__lead--tile"><Icon name="folder" /></span>
            {/snippet}
            <span class="ewe-badge {it.state === 'synced' ? 'ewe-badge--success' : it.state === 'syncing' ? 'ewe-badge--accent' : it.state === 'error' ? 'ewe-badge--danger' : it.state === 'conflict' ? 'ewe-badge--warning' : ''}">
              <span class="ewe-badge__label">{{ synced: "Up to date", syncing: "Syncing…", paused: "Paused", error: "Couldn’t sync", conflict: "Needs a decision", offline: "Offline", pending: "Waiting" }[it.state] || it.state}</span>
            </span>
          </Row>
        {/each}
      </Group>
    {/if}

    <p class="note">
      Every computer you sign in to gets the same folders, settings and browser. Each one can switch any of them off
      for itself here.
    </p>
  {/if}
</Page>
