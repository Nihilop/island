// « Nouveautés » : changelog embarqué + version vue persistée. Toutes les versions entre la
// dernière vue et la courante (sautées incluses) sont « non vues » → badge sur l'engrenage.
// Install neuve = rien de nouveau ; mise à jour depuis une version sans suivi (v1) = notes
// de la version courante.

import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import entries from "../changelog.json";

export interface ChangelogEntry {
  version: string;
  date: string;
  highlight?: boolean;
  items: Record<string, string[]>;
}

export const changelog = entries as ChangelogEntry[];
export const currentVersion = ref("");
const seen = ref<string | null>(null);
/** Versions non vues au moment où la liste a été ouverte (étiquette « Nouveau »). */
export const freshVersions = ref<Set<string>>(new Set());

function cmp(a: string, b: string): number {
  const pa = a.split(/[.-]/).map((x) => (isNaN(+x) ? x : +x));
  const pb = b.split(/[.-]/).map((x) => (isNaN(+x) ? x : +x));
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const x = pa[i] ?? 0;
    const y = pb[i] ?? 0;
    if (x === y) continue;
    if (typeof x === "number" && typeof y === "number") return x - y;
    return String(x) < String(y) ? -1 : 1;
  }
  return 0;
}

export const unseen = computed(() =>
  seen.value === null ? [] : changelog.filter((e) => cmp(e.version, seen.value!) > 0 && cmp(e.version, currentVersion.value) <= 0),
);

export async function initChangelog() {
  currentVersion.value = await getVersion().catch(() => "");
  const stored = await invoke<string | null>("storage_get", { ext: "__app__", key: "changelogSeen" }).catch(() => null);
  if (stored) {
    seen.value = stored;
    return;
  }
  // Pas de suivi : utilisateur v1 (store déjà peuplé) ou install neuve.
  const keys = await invoke<string[]>("storage_keys", { ext: "__app__" }).catch(() => [] as string[]);
  const existingUser = keys.some((k) => k !== "notch" && k !== "display");
  if (existingUser) seen.value = "0";
  else markSeen();
}

export function markSeen() {
  freshVersions.value = new Set(unseen.value.map((e) => e.version));
  seen.value = currentVersion.value;
  invoke("storage_set", { ext: "__app__", key: "changelogSeen", value: currentVersion.value }).catch(() => {});
}
