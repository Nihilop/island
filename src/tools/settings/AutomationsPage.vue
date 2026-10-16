<script setup lang="ts">
// Réglages › Automatisations : règles « pendant / quand … alors … » (cf. automations/store.ts).
// Liste (interrupteur, résumé, modifier / supprimer), éditeur en place (déclencheur, actions),
// modèles prêts à l'emploi, journal des dernières exécutions.
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { PhLightning, PhPencilSimple, PhPlay, PhPlus, PhTrash, PhX } from "@phosphor-icons/vue";
import Group from "../../ui/Group.vue";
import Row from "../../ui/Row.vue";
import Segmented from "../../ui/Segmented.vue";
import Select from "../../ui/Select.vue";
import TextField from "../../ui/TextField.vue";
import { Switch } from "@/sdk/ui/switch";
import { Button } from "@/sdk/ui/button";
import { Slider } from "@/sdk/ui/slider";
import { automations, deactivate, newRule, removeRule, testRule, REVERSIBLE, type Action, type Rule, type Trigger } from "../../automations/store";
import { activity, running, stats, type ActApp } from "../activity/store";
import { tools } from "../registry";
import { locale } from "../../sdk/i18n";
import { t } from "../../notch/i18n";

// --- Données des sélecteurs ------------------------------------------------------------------

const games = ref<{ key: string; name: string }[]>([]);
const apps = ref<ActApp[]>([]);
const outputs = ref<{ id: string; name: string; default: boolean }[]>([]);

async function loadPickers() {
  const iso = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  const [days, list, outs] = await Promise.all([
    stats(iso(new Date(Date.now() - 90 * 86_400_000)), iso(new Date())).catch(() => []),
    running().catch(() => [] as ActApp[]),
    invoke<{ id: string; name: string; default: boolean }[]>("audio_outputs").catch(() => []),
  ]);
  const map = new Map<string, string>();
  for (const d of days) for (const g of d.games) map.set(g.key, g.name);
  if (activity.game) map.set(activity.game.key, activity.game.name);
  games.value = [...map].map(([key, name]) => ({ key, name })).sort((a, b) => a.name.localeCompare(b.name));
  apps.value = list;
  outputs.value = outs;
}
onMounted(loadPickers);

// --- Libellés --------------------------------------------------------------------------------

const DAYS = computed(() => [1, 2, 3, 4, 5, 6, 0].map((d) => ({ d, label: new Intl.DateTimeFormat(locale.value, { weekday: "narrow" }).format(new Date(2026, 9, 11 + d)) })));

function triggerLabel(tr: Trigger): string {
  switch (tr.type) {
    case "game":
      return tr.key ? t("auto.trigger.gameOne", { name: tr.name ?? tr.key }) : t("auto.trigger.gameAny");
    case "app":
      return t("auto.trigger.appOne", { name: tr.name });
    case "time":
      return t("auto.trigger.timeRange", { from: tr.from, to: tr.to });
    case "start":
      return t("auto.trigger.start");
  }
}

function actionLabel(a: Action): string {
  switch (a.type) {
    case "volume":
      return t("auto.action.volumeAt", { n: Math.round(a.level * 100) });
    case "output":
      return a.name;
    case "launch":
      return t("auto.action.launchOne", { name: a.name });
    case "close":
      return t("auto.action.closeOne", { name: a.name });
    case "timer":
      return t("auto.action.timerFor", { n: a.minutes });
    case "tool":
      return t("auto.action.toolOne", { name: tools.value.find((x) => x.id === a.id)?.title() ?? a.id });
    case "notify":
      return `« ${a.text} »`;
    default:
      return t(`auto.action.${a.type}.${a.on ? "on" : "off"}`);
  }
}

const summary = (r: Rule) => `${triggerLabel(r.trigger)} → ${r.actions.length ? r.actions.map(actionLabel).join(" · ") : t("auto.noAction")}`;

// --- Édition ---------------------------------------------------------------------------------

const editing = ref<string | null>(null);
const rule = computed(() => automations.rules.find((r) => r.id === editing.value) ?? null);

async function edit(r: Rule) {
  // Une règle active sort proprement avant d'être modifiée (ses changements sont restaurés).
  await deactivate(r.id);
  editing.value = r.id;
  void loadPickers();
}

function add(partial?: Partial<Rule>) {
  const r = newRule(partial);
  automations.rules.push(r);
  void edit(r);
}

