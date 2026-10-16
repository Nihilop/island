<script setup lang="ts">
// Outil « Marchés » (560 × 380) : liste de suivi bourse (Yahoo Finance) / crypto
// (CoinGecko), cours + variation du jour (24 h en crypto) + mini-graphique. Onglets et
// rafraîchissement dans l'en-tête ; rafraîchi toutes les 60 s tant que l'outil est ouvert.
import { computed, onBeforeUnmount, onMounted, watch } from "vue";
import { PhArrowClockwise, PhChartLineUp, PhWarningCircle, PhX } from "@phosphor-icons/vue";
import Segmented from "../../ui/Segmented.vue";
import IconButton from "../../ui/IconButton.vue";
import Empty from "../../ui/Empty.vue";
import Sparkline from "../../ui/Sparkline.vue";
import MarketSearch from "./MarketSearch.vue";
import { Button } from "@/sdk/ui/button";
import { Skeleton } from "@/sdk/ui/skeleton";
import { Item, ItemActions, ItemContent, ItemDescription, ItemMedia, ItemTitle } from "@/sdk/ui/item";
import { useDraft } from "../../stores/drafts";
import { markets, ordered, ready, refresh, remove, watchlist, type Quote } from "./store";
import { locale } from "../../sdk/i18n";
import { t } from "../../notch/i18n";

const query = useDraft("markets.query", () => "");
const m = computed(() => markets[watchlist.tab]);
const list = computed(() => ordered(watchlist.tab));
const firstLoad = computed(() => !m.value.loaded && !m.value.error && watchlist[watchlist.tab].length > 0);

let timer: number | undefined;
onMounted(async () => {
  await ready;
  void refresh();
  timer = window.setInterval(() => void refresh(), 60_000);
});
onBeforeUnmount(() => window.clearInterval(timer));
watch(() => watchlist.tab, () => void refresh());
watch(
  () => watchlist.currency,
  () => void refresh("crypto"),
);

function price(q: Quote) {
  // Bourse de Londres : cotations en pence (« GBp »).
  const pence = q.currency === "GBp";
  const v = pence ? q.price / 100 : q.price;
  const cur = pence ? "GBP" : q.currency;
  const digits = v >= 1000 ? 2 : v >= 1 ? 2 : v >= 0.01 ? 4 : 6;
  try {
    if (!cur) throw 0;
    return new Intl.NumberFormat(locale.value, { style: "currency", currency: cur, maximumFractionDigits: digits }).format(v);
  } catch {
    return new Intl.NumberFormat(locale.value, { maximumFractionDigits: digits }).format(v) + (cur ? ` ${cur}` : "");
  }
}

const pct = (n: number) =>
  new Intl.NumberFormat(locale.value, { style: "percent", signDisplay: "exceptZero", maximumFractionDigits: 2, minimumFractionDigits: 2 }).format(n / 100);

/** Courbe ramenée à 0..100 (min → bas, max → haut). */
function curve(points: number[]) {
  const lo = Math.min(...points);
  const hi = Math.max(...points);
  return points.map((p) => (hi === lo ? 50 : ((p - lo) / (hi - lo)) * 100));
}

const initials = (q: Quote) => q.symbol.replace(/^\^/, "").replace(/[^A-Za-z0-9]/g, "").slice(0, 2).toUpperCase() || "?";

const errorText = (code: string | null) =>
  t(code === "rateLimited" ? "markets.rateLimited" : "markets.unavailable");
</script>

