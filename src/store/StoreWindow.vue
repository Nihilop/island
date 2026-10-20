<script setup lang="ts">
// Fenêtre Magasin (`store-main`), façon App Store : barre latérale (Découvrir, Fonds animés,
// Extensions, Bibliothèque, étiquettes, compte), recherche, tri, grille ou liste, fiche d'un
// élément. Ouverte depuis la Vue d'ensemble et l'outil Fonds animés ; arguments :
// `{ section?, item? }`. Les installations passent par `./local` (consentement des
// extensions dans la fenêtre d'installation).
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  PhArrowsClockwise,
  PhCaretLeft,
  PhCaretRight,
  PhCompass,
  PhImageSquare,
  PhList,
  PhMagnifyingGlass,
  PhMinus,
  PhPuzzlePiece,
  PhSquaresFour,
  PhStack,
  PhStorefront,
  PhTag,
  PhX,
} from "@phosphor-icons/vue";
import IconButton from "../ui/IconButton.vue";
import Segmented from "../ui/Segmented.vue";
import StoreAccount from "../tools/wallpaper/StoreAccount.vue";
import { Input } from "@/sdk/ui/input";
import { Button } from "@/sdk/ui/button";
import { Spinner } from "@/sdk/ui/spinner";
import ItemCard from "./ItemCard.vue";
import ItemRow from "./ItemRow.vue";
import ItemDetail from "./ItemDetail.vue";
import { local, watchLocal } from "./local";
import { catalog, checkUpdates, item as fetchItem, market, marketError, mediaUrl, PAGE, tags as fetchTags, updates, type Card, type Detail } from "../market/api";
import { t } from "../notch/i18n";

type Section = "home" | "scenes" | "extensions" | "library";
const win = getCurrentWindow();

const section = ref<Section>("home");
const kind = computed(() => (section.value === "scenes" ? "scene" : section.value === "extensions" ? "extension" : null));

// --- Préférences d'affichage (par poste) ------------------------------------------------

const VIEW_KEY = "island.store.view";
const view = ref<"grid" | "list">("grid");
try {
  if (localStorage.getItem(VIEW_KEY) === "list") view.value = "list";
} catch {
  /* stockage indisponible */
}
watch(view, (v) => {
  try {
    localStorage.setItem(VIEW_KEY, v);
  } catch {
    /* rien */
  }
});

// --- Recherche, filtres ------------------------------------------------------------------

const typed = ref("");
const q = ref("");
const sort = ref<"popular" | "recent">("popular");
const tag = ref<string | null>(null);
const page = ref(0);
let timer: number | undefined;
watch(typed, (v) => {
  window.clearTimeout(timer);
  timer = window.setTimeout(() => (q.value = v.trim()), 300);
});

const tagList = ref<{ tag: string; count: number }[]>([]);
async function loadTags() {
  tagList.value = kind.value ? await fetchTags(kind.value).catch(() => []) : [];
}

// --- Liste ---------------------------------------------------------------------------------

const items = ref<Card[]>([]);
const total = ref(0);
const loading = ref(false);
const error = ref<string | null>(null);
const pages = computed(() => Math.max(1, Math.ceil(total.value / PAGE)));
/** Recherche depuis Découvrir : sur tout le magasin. */
const searching = computed(() => section.value === "home" && !!q.value);

let seq = 0;
async function loadList() {
  if (section.value === "library" || (section.value === "home" && !q.value)) return;
  const n = ++seq;
  loading.value = true;
  error.value = null;
  try {
    const p = await catalog({ kind: kind.value, q: q.value || undefined, tag: tag.value ?? undefined, sort: sort.value, page: page.value });
    if (n !== seq) return;
    items.value = p.items;
    total.value = p.total;
  } catch (e) {
    if (n === seq) error.value = marketError(e);
  } finally {
    if (n === seq) loading.value = false;
  }
}

// --- Découvrir -----------------------------------------------------------------------------

const home = ref<{ scenes: Card[]; extensions: Card[]; fresh: Card[] } | null>(null);
async function loadHome() {
  loading.value = true;
  error.value = null;
  try {
    const [scenes, extensions, fresh] = await Promise.all([
      catalog({ kind: "scene", sort: "popular" }),
      catalog({ kind: "extension", sort: "popular" }),
      catalog({ kind: null, sort: "recent" }),
    ]);
    home.value = { scenes: scenes.items.slice(0, 8), extensions: extensions.items.slice(0, 8), fresh: fresh.items.slice(0, 8) };
  } catch (e) {
    error.value = marketError(e);
  } finally {
    loading.value = false;
  }
}
const hero = computed(() => home.value?.scenes.find((c) => c.thumbnail) ?? home.value?.extensions[0] ?? null);