const TEMPLATES = computed<{ id: string; rule: Partial<Rule> }[]>(() => [
  {
    id: "game",
    rule: {
      name: t("auto.tpl.game"),
      trigger: { type: "game", key: null },
      actions: [
        { type: "notifications", on: false },
        { type: "gaming", on: true },
      ],
    },
  },
  {
    id: "night",
    rule: {
      name: t("auto.tpl.night"),
      trigger: { type: "time", from: "22:00", to: "07:00", days: [] },
      actions: [
        { type: "volume", level: 0.3 },
        { type: "notifications", on: false },
      ],
    },
  },
]);

const confirmDelete = ref<string | null>(null);
async function remove(id: string) {
  if (confirmDelete.value !== id) {
    confirmDelete.value = id;
    window.setTimeout(() => confirmDelete.value === id && (confirmDelete.value = null), 3000);
    return;
  }
  confirmDelete.value = null;
  if (editing.value === id) editing.value = null;
  await removeRule(id);
}

async function toggle(r: Rule, on: boolean) {
  if (!on) await deactivate(r.id);
  r.enabled = on;
}

// Déclencheur
const TRIGGERS = computed(() => [
  { value: "game" as const, label: t("auto.trigger.game") },
  { value: "app" as const, label: t("auto.trigger.app") },
  { value: "time" as const, label: t("auto.trigger.time") },
  { value: "start" as const, label: t("auto.trigger.startShort") },
]);
const triggerType = computed({
  get: () => rule.value?.trigger.type ?? "game",
  set: (v: Trigger["type"]) => {
    if (!rule.value) return;
    rule.value.trigger =
      v === "game" ? { type: "game", key: null }
        : v === "app" ? { type: "app", path: apps.value[0]?.path ?? "", name: apps.value[0]?.name ?? "" }
          : v === "time" ? { type: "time", from: "22:00", to: "07:00", days: [] }
            : { type: "start" };
  },
});
const gameOptions = computed(() => [{ value: null as string | null, label: t("auto.trigger.gameAny") }, ...games.value.map((g) => ({ value: g.key as string | null, label: g.name }))]);
const appOptions = computed(() => apps.value.map((a) => ({ value: a.path, label: `${a.name} (${a.exe})` })));

function setGame(key: string | null) {
  if (rule.value?.trigger.type !== "game") return;
  rule.value.trigger = { type: "game", key, name: games.value.find((g) => g.key === key)?.name };
}
function setApp(path: string) {
  if (rule.value?.trigger.type !== "app") return;
  const a = apps.value.find((x) => x.path === path);
  rule.value.trigger = { type: "app", path, name: a?.name ?? path };
}
function toggleDay(d: number) {
  const tr = rule.value?.trigger;
  if (tr?.type !== "time") return;
  tr.days = tr.days.includes(d) ? tr.days.filter((x) => x !== d) : [...tr.days, d];
}
const validTime = (s: string) => /^([01]?\d|2[0-3]):[0-5]\d$/.test(s);

// Actions
const ACTION_TYPES: Action["type"][] = ["notifications", "volume", "mute", "output", "gaming", "focus", "presentation", "launch", "close", "timer", "tool", "notify"];
const addOptions = computed(() => ACTION_TYPES.map((v) => ({ value: v, label: t(`auto.action.${v}`) })));
const addChoice = ref<Action["type"] | null>(null);

function addAction(type: Action["type"] | null) {
  addChoice.value = null;
  if (!rule.value || !type) return;
  const a: Action =
    type === "volume" ? { type, level: 0.5 }
      : type === "output" ? { type, id: outputs.value[0]?.id ?? "", name: outputs.value[0]?.name ?? "" }
        : type === "launch" ? { type, path: "", name: "" }
          : type === "close" ? { type, exe: "", name: "" }
            : type === "timer" ? { type, minutes: 25 }
              : type === "tool" ? { type, id: "overview" }
                : type === "notify" ? { type, text: "" }
                  : { type, on: type === "gaming" || type === "focus" || type === "presentation" || type === "mute" ? true : false };
  rule.value.actions.push(a);
}

