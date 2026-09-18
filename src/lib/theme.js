import { writable } from "svelte/store";
import * as api from "./api.js";

/**
 * The live look (the same runtime as ewe-settings' lib/theme.js). tokens.css
 * is compiled in (Ewe Dark, the default accent) so the app paints something
 * before this runs and in a browser without Tauri. `ewe-theme show` builds
 * the token map from THIS machine's ewe.conf — the active scheme, accent,
 * look presets, accessibility modes — and every value lands as an inline
 * custom property on <html>, which beats the compiled file. Nothing here
 * knows which scheme is active: the generator already did the derivation.
 */
export const theme = writable(null);

let seq = 0;
export async function refreshTheme() {
  const mine = ++seq;
  let t;
  try {
    t = await api.themeTokens("ewe");
  } catch {
    return null; // no ewe-theme (dev browser, or ewe not deployed): keep tokens.css
  }
  if (mine !== seq || !t || !t.css_vars) return null;
  const root = document.documentElement;
  for (const [k, v] of Object.entries(t.css_vars)) root.style.setProperty(k, v);

  // Motion (README "Motion"): the four durations divided by the animation
  // speed; Off makes them 0. Reduce motion is already folded in by the
  // generator (base and slow become fast).
  const m = t.motion || {};
  const speed = Number(m.speed ?? 1);
  const dur = (ms) => (speed > 0 ? Math.round(Number(ms) / speed) : 0) + "ms";
  if (m.durFast != null) {
    root.style.setProperty("--dur-fast", dur(m.durFast));
    root.style.setProperty("--dur-base", dur(m.durBase));
    root.style.setProperty("--dur-slow", dur(m.durSlow));
    root.style.setProperty("--dur-dim", dur(m.durDim));
  }

  const variant = (t.input && t.input.variant) || (t.scheme && t.scheme.variant) || "dark";
  root.style.colorScheme = variant === "light" ? "light" : "dark";
  root.classList.toggle("dark", variant !== "light");
  root.dataset.variant = variant;
  theme.set(t);
  return t;
}

/**
 * Follow the desktop: at start, whenever the window regains focus, and — when
 * `pollKey` is given — whenever the cheap key it returns changes (checked
 * every `every` ms). ewe-sync polls the DE's user-theme.json this way, so a
 * scheme, accent, preset or accessibility change made in Settings reaches an
 * open ewe-sync window without a click into it.
 */
export function watchTheme(pollKey = null, every = 4000) {
  refreshTheme();
  const onFocus = () => refreshTheme();
  window.addEventListener("focus", onFocus);
  let timer = null;
  if (pollKey) {
    let last = null;
    const tick = async () => {
      let k;
      try {
        k = String(await pollKey());
      } catch {
        return;
      }
      if (last !== null && k !== last) refreshTheme();
      last = k;
    };
    tick();
    timer = setInterval(tick, every);
  }
  return () => {
    window.removeEventListener("focus", onFocus);
    if (timer) clearInterval(timer);
  };
}
