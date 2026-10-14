// Brouillons d'interface : l'état d'une action en cours (champ en saisie, étape d'un
// formulaire, sélection, aperçu ouvert…) survit à un changement d'onglet et à la fermeture
// du notch, parce qu'il vit dans ce store et non dans le composant (qui est démonté).
// En mémoire uniquement — rien n'est écrit sur disque.
//
//   const draft = useDraft("ask.input", () => "");   // Ref<string>, partagée et persistante
//   clearDrafts("vault.");                           // efface un groupe (ex. au verrouillage)

import { defineStore } from "pinia";
import { reactive, toRef, type Ref } from "vue";

export const useDraftsStore = defineStore("drafts", () => {
  const values = reactive<Record<string, unknown>>({});

  function clear(prefix: string) {
    for (const k of Object.keys(values)) if (k.startsWith(prefix)) delete values[k];
  }

  return { values, clear };
});

/** Ref persistante (le temps de la session) pour la clé `key`, initialisée par `init`. */
export function useDraft<T>(key: string, init: () => T): Ref<T> {
  const store = useDraftsStore();
  if (!(key in store.values)) store.values[key] = init();
  return toRef(store.values, key) as Ref<T>;
}

/** Efface les brouillons dont la clé commence par `prefix`. */
export function clearDrafts(prefix: string) {
  useDraftsStore().clear(prefix);
}
