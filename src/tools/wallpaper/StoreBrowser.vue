<script setup lang="ts">
// Onglet « Magasin » de l'outil Fonds animés : catalogue des scènes publiées (recherche,
// tri, étiquettes, pages), puis la fiche d'une scène (bannière, captures, description,
// versions) avec Installer → Appliquer. L'installation crée une scène locale, comme un
// fichier partagé ; elle s'applique ensuite à la cible choisie dans l'outil.
import { computed, onMounted, ref, watch } from "vue";
import { PhArrowLeft, PhArrowSquareOut, PhCaretLeft, PhCaretRight, PhCheck, PhDownloadSimple, PhStorefront, PhWarning, PhX } from "@phosphor-icons/vue";
import { invoke } from "@tauri-apps/api/core";
import IconButton from "../../ui/IconButton.vue";
import TextField from "../../ui/TextField.vue";
import Segmented from "../../ui/Segmented.vue";
import { Button } from "@/sdk/ui/button";
import { Spinner } from "@/sdk/ui/spinner";
import { catalog, install, item, market, marketError, mediaUrl, PAGE, watchInstalled, type Card, type Detail } from "../../market/api";
import { apply } from "../../wallpaper/api";
import { t } from "../../notch/i18n";

const props = defineProps<{ target: string | null; current: string | null }>();

// --- Catalogue ------------------------------------------------------------------------------

const typed = ref("");
const q = ref("");
const sort = ref<"recent" | "popular">("recent");
const tag = ref<string | null>(null);
const page = ref(0);
const items = ref<Card[]>([]);
const total = ref(0);
const loading = ref(false);
const error = ref<string | null>(null);
const pages = computed(() => Math.max(1, Math.ceil(total.value / PAGE)));

let timer: number | undefined;
watch(typed, (v) => {
  window.clearTimeout(timer);
  timer = window.setTimeout(() => (q.value = v.trim()), 300);
});

let seq = 0;
async function load() {
  const n = ++seq;
  loading.value = true;
  error.value = null;
  try {
    const p = await catalog({ q: q.value || undefined, tag: tag.value ?? undefined, sort: sort.value, page: page.value });
    if (n !== seq) return;
    items.value = p.items;
    total.value = p.total;
  } catch (e) {
    if (n === seq) error.value = marketError(e);
  } finally {
    if (n === seq) loading.value = false;
  }
}
watch([q, sort, tag], () => {
  page.value = 0;
  void load();
});
watch(page, () => void load());

// --- Fiche ----------------------------------------------------------------------------------

const detail = ref<Detail | null>(null);
const opening = ref<string | null>(null);
async function open(c: Card) {
  if (opening.value) return;
  opening.value = c.id;
  error.value = null;
  try {
    detail.value = await item(c.id);
  } catch (e) {
    error.value = marketError(e);
  } finally {
    opening.value = null;
  }
}
function filterTag(tg: string) {
  detail.value = null;
  tag.value = tg;
}

const installed = computed(() => (detail.value ? market.installed[detail.value.id] : undefined));
const outdated = computed(() => !!installed.value && !!detail.value?.version && installed.value.version !== detail.value.version);
const busy = ref<"install" | "apply" | null>(null);
const actionError = ref<string | null>(null);

async function doInstall() {
  if (!detail.value || busy.value) return;
  busy.value = "install";
  actionError.value = null;
  try {
    await install(detail.value.id);
  } catch (e) {
    actionError.value = marketError(e);
  } finally {
    busy.value = null;
  }
}
async function doApply() {
  if (!installed.value || busy.value) return;
  busy.value = "apply";
  actionError.value = null;
  try {
    await apply(installed.value.scene, props.target);
  } catch (e) {
    actionError.value = String(e);
  } finally {
    busy.value = null;
  }
}
const applied = computed(() => !!installed.value && props.current === installed.value.scene);

const openLink = (url: string) => void invoke("open_url", { url });
const size = (b: number) => (b < 1 << 20 ? `${Math.max(1, Math.round(b / 1024))} Ko` : `${(b / (1 << 20)).toFixed(1)} Mo`);
const date = (iso: string) => new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(iso));
const cover = (u: string | null | undefined) => {
  const url = mediaUrl(u);
  return url ? { backgroundImage: `url("${url}")`, backgroundSize: "cover", backgroundPosition: "center" } : {};
};