<template>
  <div class="flex h-full flex-col gap-2 px-4">
    <Teleport defer to="#notch-header-actions">
      <Segmented
        v-model="watchlist.tab"
        size="sm"
        :options="[
          { value: 'stocks', label: t('markets.stocks') },
          { value: 'crypto', label: t('markets.crypto') },
        ]"
      />
      <IconButton :icon="PhArrowClockwise" :label="t('markets.refresh')" :loading="m.loading" :disabled="m.loading" @click="refresh()" />
    </Teleport>

    <MarketSearch />

    <div
      v-if="query.trim().length < 2"
      class="min-h-0 flex-1 overflow-y-auto [scrollbar-color:var(--fill-3)_transparent] [scrollbar-width:thin]"
    >
      <div v-if="firstLoad" class="flex flex-col gap-1.5 py-1">
        <Skeleton v-for="i in Math.min(4, watchlist[watchlist.tab].length || 3)" :key="i" class="h-10 rounded-[10px]" />
      </div>
      <Empty v-else-if="m.error && !list.length" :icon="PhWarningCircle" :title="errorText(m.error)">
        <Button :loading="m.loading" :disabled="m.loading" @click="refresh()">{{ t("markets.retry") }}</Button>
      </Empty>
      <Empty
        v-else-if="!list.length"
        :icon="PhChartLineUp"
        :title="t('markets.empty')"
        :text="t(watchlist.tab === 'stocks' ? 'markets.emptyStocks' : 'markets.emptyCrypto')"
      />
      <TransitionGroup
        v-else
        tag="div"
        move-class="transition-transform duration-240 ease-out"
        leave-active-class="overflow-hidden transition-[height,opacity] duration-150 ease-out"
        leave-to-class="h-0! opacity-0"
      >
        <Item v-for="q in list" :key="q.id" class="group/row">
          <ItemMedia v-if="q.image" variant="image" class="rounded-full"><img :src="q.image" alt="" /></ItemMedia>
          <ItemMedia v-else variant="icon" class="type-caption rounded-full font-semibold">{{ initials(q) }}</ItemMedia>
          <ItemContent>
            <ItemTitle class="w-full truncate">{{ q.name }}</ItemTitle>
            <ItemDescription>{{ q.symbol }}{{ q.exchange ? ` · ${q.exchange}` : "" }}</ItemDescription>
          </ItemContent>
          <Sparkline
            v-if="q.points.length > 1"
            :values="curve(q.points)"
            still
            :width="96"
            :height="24"
            :color="q.change >= 0 ? 'var(--sys-green)' : 'var(--sys-red)'"
            class="shrink-0"
          />
          <div class="flex w-[104px] shrink-0 flex-col items-end gap-0.5">
            <span class="type-footnote font-semibold tabular-nums">{{ price(q) }}</span>
            <span
              class="type-caption rounded-full px-1.5 font-medium tabular-nums"
              :class="q.change >= 0 ? 'bg-sys-green/15 text-sys-green' : 'bg-sys-red/15 text-sys-red'"
            >
              {{ pct(q.change) }}
            </span>
          </div>
          <ItemActions class="-mr-1 w-[22px] shrink-0">
            <Button
              variant="ghost"
              size="icon-xs"
              class="size-[22px] text-label-3 opacity-0 transition-opacity duration-150 ease-out hover:bg-transparent hover:text-label-2 active:scale-100 group-hover/row:opacity-100"
              :aria-label="t('markets.remove')"
              :title="t('markets.remove')"
              @click="remove(watchlist.tab, q.id)"
            >
              <PhX :size="12" weight="bold" />
            </Button>
          </ItemActions>
        </Item>
      </TransitionGroup>
    </div>

    <div class="type-caption flex h-5 shrink-0 items-center gap-2 text-label-3">
      <span v-if="m.error && list.length" class="flex items-center gap-1 text-sys-orange">
        <PhWarningCircle :size="12" weight="bold" />{{ errorText(m.error) }}
      </span>
      <span class="truncate">
        {{ t("markets.source", { source: watchlist.tab === "stocks" ? "Yahoo Finance" : "CoinGecko" }) }}
      </span>
      <button
        v-if="watchlist.tab === 'crypto'"
        type="button"
        class="ml-auto rounded-full px-1.5 font-medium text-label-2 hover:bg-fill-2"
        :title="t('markets.currency')"
        data-no-drag
        @click="watchlist.currency = watchlist.currency === 'eur' ? 'usd' : 'eur'"
      >
        {{ watchlist.currency.toUpperCase() }}
      </button>
    </div>
  </div>
</template>
