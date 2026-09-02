<script>
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { route, cloud, trayState } from "../stores";

  let version = $state("");
  onMount(async () => {
    try {
      version = await getVersion();
    } catch {
      // dev server outside Tauri
    }
  });

  // Phosphor Fill codepoints — the DE's icon language
  const items = [
    { id: "account", label: "Account", icon: 0xe4c2 }, // user
    { id: "machine", label: "This machine", icon: 0xe1fc }, // desktop
    { id: "machines", label: "Machines", icon: 0xe2b0 }, // devices
    { id: "folders", label: "Folders", icon: 0xe256 } // folder
  ];
  const dotColor = {
    idle: "bg-green-500",
    syncing: "bg-sky-400",
    conflict: "bg-amber-500",
    offline: "bg-zinc-500",
    "signed-out": "bg-zinc-500"
  };
</script>

<aside
  class="flex w-14 shrink-0 flex-col border-r border-zinc-200 bg-white/60 md:w-52 dark:border-zinc-700/60 dark:bg-zinc-800/40"
>
  <div class="flex items-center justify-center gap-2.5 px-2 pb-4 pt-5 md:justify-start md:px-4">
    <!-- the fleece mark -->
    <div
      class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg"
      style="background: linear-gradient(135deg, #8fbce0, #3d6d99)"
    >
      <svg viewBox="0 0 64 64" class="h-6 w-6" fill="#fff">
        <circle cx="24" cy="30" r="9" /><circle cx="33" cy="26" r="9.5" /><circle cx="42" cy="31" r="8.5" />
        <circle cx="28" cy="38" r="8.5" /><circle cx="38" cy="39" r="8.5" /><circle cx="48" cy="38" r="5.5" />
        <rect x="26" y="44" width="3.2" height="9" rx="1.6" /><rect x="37" y="44" width="3.2" height="9" rx="1.6" />
      </svg>
    </div>
    <div class="hidden text-lg font-semibold tracking-tight md:block">Flock</div>
  </div>

  <nav class="flex flex-col gap-0.5 px-1.5 md:px-2.5">
    {#each items as it}
      <button
        title={it.label}
        class="relative flex items-center justify-center gap-2.5 rounded-lg px-2.5 py-2 text-sm font-medium transition-colors md:justify-start
          {$route === it.id
          ? 'text-white'
          : 'text-zinc-600 hover:bg-zinc-200/60 dark:text-zinc-300 dark:hover:bg-zinc-700/50'}"
        style={$route === it.id ? "background: var(--accent)" : ""}
        onclick={() => route.set(it.id)}
      >
        <span class="ph-i w-4 shrink-0 text-center text-[16px]">{String.fromCodePoint(it.icon)}</span>
        <span class="hidden flex-1 text-left md:block">{it.label}</span>
      </button>
    {/each}
  </nav>

  <div class="mt-auto px-3 pb-4 pt-3 text-xs text-zinc-500 dark:text-zinc-400">
    <div class="hidden items-center gap-2 md:flex">
      <span class="h-2 w-2 rounded-full {dotColor[$trayState] || 'bg-zinc-500'}"></span>
      <span class="truncate">
        {#if $cloud?.signed_in}{$cloud.display_name || $cloud.user}{:else}Not signed in{/if}
      </span>
    </div>
    {#if version}<div class="mt-1 hidden md:block">Flock {version}</div>{/if}
  </div>
</aside>
