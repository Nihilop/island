// Micro-i18n PARTAGÉ (une instance dans le runtime @island/sdk → host + extensions).
// Pensé pour scaler à 200+ langues : LAZY — on ne garde en mémoire QUE la locale active
// (jamais toutes), rechargée à chaque changement de langue. Pas de dépendance (vue-i18n
// serait surdimensionné pour des strings UI). Namespacé : host = "host", chaque extension
// = son id → zéro collision de clés.
import { ref, reactive } from "vue";

export type Dict = Record<string, string>;
/** Fournit les messages d'un namespace pour UNE locale (sync ou lazy via import()). */
export type MessageLoader = (locale: string) => Dict | Promise<Dict>;

/** Locale active (réactive). Le host la pilote depuis appStore.lang (event lang://changed). */
export const locale = ref<string>("fr");

const loaders = new Map<string, MessageLoader>(); // ns → loader
const catalog = reactive<Record<string, Dict>>({}); // ns → messages de la locale ACTIVE

async function loadNs(ns: string) {
  const l = loaders.get(ns);
  if (!l) return;
  try { catalog[ns] = (await l(locale.value)) || {}; } catch { /* garde l'ancien dict */ }
}

/** Enregistre les messages d'un namespace (charge tout de suite la locale active). */
export function registerMessages(ns: string, loader: MessageLoader) {
  loaders.set(ns, loader);
  void loadNs(ns);
}

/** Change la locale active → recharge (lazy) la nouvelle locale pour TOUS les namespaces. */
export async function setLocale(l: string) {
  if (!l || l === locale.value) return;
  locale.value = l;
  await Promise.all([...loaders.keys()].map(loadNs));
}

/** Résout une clé. `{param}` interpolé depuis `params`. Réactif → re-render au switch. */
export function translate(ns: string, key: string, params?: Record<string, unknown>): string {
  void locale.value; // dépendance réactive
  const dict = catalog[ns];
  let s = (dict && dict[key]) ?? key; // fallback = la clé (visible mais non bloquant)
  if (params) for (const k in params) s = s.split("{" + k + "}").join(String(params[k]));
  return s;
}
