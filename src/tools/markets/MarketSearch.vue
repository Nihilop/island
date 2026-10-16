<script setup lang="ts">
// Ajout d'une valeur : champ de recherche (debounce 300 ms, 2 caractères min) et résultats
// cliquables de l'onglet courant (Yahoo pour la bourse, CoinGecko pour la crypto). La saisie
// et les résultats sont des brouillons : ils survivent à la fermeture du notch.
import { ref, watch } from "vue";
import { PhCheck, PhPlus } from "@phosphor-icons/vue";
import TextField from "../../ui/TextField.vue";
import { Spinner } from "@/sdk/ui/spinner";
import { Item, ItemActions, ItemContent, ItemDescription, ItemMedia, ItemTitle } from "@/sdk/ui/item";
import { useDraft } from "../../stores/drafts";
import { add, search, watchlist, type SearchHit } from "./store";
import { t } from "../../notch/i18n";

const query = useDraft("markets.query", () => "");
const results = useDraft<SearchHit[]>("markets.results", () => []);
const searching = ref(false);
const failed = ref(false);
const adding = ref<string | null>(null);
let timer: number | undefined;
let seq = 0;

watch(
  [query, () => watchlist.tab],
  ([q], old) => {
    window.clearTimeout(timer);
    const tabChanged = old && old[1] !== watchlist.tab;
    if (q.trim().length < 2) {
      results.value = [];
      searching.value = false;
      return;
    }
    if (tabChanged) results.value = [];
    searching.value = true;
    failed.value = false;
    const tab = watchlist.tab;
    const id = ++seq;
    timer = window.setTimeout(async () => {
      const r = await search(tab, q.trim()).catch(() => null);
      if (id !== seq) return; // réponse périmée (saisie ou onglet changé entre-temps)
      results.value = r ?? [];
      failed.value = r === null;
      searching.value = false;
    }, 300);
  },
);

async function pick(h: SearchHit) {
  if (adding.value || watchlist[watchlist.tab].includes(h.id)) return;
  adding.value = h.id;
  try {
    await add(watchlist.tab, h.id);
    query.value = "";
  } finally {
    adding.value = null;
  }
}
</script>

<template>
  <div class="flex flex-col gap-1.5">
    <div class="relative">
      <TextField v-model="query" :placeholder="t(watchlist.tab === 'stocks' ? 'markets.searchStocks' : 'markets.searchCrypto')" />
      <Spinner v-if="searching" class="absolute top-1/2 right-2.5 -translate-y-1/2 text-label-3" />
    </div>
    <div v-if="query.trim().length >= 2 && !searching" class="flex flex-col">
      <div v-if="failed" class="type-caption px-2 py-3 text-center text-label-3">{{ t("markets.searchFailed") }}</div>
      <div v-else-if="!results.length" class="type-caption px-2 py-3 text-center text-label-3">{{ t("markets.noResults") }}</div>
      <Item
        v-for="h in results"
        :key="h.id"
        as="button"
        type="button"
        size="sm"
        class="text-left"
        :disabled="!!adding"
        @click="pick(h)"
      >
        <ItemMedia v-if="h.image" variant="image" class="size-5 rounded-full"><img :src="h.image" alt="" /></ItemMedia>
        <ItemContent class="flex-row items-baseline gap-2">
          <ItemTitle class="shrink-0">{{ h.symbol }}</ItemTitle>
          <ItemDescription class="truncate">{{ h.name }}{{ h.detail ? ` · ${h.detail}` : "" }}</ItemDescription>
        </ItemContent>
        <ItemActions class="text-label-3">
          <Spinner v-if="adding === h.id" />
          <PhCheck v-else-if="watchlist[watchlist.tab].includes(h.id)" :size="13" weight="bold" class="text-sys-green" />
          <PhPlus v-else :size="13" weight="bold" />
        </ItemActions>
      </Item>
    </div>
  </div>
</template>