// --- Bibliothèque : installés, mises à jour en tête -------------------------------------

const library = ref<Card[]>([]);
async function loadLibrary() {
  loading.value = true;
  error.value = null;
  try {
    const ids = [...Object.keys(market.installed), ...local.extensions.keys()];
    const found = await Promise.all(ids.map((id) => fetchItem(id).catch(() => null)));
    const list = found.filter((x): x is Detail => !!x);
    const rank = (c: Card) => (updates.list.some((u) => u.id === c.id) ? 0 : 1);
    library.value = list.sort((a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name));
  } catch (e) {
    error.value = marketError(e);
  } finally {
    loading.value = false;
  }
}

// --- Fiche ---------------------------------------------------------------------------------

const detail = ref<Detail | null>(null);
const opening = ref<string | null>(null);
async function open(c: Pick<Card, "id">) {
  if (opening.value) return;
  opening.value = c.id;
  try {
    detail.value = await fetchItem(c.id);
    document.getElementById("store-main")?.scrollTo({ top: 0 });
  } catch (e) {
    error.value = marketError(e);
  } finally {
    opening.value = null;
  }
}

// --- Navigation ----------------------------------------------------------------------------

function go(s: Section, opts: { tag?: string | null } = {}) {
  detail.value = null;
  section.value = s;
  tag.value = opts.tag ?? null;
  page.value = 0;
  if (s !== "home") {
    typed.value = "";
    q.value = "";
  }
}
function filterTag(tg: string) {
  go(detail.value?.kind === "extension" || section.value === "extensions" ? "extensions" : "scenes", { tag: tg });
}

watch([section], () => {
  void loadTags();
  if (section.value === "home") void loadHome();
  else if (section.value === "library") void loadLibrary();
  else void loadList();
});
watch([q, sort, tag], () => {
  page.value = 0;
  void loadList();
});
watch(page, () => void loadList());
watch(
  () => updates.list.length,
  () => section.value === "library" && void loadLibrary(),
);

const NAV = computed(() => [
  { id: "home" as Section, label: t("store.nav.discover"), icon: PhCompass },
  { id: "scenes" as Section, label: t("store.nav.scenes"), icon: PhImageSquare },
  { id: "extensions" as Section, label: t("store.nav.extensions"), icon: PhPuzzlePiece },
  { id: "library" as Section, label: t("store.nav.library"), icon: PhStack, badge: updates.list.length },
]);
const title = computed(() => (searching.value ? t("store.results", { q: q.value }) : (NAV.value.find((n) => n.id === section.value)?.label ?? "")));

// --- Fenêtre -------------------------------------------------------------------------------

function applyTheme(v: "dark" | "light") {
  const root = document.documentElement;
  root.classList.toggle("light", v === "light");
  root.classList.toggle("dark", v === "dark");
  root.style.colorScheme = v;
}

function route(args: { section?: Section; item?: string } | null) {
  if (args?.section) go(args.section);
  if (args?.item) void open({ id: args.item });
}

const unlisten: UnlistenFn[] = [];
onMounted(async () => {
  watchLocal();
  unlisten.push(await listen<"dark" | "light">("theme://changed", (e) => applyTheme(e.payload)));
  void emit("theme://get");
  unlisten.push(await listen<{ section?: Section; item?: string }>("window://args", (e) => route(e.payload)));
  const args = await invoke<{ section?: Section; item?: string } | null>("window_args").catch(() => null);
  if (!args?.section || args.section === "home") void loadHome();
  route(args);
  void loadTags();
});
onBeforeUnmount(() => unlisten.forEach((u) => u()));
</script>