onMounted(() => {
  watchInstalled();
  void load();
});
</script>

<template>
  <!-- Fiche -->
  <div v-if="detail" class="flex flex-col gap-3">
    <div class="flex items-center gap-2">
      <IconButton :icon="PhArrowLeft" :label="t('market.back')" @click="detail = null" />
      <span class="type-footnote min-w-0 flex-1 truncate font-medium">{{ detail.name }}</span>
    </div>

    <div class="aspect-[3/1] w-full overflow-hidden rounded-[10px] bg-fill-2" :style="cover(detail.media.banner ?? detail.media.thumbnail)" />

    <div class="flex items-start gap-3">
      <div class="min-w-0 flex-1">
        <div class="type-body truncate font-semibold">{{ detail.name }}</div>
        <div class="type-caption mt-0.5 flex items-center gap-1.5 text-label-3">
          <img v-if="detail.authorAvatar" :src="detail.authorAvatar" alt="" class="size-4 rounded-full" />
          <span>{{ detail.author }}</span>
          <span>·</span>
          <span>{{ t("market.downloads", { n: detail.downloads }) }}</span>
          <template v-if="detail.version"><span>·</span><span>v{{ detail.version }}</span></template>
          <template v-if="detail.license"><span>·</span><span>{{ detail.license }}</span></template>
        </div>
      </div>
      <div class="flex shrink-0 items-center gap-1.5">
        <template v-if="installed">
          <Button v-if="outdated" size="sm" variant="secondary" :disabled="!!busy" @click="doInstall">
            <Spinner v-if="busy === 'install'" /><PhDownloadSimple v-else />{{ t("market.installVersion", { v: detail.version ?? "" }) }}
          </Button>
          <Button size="sm" :disabled="!!busy || applied" @click="doApply">
            <Spinner v-if="busy === 'apply'" /><PhCheck v-else-if="applied" />{{ applied ? t("market.applied") : t("wall.apply") }}
          </Button>
        </template>
        <Button v-else size="sm" :disabled="!!busy" @click="doInstall">
          <Spinner v-if="busy === 'install'" /><PhDownloadSimple v-else />{{ busy === "install" ? t("market.installing") : t("market.install") }}
        </Button>
      </div>
    </div>
    <p v-if="installed && !outdated" class="type-caption -mt-1.5 text-label-3">{{ t("market.installedHint") }}</p>

    <div v-if="actionError" class="type-caption flex items-center gap-1.5 rounded-[8px] bg-sys-red/10 px-2 py-1 text-sys-red">
      <PhWarning :size="12" weight="bold" />{{ actionError }}
    </div>

    <p v-if="detail.summary" class="type-footnote text-label-2">{{ detail.summary }}</p>

    <div v-if="detail.tags.length" class="flex flex-wrap gap-1">
      <button
        v-for="tg in detail.tags"
        :key="tg"
        type="button"
        class="type-caption rounded-full bg-fill-2 px-2 py-0.5 text-label-2 hover:bg-fill-3"
        :title="t('market.filterTag')"
        @click="filterTag(tg)"
      >
        {{ tg }}
      </button>
    </div>

    <div v-if="detail.media.screenshots.length" class="flex gap-2 overflow-x-auto pb-1">
      <button
        v-for="(s, i) in detail.media.screenshots"
        :key="s"
        type="button"
        class="aspect-video h-24 shrink-0 overflow-hidden rounded-[8px] bg-fill-2"
        :style="cover(s)"
        :title="t('market.screenshot', { n: i + 1 })"
        @click="openLink(mediaUrl(s) ?? '')"
      />
    </div>

    <p v-if="detail.description" class="type-footnote whitespace-pre-wrap text-label-2">{{ detail.description }}</p>

    <div v-if="detail.homepage || detail.repository" class="flex flex-wrap gap-3">
      <button v-if="detail.homepage" type="button" class="type-caption flex items-center gap-1 text-sys-link hover:underline" @click="openLink(detail.homepage)">
        <PhArrowSquareOut :size="11" />{{ t("market.homepage") }}
      </button>
      <button v-if="detail.repository" type="button" class="type-caption flex items-center gap-1 text-sys-link hover:underline" @click="openLink(detail.repository)">
        <PhArrowSquareOut :size="11" />{{ t("market.repository") }}
      </button>
    </div>

    <div v-if="detail.versions.length" class="rounded-[10px] bg-fill-1 px-3 py-2">
      <div class="type-caption mb-1 font-medium text-label-2">{{ t("market.versions") }}</div>
      <div v-for="v in detail.versions" :key="v.version" class="type-caption flex gap-2 py-0.5 text-label-3">
        <span class="w-10 shrink-0 font-medium text-label-2">v{{ v.version }}</span>
        <span class="w-24 shrink-0">{{ date(v.createdAt) }}</span>
        <span class="w-14 shrink-0">{{ size(v.size) }}</span>
        <span class="min-w-0 flex-1 truncate" :title="v.changelog">{{ v.changelog }}</span>
      </div>
    </div>
  </div>

  <!-- Catalogue -->
  <div v-else class="flex flex-col">
    <div class="mb-2.5 flex items-center gap-2">
      <div class="min-w-0 flex-1"><TextField v-model="typed" :placeholder="t('market.search')" /></div>
      <Segmented
        v-model="sort"
        size="sm"
        :options="[
          { value: 'recent', label: t('market.sortRecent') },
          { value: 'popular', label: t('market.sortPopular') },
        ]"
      />
    </div>
    <div v-if="tag" class="mb-2 flex">
      <button type="button" class="type-caption flex items-center gap-1 rounded-full bg-sys-link px-2 py-0.5 text-white" @click="tag = null">
        {{ tag }}<PhX :size="10" weight="bold" />
      </button>
    </div>

    <div v-if="error" class="type-caption mb-2 flex items-center gap-1.5 rounded-[8px] bg-sys-red/10 px-2 py-1 text-sys-red">
      <PhWarning :size="12" weight="bold" /><span class="flex-1">{{ error }}</span>
      <button type="button" class="font-medium underline" @click="load">{{ t("market.retry") }}</button>
    </div>

    <div v-if="loading && !items.length" class="flex justify-center py-10"><Spinner /></div>
    <div v-else class="relative grid grid-cols-4 gap-x-2.5 gap-y-2 p-0.5">
      <div v-if="loading" class="absolute inset-0 z-10 flex items-center justify-center"><Spinner /></div>
      <button v-for="c in items" :key="c.id" type="button" class="group min-w-0 text-left" :disabled="!!opening" @click="open(c)">
        <div class="relative aspect-video w-full overflow-hidden rounded-[8px] bg-fill-2 ring-2 ring-transparent transition-[box-shadow] group-hover:ring-fill-3" :style="cover(c.thumbnail)">
          <span v-if="opening === c.id" class="absolute inset-0 flex items-center justify-center bg-black/30"><Spinner class="text-white" /></span>
          <span v-if="market.installed[c.id]" class="absolute top-1 right-1 flex size-4 items-center justify-center rounded-full bg-sys-green text-white" :title="t('market.installed')">
            <PhCheck :size="10" weight="bold" />
          </span>
        </div>
        <div class="type-caption mt-1 truncate text-label">{{ c.name }}</div>
        <div class="type-caption truncate text-label-3">{{ c.author }} · {{ t("market.downloads", { n: c.downloads }) }}</div>
      </button>
    </div>
    <div v-if="!loading && !error && !items.length" class="flex flex-col items-center gap-1 py-8 text-label-3">
      <PhStorefront :size="22" />
      <p class="type-footnote">{{ q || tag ? t("market.noResult") : t("market.empty") }}</p>
    </div>

    <div v-if="pages > 1" class="mt-2 flex items-center justify-end gap-1">
      <IconButton :icon="PhCaretLeft" :label="t('wall.prev')" :disabled="page === 0 || loading" @click="page--" />
      <span class="type-caption tabular-nums text-label-3">{{ page + 1 }} / {{ pages }}</span>
      <IconButton :icon="PhCaretRight" :label="t('wall.next')" :disabled="page >= pages - 1 || loading" @click="page++" />
    </div>
  </div>
</template>