const outputOptions = computed(() => outputs.value.map((o) => ({ value: o.id, label: o.name })));
const launchOptions = computed(() => [
  ...apps.value.map((a) => ({ value: a.path, label: a.name })),
  ...games.value
    .map((g) => ({ g, target: g.key.match(/^steam:(\d+)$/) ? `steam://rungameid/${g.key.slice(6)}` : /^[a-z]:\\/i.test(g.key) ? g.key : null }))
    .filter((x) => x.target)
    .map((x) => ({ value: x.target!, label: `${x.g.name} · ${t("auto.game")}` })),
]);
const closeOptions = computed(() => apps.value.map((a) => ({ value: a.exe, label: `${a.name} (${a.exe})` })));
const toolOptions = computed(() => tools.value.filter((x) => x.dock !== false).map((x) => ({ value: x.id, label: x.title() })));

function pickLaunch(a: Action, path: string) {
  if (a.type !== "launch") return;
  a.path = path;
  a.name = launchOptions.value.find((o) => o.value === path)?.label ?? path;
}
function pickClose(a: Action, exe: string) {
  if (a.type !== "close") return;
  a.exe = exe;
  a.name = apps.value.find((x) => x.exe === exe)?.name ?? exe;
}
function pickOutput(a: Action, id: string) {
  if (a.type !== "output") return;
  a.id = id;
  a.name = outputs.value.find((o) => o.id === id)?.name ?? id;
}

const testing = ref(false);
async function test() {
  if (!rule.value || testing.value) return;
  testing.value = true;
  try {
    await testRule(rule.value);
  } finally {
    testing.value = false;
  }
}

