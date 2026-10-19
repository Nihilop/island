// Liste paginée : chargement (vue entière la première fois), erreur, rechargement, et une
// action par ligne avec son propre indicateur (`busy` = clé de la ligne en cours).
import { ref, watch, type Ref } from "vue";
import { api, message, type Page } from "./api";

export const PAGE_SIZE = 30;

export function usePaged<T>(path: () => string) {
  const data = ref<Page<T> | null>(null) as Ref<Page<T> | null>;
  const page = ref(0);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const busy = ref<string | null>(null);

  async function load() {
    loading.value = true;
    error.value = null;
    try {
      const sep = path().includes("?") ? "&" : "?";
      data.value = await api<Page<T>>("GET", `${path()}${sep}page=${page.value}`);
    } catch (e) {
      error.value = message(e);
    } finally {
      loading.value = false;
    }
  }

  /** Action sur une ligne puis rechargement ; une seule à la fois. */
  async function act(key: string, fn: () => Promise<unknown>) {
    if (busy.value) return;
    busy.value = key;
    error.value = null;
    try {
      await fn();
      await load();
    } catch (e) {
      error.value = message(e);
    } finally {
      busy.value = null;
    }
  }

  watch(path, () => {
    page.value = 0;
    void load();
  });
  watch(page, () => void load());
  void load();
  return { data, page, loading, error, busy, load, act };
}
