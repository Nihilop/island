<script setup lang="ts">
// Outil « Activité » (560 × 340) : jeu en cours (jaquette, source, durée), temps de jeu du
// jour / des 7 derniers jours (et des apps si suivies), apps ouvertes avec correction du
// classement (jeu · app · ignorer). Tout reste sur ce PC.
import { computed, onMounted, ref, watch } from "vue";
import { PhAppWindow, PhArrowsClockwise, PhGameController, PhListBullets } from "@phosphor-icons/vue";
import IconButton from "../../ui/IconButton.vue";
import Segmented from "../../ui/Segmented.vue";
import Empty from "../../ui/Empty.vue";
import { activity, duration, overrides, playing, refreshCatalog, running, setOverride, stats, type ActApp, type DayStats, type Kind, type Usage } from "./store";
import { t } from "../../notch/i18n";

type Range = "today" | "week";
const tab = ref<"stats" | "apps">("stats");
const range = ref<Range>("today");
const days = ref<DayStats[]>([]);
const apps = ref<ActApp[]>([]);
const busy = ref<string | null>(null);
const loading = ref(false);

const iso = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;

async function loadStats() {
  loading.value = true;
  const to = new Date();
  const from = new Date();
  if (range.value === "week") from.setDate(from.getDate() - 6);
  days.value = await stats(iso(from), iso(to)).catch(() => []);
  loading.value = false;
}

/** Apps ignorées (le service les liste encore comme « app ») : chemins en minuscules. */
const ignored = ref(new Set<string>());
async function loadApps() {
  const [list, ov] = await Promise.all([running().catch(() => []), overrides().catch(() => [])]);
  apps.value = list;
  ignored.value = new Set(ov.filter((o) => o.kind === "ignore").map((o) => o.path.toLowerCase()));
}

onMounted(() => {
  void loadStats();
  void loadApps();
});
watch(range, loadStats);
watch(tab, (v) => v === "apps" && loadApps());
// Le jeu en cours change : les totaux aussi.
watch(() => activity.game?.key, loadStats);

/** Totaux sur la période, triés. */
function total(pick: (d: DayStats) => Usage[]): Usage[] {
  const map = new Map<string, Usage>();
  for (const d of days.value)
    for (const u of pick(d)) {
      const cur = map.get(u.key);
      map.set(u.key, { key: u.key, name: u.name, seconds: (cur?.seconds ?? 0) + u.seconds });
    }
  return [...map.values()].filter((u) => u.seconds >= 60).sort((a, b) => b.seconds - a.seconds);
}
const games = computed(() => total((d) => d.games));
const appsUsage = computed(() => total((d) => d.apps));
const maxOf = (list: Usage[]) => Math.max(1, ...list.map((u) => u.seconds));
const totalGames = computed(() => games.value.reduce((s, u) => s + u.seconds, 0));

const SOURCE: Record<string, string> = {
  steam: "Steam",
  epic: "Epic",
  gog: "GOG",
  ea: "EA",
  ubisoft: "Ubisoft",
  battlenet: "Battle.net",
  xbox: "Xbox",
  discord: "Discord",
  heuristic: t("activity.source.heuristic"),
  user: t("activity.source.user"),
};

async function classify(a: ActApp, kind: Kind | "ignore") {
  if (busy.value) return;
  busy.value = a.path;
  try {
    // Re-cliquer « Ignorer » sur une app ignorée annule la correction.
    await setOverride(a.path, kind === "ignore" && ignored.value.has(a.path.toLowerCase()) ? null : kind);
    await loadApps();
  } finally {
    busy.value = null;
  }
}

async function rescan() {
  if (busy.value) return;
  busy.value = "rescan";
  try {
    await refreshCatalog();
    await loadApps();
  } finally {
    busy.value = null;
  }
}

const classOptions = computed(() => [
  { value: "game" as const, label: t("activity.kind.game") },
  { value: "app" as const, label: t("activity.kind.app") },
]);
</script>

