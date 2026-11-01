<script>
  // Overview — everything the account keeps, on one page. Signed out it
  // says what signing in brings; on a new computer whose account holds a
  // backup it offers to bring everything back in one click; on the first
  // computer it offers the folders worth keeping. Above everything else sits
  // "Needs you": every open problem (a folder changed in two places, a
  // folder that couldn't sync, two computers' settings, ewe Browser failing)
  // as one alert with the action that resolves it — the same list that puts
  // a count on the nav and the state under your name (stores.js attention).
  // Then the account and what syncs on THIS computer, one row each. Nothing
  // else in ewe has a sync button (rule 6): ewe Browser and ewe Safe sync
  // themselves, this page shows them and its Sync now asks them to go.
  import { onMount } from "svelte";
  import * as api from "../api";
  import {
    route, cloud, sync, busy, folders, browser, safe, preview, avatar, attention, focusFolder, toast,
    fmtTime, fmtBytes, conflictOf, folderError, settingsError, conflictInfo, firstLine
  } from "../stores";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Row from "./ui/Row.svelte";
  import Toggle from "./ui/Toggle.svelte";
  import Check from "./ui/Check.svelte";
  import Alert from "./ui/Alert.svelte";
  import Icon from "./ui/Icon.svelte";

  let { refresh, syncNow } = $props();

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
  const welcomeShown = $derived(newMachine && !restoreHidden);

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

  // ── needs you ─────────────────────────────────────────────────────────────
  // the welcome card already asks the settings question on a new computer
  const needs = $derived($attention.filter((it) => !(it.kind === "settings" && welcomeShown)));

  function openFolder(key) {
    focusFolder.set(key);
    route.set("folders");
  }
  async function retry(key, name) {
    try {
      const r = await api.foldersRun(key);
      if (r?.ok) toast(`**${name}** synced`, "success");
    } catch (e) {
      toast(`Couldn’t sync **${name}**: ${folderError(e).reason}`, "danger", 8000);
    }
    refresh();
  }
  function settingsWho(it) {
    if (it.mine === true) return "this computer";
    return it.machine || "another computer";
  }

  // ── the rows ──────────────────────────────────────────────────────────────
  const settingsText = $derived.by(() => {
    const s = $sync;
    if (!s) return "Reading…";
    if (conflict === "remote-exists") return "Your account has settings from another computer";
    if (conflict) return "Needs a decision: two computers changed your settings";
    if (!s.enabled) return "Off on this computer";
    if (!s.local_synced_at) return "Not backed up yet";
    return `Up to date · ${fmtTime(s.local_synced_at)}`;
  });

  const browserText = $derived.by(() => {
    const b = $browser;
    const what = "Bookmarks, history and open tabs";
    if (!b) return what;
    if (!b.installed) return "ewe Browser isn’t installed. Find it in Komble.";
    if (!b.on) return `${what} · off on this computer`;
    if (b.state === "syncing") return `${what} · syncing…`;
    if (b.state === "error") return `${what} · couldn’t sync`;
    if (b.last_sync) return `${what} · up to date · ${fmtTime(b.last_sync)}`;
    return `${what} · syncs when ewe Browser runs`;
  });

  const safeText = $derived.by(() => {
    const s = $safe;
    if (!s) return "Kept by ewe Safe";
    if (!s.installed) return "ewe Safe isn’t installed. Find it in Komble.";
    if (s.answered === false) return "ewe Safe didn’t answer";
    if (!s.set_up) return "Set up ewe Safe to keep your passwords in your account";
    if (s.place === "local") return "Only on this computer. Move them to your account in ewe Safe.";
    if (s.state === "in-sync") return `Kept by ewe Safe · up to date${s.last ? ` · ${fmtTime(s.last)}` : ""}`;
    if (s.state) return `Kept by ewe Safe · ${String(s.state).replace(/-/g, " ")}`;
    return "Kept by ewe Safe";
  });

  const folderNeeds = $derived($attention.filter((it) => it.route === "folders").length);
  const foldersText = $derived.by(() => {
    const names = [...pairs.map((p) => p.name), ...appFolders.map((f) => f.name)];
    if (!names.length) return "None yet. Choose the folders to keep.";
    const list = names.length > 3 ? `${names.slice(0, 3).join(", ")} and ${names.length - 3} more` : names.join(", ");
    const conflicts = pairs.filter((p) => p.conflicts?.length).length;
    const errors = pairs.filter((p) => !p.conflicts?.length && p.state === "error").length;
    if (conflicts && !errors) return `${list} · ${conflicts} need${conflicts === 1 ? "s" : ""} a decision`;
    if (errors && !conflicts) return `${list} · ${errors} couldn’t sync`;
    if (folderNeeds) return `${list} · ${folderNeeds} need you`;
    if (pairs.some((p) => p.running)) return `${list} · syncing…`;
    if (appFolders.length && !pairs.length) return `${list} · synced by the Nextcloud app`;
    if (!$folders?.auto) return `${list} · paused`;
    return `${list} · up to date`;
  });

  async function setConf(key, on, what) {
    try {
      await api.confSet(key, on);
      toast(`**${what}** ${on ? "syncs on this computer" : "stays on this computer"}`, "info");
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
  const server = $derived(String($cloud?.server || "").replace(/^https?:\/\//, "").replace(/\/+$/, ""));
</script>

{#snippet syncAction()}
  <!-- busy keeps the accent (aria-busy, not disabled): progress, not "unavailable" -->
  <button class="ewe-btn ewe-btn--primary" aria-busy={$busy === "sync"} aria-disabled={!!$busy} onclick={syncNow}>
    {#if $busy === "sync"}<span class="ewe-spinner ewe-spinner--sm ewe-spinner--on-accent" aria-hidden="true"></span>Syncing…{:else}<Icon name="refresh" />Sync now{/if}
  </button>
{/snippet}

<Page
  title="Overview"
  desc={signedIn ? "Everything your account keeps, and whether it’s up to date." : "One account for everything that’s yours."}
  actions={signedIn && !welcomeShown ? syncAction : undefined}
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
          <li><Icon name="sliders" /><span><b>Settings & apps</b> · your look, bar, displays, keyboard and the apps you use</span></li>
          <li><Icon name="globe" /><span><b>ewe Browser</b> · bookmarks, history and the tabs open on your other computers</span></li>
          <li><Icon name="lock" /><span><b>Passwords</b> · ewe Safe keeps them encrypted in your account</span></li>
          <li><Icon name="folder" /><span><b>Folders</b> · Documents, Desktop, any folder you choose</span></li>
        </ul>
        <div class="ewe-card__foot">
          <button class="ewe-btn ewe-btn--primary" onclick={() => route.set("account")}>Sign in</button>
        </div>
      </div>
    </Group>
  {:else}
    <!-- Needs you: one alert per open problem, each with what resolves it -->
    {#if needs.length || $cloud.offline}
      <section class="needs" aria-label="Needs you">
        {#if needs.length > 1}<h2 class="ewe-section__title ewe-section__title--overline needs__title">Needs you · {needs.length}</h2>{/if}
        {#if $cloud.offline}
          <Alert tone="" icon="cloudOff" title="Offline">
            Your server didn’t answer, so this shows what was known at the last check. Everything syncs again when it
            answers.
          </Alert>
        {/if}
        {#each needs as it (it.kind + (it.key || ""))}
          {#if it.kind === "conflict"}
            {@const c = conflictInfo(it.files[0])}
            <Alert tone="warning" title={`${it.name} changed in two places`}>
              {#if it.files.length === 1}
                {c.file} was edited on this computer and somewhere else. Both copies are kept until you choose one.
              {:else}
                {it.files.length} files were edited on this computer and somewhere else. Both copies of each are kept
                until you choose.
              {/if}
              {#snippet actions()}
                <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" onclick={() => openFolder(it.key)}>
                  {it.files.length === 1 ? "Choose a copy" : "Choose copies"}
                </button>
              {/snippet}
            </Alert>
          {:else if it.kind === "error"}
            {@const e = folderError(it.raw)}
            <Alert tone="danger" title={`Couldn’t sync ${it.name}: ${e.reason}`}>
              {e.hint}
              {#snippet actions()}
                <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" onclick={() => retry(it.key, it.name)}>Try again</button>
                <button class="ewe-btn ewe-btn--sm ewe-btn--ghost" onclick={() => openFolder(it.key)}>See details</button>
              {/snippet}
            </Alert>
          {:else if it.kind === "settings"}
            <Alert
              tone="warning"
              title={it.conflict === "remote-exists" ? "Your account already has settings" : "Two computers changed your settings"}
            >
              {#if it.conflict === "remote-exists"}
                Saved by {settingsWho(it)}{it.when ? ` ${fmtTime(it.when)}` : ""}. Choose which settings this computer
                keeps; nothing syncs until you do.
              {:else}
                {settingsWho(it) === "this computer" ? "This computer" : settingsWho(it)} saved newer settings{it.when
                  ? ` ${fmtTime(it.when)}`
                  : ""}. Choose which ones to keep; nothing syncs until you do.
              {/if}
              {#snippet actions()}
                <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" onclick={() => route.set("machine")}>Choose settings</button>
              {/snippet}
            </Alert>
          {:else if it.kind === "settings-error"}
            {@const e = settingsError(it.raw)}
            <Alert tone="danger" title={`Your settings didn’t sync: ${e.reason}`}>
              {e.hint}
              {#snippet actions()}
                <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" aria-disabled={!!$busy} onclick={syncNow}>Try again</button>
                <button class="ewe-btn ewe-btn--sm ewe-btn--ghost" onclick={() => route.set("machine")}>See details</button>
              {/snippet}
            </Alert>
          {:else if it.kind === "browser"}
            <Alert tone="danger" title="ewe Browser couldn’t sync">
              {firstLine(it.raw) || "The last try failed."} Its bookmarks, history and tabs stay on this computer until
              it syncs.
              {#snippet actions()}
                <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" aria-disabled={!!$busy} onclick={syncNow}>Try again</button>
              {/snippet}
            </Alert>
          {/if}
        {/each}
      </section>
    {/if}

    {#if welcomeShown}
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
                <li><Icon name="check" /><span><b>ewe Browser</b> · bookmarks, history and open tabs</span></li>
              {/if}
              {#if $preview.safe}
                <li><Icon name="check" /><span><b>Passwords</b> · in ewe Safe, after you unlock it</span></li>
              {/if}
            </ul>
          {:else}
            <div class="waiting"><span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>Reading what’s in your account…</div>
          {/if}
          <div class="ewe-card__foot">
            <button class="ewe-btn ewe-btn--ghost" disabled={!!$busy} onclick={notNow}>Not now</button>
            <button class="ewe-btn ewe-btn--primary" aria-busy={$busy === "restore"} aria-disabled={!!$busy} onclick={bringBack}>
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
            <button class="ewe-btn ewe-btn--primary" aria-busy={$busy === "folders"} aria-disabled={!!$busy} onclick={startSyncing}>
              {#if $busy === "folders"}<span class="ewe-spinner ewe-spinner--sm ewe-spinner--on-accent" aria-hidden="true"></span>Starting…{:else}Start syncing{/if}
            </button>
          </div>
        </div>
      </Group>
    {/if}

    <!-- the account: who, where, how much room -->
    <div class="ewe-list">
      <div class="ewe-row ewe-row--tall">
        {#if $avatar}
          <img class="ewe-avatar acct-avatar avatar-img" src={$avatar} alt="" />
        {:else}
          <span class="ewe-avatar acct-avatar" aria-hidden="true">{($cloud.display_name || $cloud.user || "?").slice(0, 1).toUpperCase()}</span>
        {/if}
        <span class="ewe-row__text">
          <span class="ewe-row__title acct-name">{$cloud.display_name || $cloud.user}</span>
          <span class="ewe-row__desc">{server} · {$cloud.offline ? "offline" : "signed in"}</span>
        </span>
        {#if $cloud.quota && $cloud.quota.total > 0}
          <span class="quota-mini">
            <span class="quota-mini__text"><b>{fmtBytes($cloud.quota.total - $cloud.quota.used)} free</b> of {fmtBytes($cloud.quota.total)}</span>
            <span
              class="ewe-meter__track quota-mini__track {quotaPct >= 95 ? 'is-danger' : quotaPct >= 80 ? 'is-warning' : ''}"
              role="meter"
              aria-label="Storage used"
              aria-valuemin="0"
              aria-valuemax="100"
              aria-valuenow={quotaPct}
            >
              <span class="ewe-meter__fill" style="--value: {quotaPct}%"></span>
            </span>
          </span>
        {/if}
      </div>
    </div>

    <Group title="On this computer">
      <Row icon="sliders" title="Settings & apps" sub={settingsText}>
        <Toggle label="Keep settings and apps in step" on={!!$sync?.enabled} disabled={!$sync} toggled={(v) => setConf("sync.enabled", v, "Settings & apps")} />
      </Row>
      <div class="ewe-list__divider"></div>
      <Row icon="globe" title="ewe Browser" sub={browserText}>
        <Toggle
          label="Keep ewe Browser in step"
          on={$browser?.on !== false}
          disabled={!$browser?.installed}
          toggled={(v) => setConf("sync.browser", v, "ewe Browser")}
        />
      </Row>
      <div class="ewe-list__divider"></div>
      {#if $safe?.installed}
        <button class="ewe-row ewe-row--interactive row-button" onclick={() => api.openSafe().catch((e) => toast(String(e), "error"))}>
          <span class="ewe-row__lead ewe-row__lead--tile"><Icon name="lock" /></span>
          <span class="ewe-row__text"><span class="ewe-row__title">Passwords</span><span class="ewe-row__desc">{safeText}</span></span>
          <span class="ewe-row__trail">Open ewe Safe<Icon name="caretRight" /></span>
        </button>
      {:else}
        <Row icon="lock" title="Passwords" sub={safeText} />
      {/if}
      <div class="ewe-list__divider"></div>
      <button class="ewe-row ewe-row--interactive row-button" onclick={() => route.set("folders")}>
        <span class="ewe-row__lead ewe-row__lead--tile"><Icon name="folder" /></span>
        <span class="ewe-row__text"><span class="ewe-row__title">Folders</span><span class="ewe-row__desc">{foldersText}</span></span>
        <span class="ewe-row__trail">
          {#if folderNeeds}
            <span class="ewe-badge ewe-badge--{$attention.some((i) => i.route === 'folders' && i.kind === 'error') ? 'danger' : 'warning'}"
              ><span class="ewe-badge__label">{folderNeeds}</span></span
            >
          {:else if !pairs.length && !appFolders.length}
            Choose folders
          {/if}
          <Icon name="caretRight" />
        </span>
      </button>
    </Group>

    <p class="note">Every computer you sign in to gets the same things. Each one can turn any of them off for itself.</p>
  {/if}
</Page>
