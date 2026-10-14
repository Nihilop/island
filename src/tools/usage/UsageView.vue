<script setup lang="ts">
// Outil « Quotas IA » (600 × 320) : une carte par fournisseur actif — plan, fenêtres
// (session, semaine…) avec barre (orange ≥ 80, rouge ≥ 95), reset, part restante.
// Polling 60 s (le cache Rust rend ça gratuit) ; horloge relative toutes les 30 s.
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { PhArrowsClockwise, PhGauge, PhSlidersHorizontal, PhWarningCircle } from "@phosphor-icons/vue";
import IconButton from "../../ui/IconButton.vue";
import Empty from "../../ui/Empty.vue";
import { Button } from "@/sdk/ui/button";
import { Badge } from "@/sdk/ui/badge";
import { Skeleton } from "@/sdk/ui/skeleton";
import { settings } from "../../notch/settings";
import { openSettings } from "../settings/nav";
import { t } from "../../notch/i18n";

interface Usage {
  provider: string;
  plan: string | null;
  windows: { kind: string; used: number; resetsAt: number | null }[];
  error: string | null;
  fetchedAt: number;
}

const data = ref<Usage[]>([]);
const loading = ref(false);
const now = ref(Date.now() / 1000);
const active = computed(() => (["claude", "chatgpt"] as const).filter((p) => settings.usageProviders[p]));

async function load(force = false) {
  if (!active.value.length) return;
  loading.value = true;
  data.value = await invoke<Usage[]>("usage_get", { providers: active.value, force }).catch(() => data.value);
  loading.value = false;
}

let poll: number | undefined;
let clock: number | undefined;
onMounted(() => {
  void load();
  poll = window.setInterval(load, 60_000);
  clock = window.setInterval(() => (now.value = Date.now() / 1000), 30_000);
});
onBeforeUnmount(() => {
  window.clearInterval(poll);
  window.clearInterval(clock);
});

function resetIn(at: number | null) {
  if (!at) return "";
  const s = Math.max(0, at - now.value);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  return t("usage.resetIn", { t: h >= 24 ? `${Math.floor(h / 24)} j ${h % 24} h` : h ? `${h} h ${m} min` : `${m} min` });
}
function ago(at: number) {
  const m = Math.floor((now.value - at) / 60);
  return m < 1 ? t("clip.now") : t("usage.ago", { n: m < 60 ? `${m} min` : `${Math.floor(m / 60)} h` });
}
const color = (u: number) => (u >= 95 ? "var(--sys-red)" : u >= 80 ? "var(--sys-orange)" : "var(--label)");
</script>

<template>
  <div class="h-full px-4">
    <Teleport defer to="#notch-header-actions">
      <IconButton :icon="PhArrowsClockwise" :label="t('usage.refresh')" :loading="loading" @click="load(true)" />
      <IconButton :icon="PhSlidersHorizontal" :label="t('usage.manage')" @click="openSettings('usage')" />
    </Teleport>

    <Empty v-if="!active.length" :icon="PhGauge" :title="t('usage.none')" :text="t('usage.noneText')">
      <Button @click="openSettings('usage')">{{ t("usage.manage") }}</Button>
    </Empty>
    <div v-else class="flex h-full gap-2 overflow-x-auto [scrollbar-width:none]">
      <template v-if="!data.length">
        <Skeleton v-for="p in active" :key="p" class="flex flex-[1_0_168px] flex-col gap-2.5 rounded-[14px] bg-fill-1 p-3 animate-[pulse_1.4s_ease-in-out_infinite]" />
      </template>
      <div v-for="u in data" :key="u.provider" class="flex flex-[1_0_168px] flex-col gap-2.5 rounded-[14px] bg-fill-1 p-3">
        <div class="flex items-center gap-1.5">
          <span class="type-footnote font-semibold">{{ t(`usage.provider.${u.provider}`) }}</span>
          <Badge v-if="u.plan" class="type-caption h-auto font-normal capitalize">{{ u.plan }}</Badge>
        </div>
        <div v-if="!u.windows.length && u.error" class="type-caption flex items-center gap-1.5 text-label-2">
          <PhWarningCircle :size="14" weight="fill" class="text-sys-orange" />
          {{ t(`usage.err.${u.error}`) }}
        </div>
        <div v-for="w in u.windows" :key="w.kind" class="flex flex-col gap-1">
          <div class="flex justify-between">
            <span class="type-caption text-label-2">{{ t(`usage.window.${w.kind}`) }}</span>
            <span class="type-caption font-semibold tabular-nums" :style="{ color: color(w.used) }">{{ Math.round(w.used) }} %</span>
          </div>
          <div class="h-[5px] overflow-hidden rounded-full bg-fill-2"><div class="h-full origin-left transition-transform duration-360 ease-out" :style="{ transform: `scaleX(${Math.min(1, w.used / 100)})`, background: color(w.used) }" /></div>
          <div class="flex justify-between type-caption text-label-3">
            <span>{{ resetIn(w.resetsAt) }}</span>
            <span :style="{ color: w.used >= 95 ? 'var(--sys-red)' : undefined }">{{ t("usage.left", { n: Math.max(0, Math.round(100 - w.used)) }) }}</span>
          </div>
        </div>
        <div class="type-caption mt-auto">
          <span v-if="u.error && u.windows.length" class="text-sys-orange">{{ t("usage.stale", { t: ago(u.fetchedAt) }) }}</span>
          <span v-else-if="u.fetchedAt" class="text-label-4">{{ ago(u.fetchedAt) }}</span>
        </div>
      </div>
    </div>
  </div>
</template>