<template>
  <div class="flex h-full flex-col gap-2.5 px-4 pb-2">
    <Teleport defer to="#notch-header-actions">
      <IconButton :icon="PhArrowsClockwise" :label="t('activity.rescan')" :loading="busy === 'rescan'" :disabled="!!busy" @click="rescan" />
    </Teleport>

    <!-- En cours -->
    <div class="flex items-center gap-3 rounded-[14px] bg-fill-1 p-2.5">
      <template v-if="playing">
        <img v-if="playing.art" :src="playing.art" alt="" class="h-[52px] w-[110px] shrink-0 rounded-[9px] object-cover" />
        <span v-else class="grid size-[52px] shrink-0 place-items-center rounded-[12px] bg-fill-2 text-sys-green"><PhGameController :size="24" weight="fill" /></span>
        <div class="min-w-0 flex-1">
          <div class="type-caption flex items-center gap-1.5 text-sys-green">
            <span class="size-1.5 rounded-full bg-sys-green" />{{ t("activity.playing") }}
            <span v-if="SOURCE[playing.source]" class="text-label-3">· {{ SOURCE[playing.source] }}</span>
          </div>
          <div class="type-body truncate font-semibold">{{ playing.name }}</div>
          <div class="type-caption tabular-nums text-label-2">{{ t("activity.since", { d: duration(playing.elapsed) }) }}</div>
        </div>
      </template>
      <template v-else>
        <span class="grid size-[52px] shrink-0 place-items-center rounded-[12px] bg-fill-2 text-label-3">
          <img v-if="activity.foreground?.icon" :src="activity.foreground.icon" alt="" class="size-7" />
          <PhAppWindow v-else :size="22" weight="duotone" />
        </span>
        <div class="min-w-0 flex-1">
          <div class="type-caption text-label-3">{{ t("activity.noGame") }}</div>
          <div class="type-body truncate">{{ activity.foreground?.name ?? "—" }}</div>
        </div>
      </template>
    </div>

    <div class="flex items-center gap-2">
      <Segmented
        v-model="tab"
        size="sm"
        :options="[
          { value: 'stats', label: t('activity.tab.stats') },
          { value: 'apps', label: t('activity.tab.apps') },
        ]"
      />
      <Segmented
        v-if="tab === 'stats'"
        v-model="range"
        size="sm"
        class="ml-auto"
        :options="[
          { value: 'today', label: t('activity.today') },
          { value: 'week', label: t('activity.week') },
        ]"
      />
    </div>

    <!-- Temps passé -->
    <div v-if="tab === 'stats'" class="min-h-0 flex-1 overflow-y-auto pr-1 [scrollbar-width:thin]">
      <Empty v-if="!loading && !games.length && !appsUsage.length" :icon="PhGameController" :title="t('activity.empty')" :text="t('activity.emptyHelp')" />
      <template v-else>
        <div v-if="games.length" class="type-caption mb-1 flex justify-between text-label-3">
          <span>{{ t("activity.games") }}</span><span class="tabular-nums">{{ duration(totalGames * 1000) }}</span>
        </div>
        <div v-for="u in games.slice(0, 6)" :key="u.key" class="mb-1.5">
          <div class="type-footnote flex justify-between gap-3">
            <span class="truncate">{{ u.name }}</span><span class="shrink-0 tabular-nums text-label-2">{{ duration(u.seconds * 1000) }}</span>
          </div>
          <div class="mt-1 h-1.5 rounded-full bg-fill-1">
            <div class="h-full rounded-full bg-sys-green transition-[width] duration-300" :style="{ width: `${(u.seconds / maxOf(games)) * 100}%` }" />
          </div>
        </div>
        <template v-if="appsUsage.length">
          <div class="type-caption mt-2 mb-1 text-label-3">{{ t("activity.apps") }}</div>
          <div v-for="u in appsUsage.slice(0, 6)" :key="u.key" class="mb-1.5">
            <div class="type-footnote flex justify-between gap-3">
              <span class="truncate">{{ u.name }}</span><span class="shrink-0 tabular-nums text-label-2">{{ duration(u.seconds * 1000) }}</span>
            </div>
            <div class="mt-1 h-1.5 rounded-full bg-fill-1">
              <div class="h-full rounded-full bg-sys-link transition-[width] duration-300" :style="{ width: `${(u.seconds / maxOf(appsUsage)) * 100}%` }" />
            </div>
          </div>
        </template>
      </template>
    </div>

    <!-- Apps ouvertes : corriger le classement -->
    <div v-else class="min-h-0 flex-1 overflow-y-auto pr-1 [scrollbar-width:thin]">
      <Empty v-if="!apps.length" :icon="PhListBullets" :title="t('activity.noApps')" />
      <div v-for="a in apps" :key="a.path" class="flex items-center gap-2.5 rounded-[10px] px-2 py-1.5 hover:bg-fill-1">
        <img v-if="a.icon" :src="a.icon" alt="" class="size-6 shrink-0" />
        <span v-else class="grid size-6 shrink-0 place-items-center text-label-3"><PhAppWindow :size="16" weight="bold" /></span>
        <div class="min-w-0 flex-1">
          <div class="type-footnote truncate">{{ a.name }}</div>
          <div class="type-caption truncate text-label-3">{{ a.exe }}<template v-if="SOURCE[a.source]"> · {{ SOURCE[a.source] }}</template></div>
        </div>
        <Segmented v-if="!ignored.has(a.path.toLowerCase())" :model-value="a.kind" size="sm" :options="classOptions" @update:model-value="classify(a, $event)" />
        <button
          type="button"
          class="type-caption shrink-0 rounded-[6px] px-1.5 py-0.5 hover:bg-fill-2"
          :class="ignored.has(a.path.toLowerCase()) ? 'bg-fill-2 text-label' : 'text-label-3 hover:text-label-2'"
          :disabled="busy === a.path"
          @click="classify(a, 'ignore')"
        >{{ ignored.has(a.path.toLowerCase()) ? t("activity.ignored") : t("activity.ignore") }}</button>
      </div>
    </div>
  </div>
</template>