<template>
  <div class="flex h-screen flex-col overflow-hidden bg-elevated text-label">
    <!-- Barre de titre -->
    <header class="flex h-11 shrink-0 items-center gap-3 border-b border-separator pr-1 pl-4" data-tauri-drag-region>
      <PhStorefront :size="15" class="text-label-3" />
      <span class="type-footnote w-[150px] font-semibold" data-tauri-drag-region>{{ t("store.title") }}</span>
      <div class="relative w-full max-w-[420px]">
        <PhMagnifyingGlass :size="13" class="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-label-3" />
        <Input v-model="typed" class="pl-8" :placeholder="kind ? t('store.searchIn', { where: title }) : t('store.search')" />
      </div>
      <div class="flex-1 self-stretch" data-tauri-drag-region />
      <IconButton :icon="PhMinus" :label="t('notes.minimize')" @click="win.minimize()" />
      <IconButton :icon="PhX" :label="t('common.close')" class="hover:bg-sys-red! hover:text-white!" @click="win.close()" />
    </header>

    <div class="flex min-h-0 flex-1">
      <!-- Barre latérale -->
      <aside class="flex w-[210px] shrink-0 flex-col border-r border-separator">
        <nav class="flex flex-col gap-0.5 p-2">
          <button
            v-for="n in NAV"
            :key="n.id"
            type="button"
            class="type-footnote flex items-center gap-2.5 rounded-[8px] px-2.5 py-1.5 text-left transition-colors"
            :class="section === n.id && !searching ? 'bg-fill-2 font-medium text-label' : 'text-label-2 hover:bg-fill-1'"
            @click="go(n.id)"
          >
            <component :is="n.icon" :size="15" :weight="section === n.id ? 'fill' : 'regular'" />
            <span class="flex-1">{{ n.label }}</span>
            <span v-if="n.badge" class="type-caption min-w-[18px] rounded-full bg-sys-link px-1.5 text-center font-medium text-white">{{ n.badge }}</span>
          </button>
        </nav>

        <div v-if="tagList.length" class="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
          <div class="type-caption flex items-center gap-1 px-2.5 pt-3 pb-1 font-semibold text-label-3"><PhTag :size="11" />{{ t("store.tags") }}</div>
          <button
            v-for="tg in tagList"
            :key="tg.tag"
            type="button"
            class="type-footnote flex w-full items-center justify-between rounded-[8px] px-2.5 py-1 text-left transition-colors"
            :class="tag === tg.tag ? 'bg-sys-link/15 text-sys-link' : 'text-label-2 hover:bg-fill-1'"
            @click="tag = tag === tg.tag ? null : tg.tag"
          >
            <span class="truncate">{{ tg.tag }}</span><span class="type-caption text-label-4">{{ tg.count }}</span>
          </button>
        </div>
        <div v-else class="flex-1" />

        <div class="border-t border-separator p-3"><StoreAccount /></div>
      </aside>

      <!-- Contenu -->
      <main id="store-main" class="min-w-0 flex-1 overflow-y-auto px-7 py-6">
        <ItemDetail v-if="detail" :item="detail" @back="detail = null" @tag="filterTag" />

        <template v-else>
          <div v-if="error" class="type-footnote mb-4 flex items-center gap-2 rounded-[10px] bg-sys-red/10 px-3 py-2 text-sys-red">
            <span class="flex-1">{{ error }}</span>
            <Button size="sm" @click="section === 'home' ? loadHome() : section === 'library' ? loadLibrary() : loadList()">{{ t("market.retry") }}</Button>
          </div>

          <!-- Découvrir -->
          <template v-if="section === 'home' && !searching">
            <div v-if="loading && !home" class="flex h-60 items-center justify-center"><Spinner /></div>
            <template v-else-if="home">
              <button
                v-if="hero"
                type="button"
                class="relative block aspect-[3/1] w-full overflow-hidden rounded-[18px] bg-fill-2 bg-cover bg-center text-left"
                :style="hero.thumbnail ? { backgroundImage: `url('${mediaUrl(hero.thumbnail)}')` } : {}"
                @click="open(hero)"
              >
                <div class="absolute inset-0 bg-gradient-to-t from-black/75 via-black/10 to-transparent" />
                <div class="absolute bottom-0 left-0 p-6 text-white">
                  <div class="type-caption font-semibold tracking-wide uppercase opacity-80">{{ t("store.featured") }}</div>
                  <div class="type-display font-semibold">{{ hero.name }}</div>
                  <div class="type-footnote opacity-80">{{ hero.summary }}</div>
                </div>
              </button>

              <section v-for="row in [
                { key: 'scenes', title: t('store.row.scenes'), list: home.scenes, more: 'scenes' as Section },
                { key: 'extensions', title: t('store.row.extensions'), list: home.extensions, more: 'extensions' as Section },
                { key: 'fresh', title: t('store.row.fresh'), list: home.fresh, more: null },
              ]" :key="row.key" class="mt-8">
                <template v-if="row.list.length">
                  <div class="mb-3 flex items-baseline justify-between">
                    <h2 class="type-title font-semibold">{{ row.title }}</h2>
                    <button v-if="row.more" type="button" class="type-footnote text-sys-link hover:underline" @click="go(row.more)">{{ t("store.seeAll") }}</button>
                  </div>
                  <div class="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-x-5 gap-y-6">
                    <ItemCard v-for="c in row.list" :key="c.id" :item="c" :show-kind="row.key === 'fresh'" @open="open(c)" />
                  </div>
                </template>
              </section>
              <div v-if="!home.scenes.length && !home.extensions.length" class="flex flex-col items-center gap-2 py-20 text-label-3">
                <PhStorefront :size="32" /><p class="type-footnote">{{ t("store.empty") }}</p>
              </div>
            </template>
          </template>

          <!-- Bibliothèque -->
          <template v-else-if="section === 'library'">
            <div class="mb-4 flex items-center justify-between">
              <h1 class="type-title font-semibold">{{ t("store.nav.library") }}</h1>
              <Button size="sm" :disabled="updates.checking" @click="checkUpdates">
                <Spinner v-if="updates.checking" /><PhArrowsClockwise v-else weight="bold" />{{ t("ext.update.check") }}
              </Button>
            </div>
            <p class="type-footnote mb-3 text-label-3">
              {{ updates.list.length ? t("store.library.updates", { n: updates.list.length }) : t("store.library.upToDate") }}
            </p>
            <div v-if="loading && !library.length" class="flex h-40 items-center justify-center"><Spinner /></div>
            <p v-else-if="!library.length" class="type-footnote py-10 text-center text-label-3">{{ t("store.library.empty") }}</p>
            <div v-else class="flex flex-col gap-1">
              <ItemRow v-for="c in library" :key="c.id" :item="c" show-kind @open="open(c)" />
            </div>
          </template>

          <!-- Liste : une section, ou une recherche -->
          <template v-else>
            <div class="mb-4 flex flex-wrap items-center gap-3">
              <h1 class="type-title flex-1 font-semibold">
                {{ title }}
                <button v-if="tag" type="button" class="type-caption ml-2 inline-flex items-center gap-1 rounded-full bg-sys-link px-2 py-0.5 align-middle text-white" @click="tag = null">
                  {{ tag }}<PhX :size="10" weight="bold" />
                </button>
              </h1>
              <Segmented
                v-model="sort"
                size="sm"
                :options="[
                  { value: 'popular', label: t('market.sortPopular') },
                  { value: 'recent', label: t('market.sortRecent') },
                ]"
              />
              <div class="flex rounded-[8px] bg-fill-1 p-0.5">
                <IconButton :icon="PhSquaresFour" :active="view === 'grid'" :label="t('store.view.grid')" @click="view = 'grid'" />
                <IconButton :icon="PhList" :active="view === 'list'" :label="t('store.view.list')" @click="view = 'list'" />
              </div>
            </div>

            <div v-if="loading && !items.length" class="flex h-60 items-center justify-center"><Spinner /></div>
            <div v-else-if="!items.length" class="flex flex-col items-center gap-2 py-20 text-label-3">
              <PhMagnifyingGlass :size="28" /><p class="type-footnote">{{ t("store.noResult") }}</p>
            </div>
            <div v-else class="relative">
              <div v-if="loading" class="absolute inset-0 z-10 flex items-start justify-center pt-20"><Spinner /></div>
              <div v-if="view === 'grid'" class="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-x-5 gap-y-6" :class="loading ? 'opacity-50' : ''">
                <ItemCard v-for="c in items" :key="c.id" :item="c" :show-kind="!kind" @open="open(c)" />
              </div>
              <div v-else class="flex flex-col gap-1" :class="loading ? 'opacity-50' : ''">
                <ItemRow v-for="c in items" :key="c.id" :item="c" :show-kind="!kind" @open="open(c)" />
              </div>
            </div>

            <div v-if="pages > 1" class="mt-6 flex items-center justify-center gap-2">
              <IconButton :icon="PhCaretLeft" :label="t('wall.prev')" :disabled="page === 0 || loading" @click="page--" />
              <span class="type-caption tabular-nums text-label-3">{{ page + 1 }} / {{ pages }}</span>
              <IconButton :icon="PhCaretRight" :label="t('wall.next')" :disabled="page >= pages - 1 || loading" @click="page++" />
            </div>
          </template>
        </template>
        <p v-if="local.error && !detail" class="type-footnote mt-4 rounded-[10px] bg-sys-red/10 px-3 py-2 text-sys-red">{{ local.error }}</p>
      </main>
    </div>
  </div>
</template>
