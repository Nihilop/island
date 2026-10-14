<script setup lang="ts">
// Outil « Notifications » (520 × 300) : le centre de notifications Windows, dans le notch.
// Clic sur une carte = aperçu complet (le texte se déplie) ; survol = ouvrir l'app /
// fermer. « Tout effacer » ferme aussi tout dans Windows.
import { onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { PhArrowSquareOut, PhBell, PhBellSlash, PhTrash, PhX } from "@phosphor-icons/vue";
import Empty from "../../ui/Empty.vue";
import IconButton from "../../ui/IconButton.vue";
import { Button } from "@/sdk/ui/button";
import { clearAll, dismiss, history, notifs, openApp, refreshAccess, requestAccess, type Notif } from "./store";
import { useDraft } from "../../stores/drafts";
import { t } from "../../notch/i18n";

const expanded = useDraft<string | null>("notifs.expanded", () => null);
const asking = ref(false);
const clearing = ref(false);
const clearError = ref(false);

async function clear() {
  if (clearing.value) return;
  clearing.value = true;
  clearError.value = false;
  try {
    clearError.value = (await clearAll()) !== null;
    if (clearError.value) window.setTimeout(() => (clearError.value = false), 2600);
  } finally {
    clearing.value = false;
  }
}

// Âge relatif, rafraîchi toutes les 30 s.
const now = ref(Date.now());
let ageTimer: number | undefined;
onMounted(() => {
  void refreshAccess();
  ageTimer = window.setInterval(() => (now.value = Date.now()), 30_000);
});
onBeforeUnmount(() => window.clearInterval(ageTimer));

function age(at: number): string {
  const m = Math.floor((now.value - at) / 60_000);
  if (m < 1) return t("clip.now");
  if (m < 60) return t("clip.min", { n: m });
  const h = Math.floor(m / 60);
  return h < 24 ? t("clip.hour", { n: h }) : t("clip.day", { n: Math.floor(h / 24) });
}

function toggle(n: Notif) {
  expanded.value = expanded.value === n.id ? null : n.id;
}

async function ask() {
  if (asking.value) return;
  asking.value = true;
  try {
    await requestAccess();
  } finally {
    asking.value = false;
  }
}
</script>

<template>
  <div class="flex h-full flex-col px-4">
    <div v-if="clearError" class="type-caption pb-1 text-sys-red">{{ t("notifs.clearFailed") }}</div>
    <Teleport defer to="#notch-header-actions">
      <span v-if="history.length" class="type-caption mr-1 text-label-3">{{ history.length }}</span>
      <IconButton :icon="PhTrash" :label="t('notifs.clear')" :loading="clearing" :disabled="!history.length || clearing" @click="clear" />
    </Teleport>

    <Empty
      v-if="notifs.access === 'unsupported' && !history.length"
      :icon="PhBellSlash"
      :title="t('notifs.unsupported')"
    />
    <Empty
      v-else-if="notifs.access !== 'allowed' && !history.length"
      :icon="PhBellSlash"
      :title="t('notifs.access.title')"
      :text="t(notifs.access === 'denied' ? 'notifs.access.denied' : 'notifs.access.text')"
    >
      <Button v-if="notifs.access === 'denied'" @click="invoke('notifs_open_settings')">{{ t("notifs.access.settings") }}</Button>
      <Button v-else variant="default" :loading="asking" @click="ask">{{ t("notifs.access.allow") }}</Button>
    </Empty>
    <Empty v-else-if="!history.length" :icon="PhBell" :title="t('notifs.empty')" />

    <div v-else class="-mx-1 flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto px-1 pb-1 [scrollbar-color:var(--fill-3)_transparent] [scrollbar-width:thin]">
      <TransitionGroup
        move-class="transition-transform duration-240 ease-out"
        enter-active-class="transition-[opacity,translate] duration-240 ease-out"
        enter-from-class="opacity-0 -translate-y-1"
        leave-active-class="absolute inset-x-1 transition-opacity duration-160 ease-out"
        leave-to-class="opacity-0"
      >
        <div
          v-for="n in history"
          :key="n.id"
          class="group/n relative flex cursor-default gap-2.5 rounded-[12px] bg-fill-1 px-2.5 py-2 transition-colors duration-150 hover:bg-fill-2"
          @click="toggle(n)"
        >
          <img v-if="n.icon" :src="n.icon" alt="" class="mt-0.5 size-7 shrink-0 rounded-[7px] object-contain" />
          <span v-else class="mt-0.5 grid size-7 shrink-0 place-items-center rounded-[7px] bg-fill-3"><PhBell :size="14" weight="fill" /></span>
          <div class="min-w-0 flex-1">
            <div class="flex items-baseline gap-1.5">
              <span class="type-caption truncate text-label-3">{{ n.app }}</span>
              <span class="type-caption shrink-0 text-label-4">{{ age(n.time) }}</span>
            </div>
            <div v-if="n.title" class="type-footnote font-medium" :class="expanded === n.id ? 'break-words' : 'truncate'">{{ n.title }}</div>
            <div
              v-if="n.body"
              class="type-caption whitespace-pre-line text-label-2"
              :class="expanded === n.id ? 'break-words select-text' : 'line-clamp-2'"
            >{{ n.body }}</div>
            <Button v-if="n.action" size="sm" variant="secondary" class="mt-1.5" @click.stop="n.action.run()">{{ n.action.label }}</Button>
          </div>
          <div class="absolute top-1.5 right-1.5 flex gap-0.5 opacity-0 transition-opacity duration-120 group-hover/n:opacity-100">
            <Button v-if="n.native?.aumid" size="icon-xs" class="bg-fill-3 text-label-2" :title="t('notifs.open')" @click.stop="openApp(n)">
              <PhArrowSquareOut :size="11" weight="bold" class="size-[11px]" />
            </Button>
            <Button size="icon-xs" class="bg-fill-3 text-label-2" :title="t('notifs.dismiss')" @click.stop="dismiss(n.id)">
              <PhX :size="11" weight="bold" class="size-[11px]" />
            </Button>
          </div>
        </div>
      </TransitionGroup>
    </div>
  </div>
</template>
