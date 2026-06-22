// Registre des entrées de launcher.
// Une extension enregistre son entrée dans `activate` (via le SDK) → elle
// n'apparaît dans la grille QUE si elle est activée. Désactivée = pas chargée
// = pas enregistrée = absente du launcher.
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";

export interface LauncherEntry {
  id: string;
  label: string;
  icon: string; // SVG (string)
  onActivate: () => void;
}

/** Un résultat dynamique fourni par un provider quand l'utilisateur tape. */
export interface LauncherResult {
  id: string;
  title: string;
  subtitle?: string;
  icon?: string; // SVG (string) ; défaut si absent
  onActivate: () => void;
}

/** Une extension peut alimenter la recherche du launcher (palette extensible). */
export interface LauncherProvider {
  onQuery: (query: string) => LauncherResult[] | Promise<LauncherResult[]>;
}

const entries = new Map<string, LauncherEntry>();
const providers = new Map<string, LauncherProvider>();
const version = ref(0);

export function setLauncherEntry(key: string, entry: LauncherEntry | null) {
  if (!entry) {
    if (entries.delete(key)) version.value++;
    return;
  }
  entries.set(key, entry);
  version.value++;
}

export function setLauncherProvider(key: string, provider: LauncherProvider | null) {
  if (!provider) {
    if (providers.delete(key)) version.value++;
    return;
  }
  providers.set(key, provider);
  version.value++;
}

export const launcherEntries = computed<LauncherEntry[]>(() => {
  void version.value;
  return [...entries.values()];
});

/** Y a-t-il au moins un provider ? (la recherche n'apparaît que si oui). */
export const hasProviders = computed<boolean>(() => {
  void version.value;
  return providers.size > 0;
});

/** Interroge tous les providers et fusionne leurs résultats (tolérant aux erreurs). */
export async function runProviders(query: string): Promise<LauncherResult[]> {
  const lists = await Promise.all(
    [...providers.values()].map((p) => Promise.resolve().then(() => p.onQuery(query)).catch(() => [] as LauncherResult[])),
  );
  return lists.flat();
}

// --- État de la VUE launcher (recherche + grille) -----------------------------------
// Partagé entre Island (qui lit `launcherCells`/`hasProviders` pour se DIMENSIONNER au
// morph, avant que Launcher.vue ne soit monté) et Launcher.vue (qui le rend).
export interface LauncherCell { id: string; label: string; icon: string; kind: string; onActivate?: () => void }

// Icônes des actions natives (le backend renvoie un NOM ; on le mappe en SVG).
const ICONS: Record<string, string> = {
  settings: "<svg viewBox='0 0 24 24'><path fill='currentColor' d='M19.14 12.94c.04-.31.06-.63.06-.94s-.02-.63-.06-.94l2.03-1.58a.48.48 0 0 0 .12-.61l-1.92-3.32a.49.49 0 0 0-.59-.22l-2.39.96a7 7 0 0 0-1.62-.94l-.36-2.54A.49.49 0 0 0 13.5 2h-3a.49.49 0 0 0-.48.42l-.36 2.54c-.59.24-1.13.56-1.62.94l-2.39-.96a.49.49 0 0 0-.59.22L2.74 8.48a.48.48 0 0 0 .12.61l2.03 1.58c-.04.31-.06.63-.06.94s.02.63.06.94l-2.03 1.58a.48.48 0 0 0-.12.61l1.92 3.32c.13.22.39.31.59.22l2.39-.96c.49.38 1.03.7 1.62.94l.36 2.54c.05.24.25.42.48.42h3c.23 0 .43-.18.48-.42l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.2.09.46 0 .59-.22l1.92-3.32a.48.48 0 0 0-.12-.61zM12 15.5A3.5 3.5 0 1 1 12 8.5a3.5 3.5 0 0 1 0 7z'/></svg>",
  moon: "<svg viewBox='0 0 24 24'><path fill='currentColor' d='M12 3a9 9 0 1 0 9 9c0-.46-.04-.92-.1-1.36a5.5 5.5 0 0 1-7.54-7.54C12.92 3.04 12.46 3 12 3z'/></svg>",
  puzzle: "<svg viewBox='0 0 24 24'><path fill='currentColor' d='M20.5 11H19V7a2 2 0 0 0-2-2h-4V3.5a2.5 2.5 0 0 0-5 0V5H4a2 2 0 0 0-2 2v3.8h1.5a2.7 2.7 0 0 1 0 5.4H2V20a2 2 0 0 0 2 2h3.8v-1.5a2.7 2.7 0 0 1 5.4 0V22H17a2 2 0 0 0 2-2v-4h1.5a2.5 2.5 0 0 0 0-5z'/></svg>",
  bell: "<svg viewBox='0 0 24 24'><path fill='currentColor' d='M12 2a6 6 0 0 0-6 6v3.4l-1.7 3A1 1 0 0 0 5.2 16h13.6a1 1 0 0 0 .9-1.6L18 11.4V8a6 6 0 0 0-6-6zM9.5 18a2.5 2.5 0 0 0 5 0z'/></svg>",
};
const RESULT_ICON =
  "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><circle cx='11' cy='11' r='7'/><path d='m21 21-4.3-4.3'/></svg>";

export const query = ref("");
const builtinActions = ref<LauncherCell[]>([]);
const providerResults = ref<LauncherCell[]>([]);

/** Charge les actions natives (Réglages, DND…) — appelé une fois au démarrage. */
export async function loadBuiltins() {
  // « Notifications » : accès au centre depuis le launcher (≠ la cloche, toujours dispo).
  const notifs: LauncherCell = { id: "notifs", label: "Notifications", icon: ICONS.bell, kind: "notifs" };
  try {
    const list = await invoke<LauncherCell[]>("list_launcher");
    builtinActions.value = [...list.map((a) => ({ ...a, icon: ICONS[a.icon] || "" })), notifs];
  } catch {
    builtinActions.value = [notifs];
  }
}

const searching = computed(() => hasProviders.value && query.value.trim().length > 0);

/** Interroge les providers et stocke les résultats (appelé débouncé par Launcher.vue). */
export async function runSearch(term: string): Promise<void> {
  if (!hasProviders.value || !term) { providerResults.value = []; return; }
  const res = await runProviders(term).catch(() => []);
  providerResults.value = res.map((r) => ({ id: r.id, label: r.title, icon: r.icon || RESULT_ICON, kind: "result", onActivate: r.onActivate }));
}

/** Vide la recherche (sortie du launcher, ou Échap). */
export function resetLauncherQuery() { query.value = ""; providerResults.value = []; }

/** Cellules affichées : recherche (providers + entrées qui matchent) sinon natifs + entrées. */
export const launcherCells = computed<LauncherCell[]>(() => {
  if (searching.value) {
    const q = query.value.trim().toLowerCase();
    const matched = launcherEntries.value
      .filter((e) => e.label.toLowerCase().includes(q))
      .map((e) => ({ id: e.id, label: e.label, icon: e.icon, kind: "entry", onActivate: e.onActivate }));
    return [...providerResults.value, ...matched];
  }
  return [
    ...builtinActions.value,
    ...launcherEntries.value.map((e) => ({ id: e.id, label: e.label, icon: e.icon, kind: "entry", onActivate: e.onActivate })),
  ];
});
