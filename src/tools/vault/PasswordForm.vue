<script setup lang="ts">
// Formulaire mot de passe (ajout / modification) : nom, identifiant, mot de passe +
// générateur (longueur 8–64, chiffres, symboles), jauge de force. Modifier une option ne
// régénère que si le mot de passe actuel a été généré (un mot de passe tapé reste).
import { clearDrafts, useDraft } from "../../stores/drafts";
import { computed, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { PhArrowsClockwise, PhTrash } from "@phosphor-icons/vue";
import Group from "../../ui/Group.vue";
import Row from "../../ui/Row.vue";
import TextField from "../../ui/TextField.vue";
import { Button } from "@/sdk/ui/button";
import { Switch } from "@/sdk/ui/switch";
import TrackBar from "../../ui/TrackBar.vue";
import { asBusy, editing, guarded, reveal, screen } from "./store";
import { DEFAULT_GEN, generate, strength, type GenOptions } from "./generator";
import { settings } from "../../notch/settings";
import { t } from "../../notch/i18n";

// Brouillon par élément (ou « new ») : survit à un changement d'onglet / une fermeture du
// notch ; effacé à l'enregistrement, à l'annulation et au verrouillage du coffre.
const base = `vault.pw.${editing.value?.id ?? "new"}.`;
const fresh = !useDraft(base + "started", () => false).value;
useDraft(base + "started", () => true).value = true;
const name = useDraft(base + "name", () => editing.value?.name ?? "");
const username = useDraft(base + "username", () => editing.value?.username ?? "");
const uri = useDraft(base + "uri", () => "");
const password = useDraft(base + "password", () => "");
const generated = useDraft(base + "generated", () => false);
const busy = ref(false);
const confirmDelete = ref(false);
let confirmTimer: number | undefined;

const gen = computed<GenOptions>(() => settings.passwordGen ?? DEFAULT_GEN);

onMounted(async () => {
  if (editing.value && fresh) {
    const pw = await reveal(editing.value.id);
    if (pw === undefined) return;
    password.value = pw;
  }
});

function regenerate() {
  password.value = generate(gen.value);
  generated.value = true;
}

watch(
  gen,
  () => {
    if (generated.value) regenerate();
  },
  { deep: true },
);

function setGen(patch: Partial<GenOptions>) {
  settings.passwordGen = { ...gen.value, ...patch };
}

const s = computed(() => strength(password.value));
const METER = { strong: "bg-sys-green", medium: "bg-sys-orange", weak: "bg-sys-red" } as const;
const canSave = computed(() => !!name.value.trim() && !!password.value && !busy.value);

async function save() {
  if (!canSave.value) return;
  busy.value = true;
  const ok = await guarded(async () => {
    if (editing.value)
      await invoke("vault_edit", { id: editing.value.id, name: name.value, username: username.value, password: password.value });
    else await invoke("vault_add", { name: name.value, username: username.value, password: password.value, uri: uri.value });
    return true;
  });
  busy.value = false;
  if (ok) {
    clearDrafts("vault.pw.");
    editing.value = null;
    screen.value = "list";
  }
}

async function remove() {
  if (!editing.value) return;
  if (!confirmDelete.value) {
    confirmDelete.value = true;
    confirmTimer = window.setTimeout(() => (confirmDelete.value = false), 3000);
    return;
  }
  window.clearTimeout(confirmTimer);
  const id = editing.value.id;
  const ok = await asBusy("delete", () => guarded(() => invoke("vault_delete", { id }).then(() => true)));
  if (ok) {
    clearDrafts("vault.pw.");
    editing.value = null;
    screen.value = "list";
  }
}

function cancel() {
  clearDrafts("vault.pw.");
  editing.value = null;
  screen.value = "list";
}
</script>

<template>
  <div class="flex flex-col gap-2 overflow-y-auto">
    <Group>
      <Row :label="t('vault.form.name')">
        <TextField v-model="name" :placeholder="t('vault.form.namePh')" :autofocus="!editing" class="w-[300px]" @enter="save" />
      </Row>
      <Row :label="t('vault.form.username')">
        <TextField v-model="username" autocomplete="username" class="w-[300px]" @enter="save" />
      </Row>
      <Row v-if="!editing" :label="t('vault.form.uri')">
        <TextField v-model="uri" placeholder="https://" class="w-[300px]" @enter="save" />
      </Row>
      <Row :label="t('vault.form.password')">
        <div class="flex items-center gap-1">
          <TextField v-model="password" secret :reveal="generated" class="w-[266px]" @enter="save" @update:model-value="generated = false" />
          <Button size="icon" class="text-label-2" :title="t('vault.form.generate')" @click="regenerate">
            <PhArrowsClockwise :size="14" weight="bold" />
          </Button>
        </div>
      </Row>
      <Row :label="t('vault.form.length')">
        <TrackBar class="w-[160px]" :value="(gen.length - 8) / 56" follow @drag="setGen({ length: Math.round(8 + $event * 56) })" @commit="setGen({ length: Math.round(8 + $event * 56) })" />
        <span class="type-caption tabular w-6 text-right text-label-2">{{ gen.length }}</span>
        <span class="type-caption text-label-3">{{ t("vault.form.digits") }}</span>
        <Switch :model-value="gen.digits" @update:model-value="setGen({ digits: $event })" />
        <span class="type-caption text-label-3">{{ t("vault.form.symbols") }}</span>
        <Switch :model-value="gen.symbols" @update:model-value="setGen({ symbols: $event })" />
      </Row>
    </Group>

    <div class="flex items-center gap-2">
      <div v-if="password" class="flex items-center gap-2">
        <div class="h-1 w-14 overflow-hidden rounded-full bg-fill-2"><div class="h-full origin-left transition-transform duration-360 ease-out" :class="METER[s.level]" :style="{ transform: `scaleX(${Math.min(1, s.bits / 100)})` }" /></div>
        <span class="type-caption text-label-3">{{ t(`vault.strength.${s.level}`) }}</span>
      </div>
      <div class="ml-auto flex gap-2">
        <Button
          v-if="editing"
          variant="ghost"
          class="gap-1 px-2 font-normal transition-[background-color,color] duration-160 hover:bg-transparent hover:text-label-3 active:scale-100"
          :class="confirmDelete && 'bg-sys-red text-white hover:bg-sys-red hover:text-white'"
          @click="remove"
        >
          <PhTrash :size="14" weight="bold" />
          <span v-if="confirmDelete" class="type-caption">{{ t("vault.form.confirmDelete") }}</span>
        </Button>
        <Button :disabled="busy" @click="cancel">{{ t("common.cancel") }}</Button>
        <Button variant="default" :disabled="!canSave && !busy" :loading="busy" @click="save">{{ editing ? t("common.save") : t("vault.add") }}</Button>
      </div>
    </div>
  </div>
</template>
