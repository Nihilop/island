// État persisté d'un outil dans le store JSON de l'hôte (`__app__`, une clé par outil).
// Chargé une fois au démarrage, sauvegardé (débouncé) à chaque changement.

import { reactive, watch, type UnwrapNestedRefs } from "vue";
import { invoke } from "@tauri-apps/api/core";

const STORE = "__app__";

export function persisted<T extends object>(key: string, defaults: T, merge?: (saved: Partial<T>) => Partial<T>) {
  const state = reactive(structuredClone(defaults)) as UnwrapNestedRefs<T>;
  let timer: number | undefined;
  const ready = invoke<Partial<T> | null>("storage_get", { ext: STORE, key })
    .catch(() => null)
    .then((saved) => {
      if (saved && typeof saved === "object") Object.assign(state, merge ? merge(saved) : saved);
      watch(
        state,
        (s) => {
          window.clearTimeout(timer);
          timer = window.setTimeout(
            () => invoke("storage_set", { ext: STORE, key, value: JSON.parse(JSON.stringify(s)) }).catch(() => {}),
            300,
          );
        },
        { deep: true },
      );
    });
  return { state, ready };
}
