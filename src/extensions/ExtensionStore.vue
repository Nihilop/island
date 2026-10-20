<script setup lang="ts">
// Réglages › Extensions › Magasin : extensions publiées (relues puis signées par le
// magasin). « Installer » télécharge le paquet par le Rust, puis ouvre la fenêtre
// d'installation (permissions en clair, état « Vérifiée par le magasin »).
import { computed, onMounted, ref, watch } from "vue";
import { PhCheck, PhDownloadSimple, PhStorefront } from "@phosphor-icons/vue";
import { invoke } from "@tauri-apps/api/core";
import Group from "../ui/Group.vue";
import Row from "../ui/Row.vue";
import Empty from "../ui/Empty.vue";
import TextField from "../ui/TextField.vue";
import { Button } from "@/sdk/ui/button";
import { Spinner } from "@/sdk/ui/spinner";
import { catalog, marketError, mediaUrl, type Page } from "../market/api";
import { all } from "./runtime";
import { openPackage } from "./install";
import { t } from "../notch/i18n";

const typed = ref("");
const q = ref("");
const page = ref<Page | null>(null);
const loading = ref(false);
const error = ref<string | null>(null);
const busy = ref<string | null>(null);

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
    const p = await catalog({ kind: "extension", sort: "popular", q: q.value || undefined });
    if (n === seq) page.value = p;
  } catch (e) {
    if (n === seq) error.value = marketError(e);
  } finally {
    if (n === seq) loading.value = false;
  }
}
watch(q, () => void load());

/** Version installée de chaque extension (pour « Installée » / « Mettre à jour »). */
const local = computed(() => new Map(all.value.map((x) => [x.manifest.id, x.manifest.version])));

async function install(id: string) {
  if (busy.value) return;
  busy.value = id;
  error.value = null;
  try {
    const path = await invoke<string>("market_download_extension", { id });
    await openPackage(path);
  } catch (e) {
    error.value = marketError(e);
  } finally {
    busy.value = null;
  }
}

onMounted(load);
</script>

<template>
  <Group :title="t('ext.store.title')" :note="t('ext.store.note')">
    <div class="px-3.5 pt-3 pb-3">
      <TextField v-model="typed" :placeholder="t('ext.store.search')" />
    </div>
    <div v-if="error" class="type-footnote border-t border-separator px-3 py-2 text-sys-red">{{ error }}</div>
    <div v-if="loading && !page" class="flex justify-center border-t border-separator py-6"><Spinner /></div>
    <div v-else-if="page && !page.items.length" class="border-t border-separator px-4 py-2">
      <Empty :icon="PhStorefront" :title="q ? t('ext.store.noResult') : t('ext.store.empty')" />
    </div>
    <template v-else-if="page">
      <Row v-for="x in page.items" :key="x.id" :label="`${x.name} ${x.version ?? ''}`" :help="x.summary || x.id">
        <span
          v-if="x.thumbnail"
          class="h-6 w-10 shrink-0 rounded-[4px] bg-fill-2 bg-cover bg-center"
          :style="{ backgroundImage: `url('${mediaUrl(x.thumbnail)}')` }"
        />
        <span class="type-caption text-label-3">{{ x.author }}</span>
        <span v-if="local.get(x.id) === x.version" class="type-caption flex items-center gap-1 text-sys-green"><PhCheck weight="bold" />{{ t("ext.store.installed") }}</span>
        <Button v-else size="sm" :loading="busy === x.id" :disabled="!!busy" @click="install(x.id)">
          <PhDownloadSimple weight="bold" />{{ local.has(x.id) ? t("ext.store.update") : t("ext.store.install") }}
        </Button>
      </Row>
    </template>
  </Group>
</template>