const logTime = (at: number) => new Intl.DateTimeFormat(locale.value, { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" }).format(new Date(at));
</script>

<template>
  <div>
    <Group :title="t('auto.title')" section="automations" :note="t('auto.note')">
      <div v-if="!automations.rules.length" class="type-caption px-3 py-3 text-label-3">{{ t("auto.empty") }}</div>
      <div v-for="r in automations.rules" :key="r.id" class="border-separator px-3 py-2 [&+&]:border-t">
        <div class="flex items-center gap-2">
          <PhLightning :size="14" :weight="r.id in automations.active ? 'fill' : 'bold'" :class="r.id in automations.active ? 'text-sys-yellow' : 'text-label-3'" class="shrink-0" />
          <div class="min-w-0 flex-1">
            <div class="type-body truncate">{{ r.name }}</div>
            <div class="type-caption truncate text-label-3">{{ summary(r) }}</div>
          </div>
          <Button size="icon-xs" variant="ghost" class="text-label-3" :title="t('auto.edit')" @click="editing === r.id ? (editing = null) : edit(r)">
            <component :is="editing === r.id ? PhX : PhPencilSimple" weight="bold" />
          </Button>
          <Button size="icon-xs" :variant="confirmDelete === r.id ? 'destructive' : 'ghost'" :class="confirmDelete === r.id ? '' : 'text-label-3'" :title="t('auto.delete')" @click="remove(r.id)">
            <PhTrash weight="bold" />
          </Button>
          <Switch :model-value="r.enabled" @update:model-value="toggle(r, $event)" />
        </div>

        <!-- Éditeur -->
        <div v-if="editing === r.id && rule" class="mt-2.5 flex flex-col gap-2.5 rounded-[10px] bg-fill-1 p-2.5">
          <TextField v-model="rule.name" :placeholder="t('auto.name')" />

          <div class="type-caption font-medium uppercase text-label-3">{{ t("auto.when") }}</div>
          <Segmented v-model="triggerType" size="sm" :options="TRIGGERS" />
          <Select
            v-if="rule.trigger.type === 'game'"
            :model-value="rule.trigger.key"
            :options="gameOptions"
            @update:model-value="setGame"
          />
          <Select
            v-else-if="rule.trigger.type === 'app'"
            :model-value="rule.trigger.path"
            :options="appOptions"
            :placeholder="t('auto.pickApp')"
            @update:model-value="setApp"
          />
          <div v-else-if="rule.trigger.type === 'time'" class="flex flex-wrap items-center gap-2">
            <span class="type-caption text-label-2">{{ t("auto.from") }}</span>
            <div class="w-[80px]"><TextField v-model="rule.trigger.from" placeholder="22:00" /></div>
            <span class="type-caption text-label-2">{{ t("auto.to") }}</span>
            <div class="w-[80px]"><TextField v-model="rule.trigger.to" placeholder="07:00" /></div>
            <div class="flex gap-0.5">
              <button
                v-for="d in DAYS"
                :key="d.d"
                type="button"
                class="type-caption grid size-6 place-items-center rounded-full font-medium"
                :class="!rule.trigger.days.length || rule.trigger.days.includes(d.d) ? 'bg-label text-notch' : 'bg-fill-2 text-label-3'"
                @click="toggleDay(d.d)"
              >{{ d.label }}</button>
            </div>
            <span v-if="!validTime(rule.trigger.from) || !validTime(rule.trigger.to)" class="type-caption text-sys-red">{{ t("auto.badTime") }}</span>
          </div>

          <div class="type-caption mt-1 font-medium uppercase text-label-3">{{ rule.trigger.type === "start" ? t("auto.then") : t("auto.during") }}</div>
          <div v-for="(a, i) in rule.actions" :key="i" class="flex items-center gap-2">
            <span class="type-footnote w-[120px] shrink-0 truncate text-label-2">{{ t(`auto.action.${a.type}`) }}</span>
            <div class="min-w-0 flex-1">
              <Slider
                v-if="a.type === 'volume'"
                :model-value="[Math.round(a.level * 100)]"
                :min="0"
                :max="100"
                @update:model-value="(v) => v && (a.level = v[0] / 100)"
              />
              <Select v-else-if="a.type === 'output'" :model-value="a.id" :options="outputOptions" @update:model-value="pickOutput(a, $event)" />
              <Select v-else-if="a.type === 'launch'" :model-value="a.path" :options="launchOptions" :placeholder="t('auto.pickApp')" @update:model-value="pickLaunch(a, $event)" />
              <Select v-else-if="a.type === 'close'" :model-value="a.exe" :options="closeOptions" :placeholder="t('auto.pickApp')" @update:model-value="pickClose(a, $event)" />
              <div v-else-if="a.type === 'timer'" class="w-[80px]">
                <TextField :model-value="String(a.minutes)" inputmode="numeric" @update:model-value="a.minutes = Math.min(180, Math.max(1, Number($event) || 25))" />
              </div>
              <Select v-else-if="a.type === 'tool'" v-model="a.id" :options="toolOptions" />
              <TextField v-else-if="a.type === 'notify'" v-model="a.text" :placeholder="t('auto.notifyText')" />
              <Segmented
                v-else
                v-model="a.on"
                size="sm"
                :options="[
                  { value: true, label: t(`auto.action.${a.type}.on`) },
                  { value: false, label: t(`auto.action.${a.type}.off`) },
                ]"
              />
            </div>
            <span v-if="REVERSIBLE.has(a.type) && rule.trigger.type !== 'start'" class="type-caption shrink-0 text-label-4" :title="t('auto.restoredHelp')">↺</span>
            <Button size="icon-xs" variant="ghost" class="text-label-3" :title="t('auto.removeAction')" @click="rule.actions.splice(i, 1)"><PhX weight="bold" /></Button>
          </div>
          <div class="flex items-center gap-2">
            <div class="w-[220px]">
              <Select v-model="addChoice" :options="addOptions" :placeholder="t('auto.addAction')" @update:model-value="addAction" />
            </div>
            <Button class="ml-auto" size="sm" variant="secondary" :loading="testing" :disabled="!rule.actions.length" @click="test">
              <PhPlay weight="fill" />{{ t("auto.test") }}
            </Button>
            <Button size="sm" variant="default" @click="editing = null">{{ t("auto.done") }}</Button>
          </div>
          <p v-if="rule.trigger.type !== 'start'" class="type-caption text-label-3">{{ t("auto.restoredHelp") }}</p>
        </div>
      </div>
      <div class="flex flex-wrap items-center gap-1.5 border-t border-separator px-3 py-2">
        <Button size="sm" variant="secondary" @click="add()"><PhPlus weight="bold" />{{ t("auto.add") }}</Button>
        <span class="type-caption ml-1 text-label-3">{{ t("auto.templates") }}</span>
        <Button v-for="tp in TEMPLATES" :key="tp.id" size="sm" variant="ghost" @click="add(JSON.parse(JSON.stringify(tp.rule)))">{{ tp.rule.name }}</Button>
      </div>
    </Group>

    <Group v-if="automations.log.length" :title="t('auto.log')">
      <Row v-for="(l, i) in automations.log.slice(0, 8)" :key="i" :label="l.rule" :help="l.error">
        <span class="type-caption text-label-3">{{ t(`auto.event.${l.event}`) }} · {{ logTime(l.at) }}</span>
      </Row>
    </Group>
  </div>
</template>
