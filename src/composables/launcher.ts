// Registre des entrées de launcher.
// Une extension enregistre son entrée dans `activate` (via le SDK) → elle
// n'apparaît dans la grille QUE si elle est activée. Désactivée = pas chargée
// = pas enregistrée = absente du launcher.
import { ref, computed } from "vue";

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
