<script setup lang="ts">
// Fiche d'un élément dans la fenêtre Magasin : bannière, en-tête et action, captures
// (agrandies au clic), description, permissions (extension), informations, versions.
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { PhArrowLeft, PhArrowSquareOut, PhX } from "@phosphor-icons/vue";
import IconButton from "../ui/IconButton.vue";
import ActionButton from "./ActionButton.vue";
import { local } from "./local";
import { mediaUrl, updateFor, type Detail } from "../market/api";
import { t } from "../notch/i18n";

const props = defineProps<{ item: Detail }>();
defineEmits<{ back: []; tag: [string] }>();

const zoom = ref<string | null>(null);
const update = computed(() => updateFor(props.item.id));
const banner = computed(() => mediaUrl(props.item.media.banner ?? props.item.media.thumbnail));

function permLabel(p: string): string {
  if (p.startsWith("network:")) return t("ext.perm.network", { host: p.slice(8) });
  if (p.startsWith("images:")) return t("ext.perm.images", { host: p.slice(7) });
  const key = `ext.perm.${p}`;
  const s = t(key);
  return s === key ? p : s;
}
const sensitive = (p: string) => ["capture", "activity", "terminal", "docker"].includes(p);
const size = (b: number) => (b < 1 << 20 ? `${Math.max(1, Math.round(b / 1024))} Ko` : `${(b / (1 << 20)).toFixed(1)} Mo`);
const date = (iso: string) => new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(iso));
const openLink = (url: string) => void invoke("open_url", { url });
</script>

<template>
  <div class="flex flex-col">
    <div class="mb-3 flex items-center gap-2">
      <IconButton :icon="PhArrowLeft" :label="t('market.back')" @click="$emit('back')" />
      <span class="type-footnote text-label-3">{{ item.kind === "scene" ? t("store.nav.scenes") : t("store.nav.extensions") }}</span>
    </div>

    <div class="aspect-[3/1] w-full overflow-hidden rounded-[16px] bg-fill-2 bg-cover bg-center" :style="banner ? { backgroundImage: `url('${banner}')` } : {}" />

    <div class="mt-4 flex items-start gap-4">
      <div class="min-w-0 flex-1">
        <h1 class="type-title font-semibold">{{ item.name }}</h1>
        <div class="type-footnote mt-0.5 flex flex-wrap items-center gap-1.5 text-label-3">
          <img v-if="item.authorAvatar" :src="item.authorAvatar" alt="" class="size-4 rounded-full" />
          <span>{{ item.author }}</span>
          <span>·</span>
          <span>{{ t("market.downloads", { n: item.downloads }) }}</span>
        </div>
        <p v-if="item.summary" class="type-body mt-2 text-label-2">{{ item.summary }}</p>
      </div>
      <div class="flex shrink-0 flex-col items-end gap-1">
        <ActionButton :item="item" size="default" />
        <span v-if="update" class="type-caption text-sys-link">{{ t("store.updateTo", { v: update.version }) }}</span>
      </div>
    </div>
    <p v-if="local.error" class="type-footnote mt-2 rounded-[8px] bg-sys-red/10 px-3 py-1.5 text-sys-red">{{ local.error }}</p>

    <div v-if="item.media.screenshots.length" class="mt-5 flex gap-3 overflow-x-auto pb-2">
      <button
        v-for="(s, i) in item.media.screenshots"
        :key="s"
        type="button"
        class="aspect-video h-40 shrink-0 overflow-hidden rounded-[12px] bg-fill-2 bg-cover bg-center ring-1 ring-separator"
        :style="{ backgroundImage: `url('${mediaUrl(s)}')` }"
        :title="t('market.screenshot', { n: i + 1 })"
        @click="zoom = mediaUrl(s)"
      />
    </div>

    <div class="mt-5 grid grid-cols-[1fr_240px] gap-6">
      <div class="min-w-0">
        <p v-if="item.description" class="type-body whitespace-pre-wrap text-label-2">{{ item.description }}</p>
        <div v-if="item.tags.length" class="mt-4 flex flex-wrap gap-1.5">
          <button v-for="tg in item.tags" :key="tg" type="button" class="type-caption rounded-full bg-fill-2 px-2.5 py-0.5 text-label-2 hover:bg-fill-3" @click="$emit('tag', tg)">
            {{ tg }}
          </button>
        </div>

        <section v-if="item.kind === 'extension'" class="mt-6">
          <h2 class="type-footnote mb-2 font-semibold">{{ t("ext.permissions") }}</h2>
          <p v-if="!item.permissions?.length" class="type-footnote text-label-3">{{ t("ext.consent.noPerms") }}</p>
          <ul v-else class="flex flex-col gap-1.5">
            <li
              v-for="p in item.permissions"
              :key="p"
              class="type-footnote flex items-center gap-2 rounded-[8px] bg-fill-1 px-3 py-1.5"
              :class="sensitive(p) ? 'text-sys-red' : 'text-label-2'"
            >
              <span class="size-1.5 shrink-0 rounded-full bg-current" />{{ permLabel(p) }}
            </li>
          </ul>
        </section>

        <section v-if="item.versions.length" class="mt-6">
          <h2 class="type-footnote mb-2 font-semibold">{{ t("market.versions") }}</h2>
          <div v-for="v in item.versions" :key="v.version" class="border-b border-separator py-2 last:border-0">
            <div class="type-footnote flex gap-2">
              <span class="font-medium">v{{ v.version }}</span><span class="text-label-3">{{ date(v.createdAt) }}</span>
            </div>
            <p v-if="v.changelog" class="type-caption mt-0.5 whitespace-pre-wrap text-label-2">{{ v.changelog }}</p>
          </div>
        </section>
      </div>

      <aside class="type-caption flex flex-col gap-2 text-label-3">
        <div class="rounded-[12px] bg-fill-1 p-3">
          <div class="flex justify-between py-1"><span>{{ t("store.info.version") }}</span><span class="text-label-2">{{ item.version ?? "—" }}</span></div>
          <div v-if="item.versions[0]" class="flex justify-between py-1"><span>{{ t("store.info.size") }}</span><span class="text-label-2">{{ size(item.versions[0].size) }}</span></div>
          <div class="flex justify-between py-1"><span>{{ t("store.info.updated") }}</span><span class="text-label-2">{{ date(item.updatedAt) }}</span></div>
          <div v-if="item.license" class="flex justify-between py-1"><span>{{ t("ext.publishWin.license") }}</span><span class="text-label-2">{{ item.license }}</span></div>
        </div>
        <button v-if="item.homepage" type="button" class="flex items-center gap-1 text-sys-link hover:underline" @click="openLink(item.homepage)">
          <PhArrowSquareOut :size="12" />{{ t("market.homepage") }}
        </button>
        <button v-if="item.repository" type="button" class="flex items-center gap-1 text-sys-link hover:underline" @click="openLink(item.repository)">
          <PhArrowSquareOut :size="12" />{{ t("market.repository") }}
        </button>
      </aside>
    </div>

    <!-- Capture agrandie -->
    <div v-if="zoom" class="fixed inset-0 z-50 flex items-center justify-center bg-black/80 p-10" @click="zoom = null">
      <img :src="zoom" alt="" class="max-h-full max-w-full rounded-[12px]" />
      <IconButton :icon="PhX" :label="t('common.close')" class="absolute top-4 right-4 bg-black/50! text-white!" @click="zoom = null" />
    </div>
  </div>
</template>
