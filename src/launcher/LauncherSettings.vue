<script setup lang="ts">
// Réglages › Launcher : ouverture (raccourci OU touche Windows), suspension (plein écran,
// apps exclues choisies parmi les apps ouvertes), moteur de recherche web, préfixes.
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { PhPlus, PhX } from "@phosphor-icons/vue";
import Group from "../ui/Group.vue";
import Row from "../ui/Row.vue";
import Segmented from "../ui/Segmented.vue";
import ShortcutField from "../ui/ShortcutField.vue";
import TextField from "../ui/TextField.vue";
import Kbd from "../ui/Kbd.vue";
import { Button } from "@/sdk/ui/button";
import { Switch } from "@/sdk/ui/switch";
import { Skeleton } from "@/sdk/ui/skeleton";
import { settings } from "../notch/settings";
import { t } from "../notch/i18n";
import { shortcutOk } from "./store";
import { useDraft } from "../stores/drafts";

interface Proc {
  exe: string;
  title: string;
  icon: string | null;
}

const cfg = settings.launcher;
const picking = ref(false);
const loading = ref(false);
const procs = ref<Proc[]>([]);
const manual = useDraft("launcher.excludeInput", () => "");

const available = computed(() => procs.value.filter((p) => !cfg.exclude.some((e) => e.toLowerCase() === p.exe.toLowerCase())));

async function pick() {
  if (picking.value) {
    picking.value = false;
    return;
  }
  picking.value = true;
  loading.value = true;
  try {
    procs.value = await invoke<Proc[]>("launcher_processes").catch(() => []);
  } finally {
    loading.value = false;
  }
}

function add(exe: string) {
  const e = exe.trim();
  if (!e) return;
  const name = /\.exe$/i.test(e) ? e : `${e}.exe`;
  if (!cfg.exclude.some((x) => x.toLowerCase() === name.toLowerCase())) cfg.exclude.push(name);
}

function addManual() {
  add(manual.value);
  manual.value = "";
}

function remove(exe: string) {
  cfg.exclude = cfg.exclude.filter((x) => x !== exe);
}

const PREFIXES = ["a", "f", "d", "s", "c"] as const;
</script>

<template>
  <div>
    <Group :title="t('launcher.settings.open')" section="launcher" :note="t('launcher.settings.openNote')">
      <Row :label="t('launcher.settings.winKey')" :help="t('launcher.settings.winKeyHelp')">
        <Switch v-model="cfg.winKey" />
      </Row>
      <Row
        :label="t('launcher.settings.shortcut')"
        :help="cfg.winKey ? t('launcher.settings.shortcutOff') : shortcutOk ? t('launcher.settings.shortcutHelp') : t('launcher.settings.shortcutTaken')"
      >
        <ShortcutField v-if="!cfg.winKey" v-model="cfg.shortcut" />
        <Kbd v-else :keys="['Win']" />
      </Row>
      <Row :label="t('launcher.settings.fullscreen')" :help="t('launcher.settings.fullscreenHelp')">
        <Switch v-model="cfg.fullscreenOff" />
      </Row>
      <Row :label="t('launcher.settings.engine')">
        <Segmented
          v-model="cfg.engine"
          size="sm"
          :options="[
            { value: 'google', label: 'Google' },
            { value: 'duckduckgo', label: 'DuckDuckGo' },
            { value: 'bing', label: 'Bing' },
          ]"
        />
      </Row>
    </Group>

    <Group :title="t('launcher.settings.exclude')" :note="t('launcher.settings.excludeNote')">
      <Row :label="t('launcher.settings.excludeWhen')" :help="t('launcher.settings.excludeWhenHelp')">
        <Segmented
          v-model="cfg.excludeWhen"
          size="sm"
          :options="[
            { value: 'focused', label: t('launcher.settings.excludeFocused') },
            { value: 'running', label: t('launcher.settings.excludeRunning') },
          ]"
        />
      </Row>
      <Row v-for="exe in cfg.exclude" :key="exe" :label="exe">
        <Button size="icon-sm" variant="ghost" :title="t('launcher.settings.remove')" @click="remove(exe)"><PhX weight="bold" /></Button>
      </Row>
      <Row :label="t('launcher.settings.addOpen')" :help="t('launcher.settings.addOpenHelp')">
        <Button :loading="loading" @click="pick"><PhPlus weight="bold" />{{ picking ? t("launcher.settings.done") : t("launcher.settings.choose") }}</Button>
      </Row>
      <template v-if="picking">
        <div v-if="loading" class="flex flex-col gap-1.5 px-3 py-2">
          <Skeleton v-for="i in 3" :key="i" class="h-8 rounded-[8px]" />
        </div>
        <div v-else-if="!available.length" class="type-caption px-3 py-2 text-label-3">{{ t("launcher.settings.noApps") }}</div>
        <div v-else class="flex max-h-[220px] flex-col overflow-y-auto px-1.5 py-1 [scrollbar-width:thin]">
          <button
            v-for="p in available"
            :key="p.exe"
            type="button"
            class="flex h-10 items-center gap-2.5 rounded-[8px] px-2 text-left transition-colors duration-100 hover:bg-fill-2"
            @click="add(p.exe)"
          >
            <img v-if="p.icon" :src="p.icon" alt="" class="size-5 shrink-0 object-contain" />
            <span v-else class="size-5 shrink-0 rounded-[5px] bg-fill-3" />
            <span class="min-w-0 flex-1">
              <span class="type-footnote block truncate font-medium">{{ p.exe }}</span>
              <span class="type-caption block truncate text-label-3">{{ p.title }}</span>
            </span>
            <PhPlus :size="13" weight="bold" class="shrink-0 text-label-3" />
          </button>
        </div>
      </template>
      <Row :label="t('launcher.settings.addManual')">
        <div class="w-[180px]"><TextField v-model="manual" placeholder="UnrealEditor.exe" @enter="addManual" /></div>
        <Button :disabled="!manual.trim()" @click="addManual">{{ t("launcher.settings.add") }}</Button>
      </Row>
    </Group>

    <Group :title="t('launcher.settings.prefixes')">
      <Row v-for="p in PREFIXES" :key="p" :label="t(`launcher.prefix.${p}`)">
        <Kbd :keys="[`${p}:`]" />
      </Row>
    </Group>
  </div>
</template>
