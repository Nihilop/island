<script setup lang="ts">
import { ref, onMounted } from "vue";
import Titlebar from "../components/Titlebar.vue";
import {
  Switch, Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Button,
} from "@island/sdk";
import { discoverManifests, getEnabled, setEnabled } from "../composables/extensions";
import { useAppStore } from "@/stores/appStore.ts";
import { storeToRefs } from "pinia";
import { emit, listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";

type Tab = "general" | "extensions";
const tab = ref<Tab>("general");
// storeToRefs garde la réactivité des refs (theme/lang/autostart) ; les actions
// se destructurent directement.
const store = useAppStore();
const { theme, lang, autostart } = storeToRefs(store);
const { onAutostart, onTheme, onLang } = store;

// --- Extensions (état d'activation possédé par l'app) ---
interface Row { id: string; name: string; enabled: boolean; dev: boolean }
const rows = ref<Row[]>([]);

async function loadRows() {
  const found = await discoverManifests();
  const enabled = (await getEnabled()) ?? found.map((e) => e.id);
  rows.value = found.map((e) => ({ id: e.id, name: e.name, enabled: enabled.includes(e.id), dev: e.dev }));
}

// Packager une extension (dev) en .island : l'app zippe manifest + dist (déjà buildé).
async function packExt(id: string) {
  const out = await save({
    defaultPath: `${id}.island`,
    filters: [{ name: "Island", extensions: ["island"] }],
  });
  if (typeof out !== "string") return;
  try {
    await invoke("pack_extension", { id, outPath: out });
    await invoke("reveal_path", { path: out }).catch(() => {});
  } catch (e) {
    console.error(e);
  }
}

function onToggleExt() {
  setEnabled(rows.value.filter((r) => r.enabled).map((r) => r.id));
  // → l'overlay réconcilie en RUNTIME (load/unload, sans redémarrage).
  emit("ext://reload").catch(() => {});
}

// Associe les fichiers .island à Island dans Windows (HKCU, sans admin).
const associated = ref(false);
async function associate() {
  try {
    await invoke("register_file_association");
    associated.value = true;
  } catch (e) {
    console.error(e);
  }
}

// Ouvre la fenêtre de création d'extension (choix du template + nom).
function openCreate() {
  invoke("open_create").catch((e) => console.error(e));
}

// Déclencheur dev : choisir un .island → ouvre la modal d'installation.
async function pickAndInstall() {
  const path = await open({
    multiple: false,
    filters: [{ name: "Island", extensions: ["island"] }],
  });
  if (typeof path === "string") {
    await invoke("open_install", { path }).catch((e) => console.error(e));
  }
}

onMounted(async () => {
  await loadRows();
  // Une install (modal) émet ext://reload → on rafraîchit la liste sans recharger la page.
  await listen("ext://reload", () => loadRows());
});
</script>

<template>
  <div class="relative h-dvh w-dvw flex flex-col">
    <header class="flex justify-between items-center gap-4 py-2 px-5 border-b" data-tauri-drag-region>
      <div><span class="brand">Island</span><span class="text-[13px] opacity-55">Réglages</span></div>
      <Titlebar data-tauri-drag-region />
    </header>

    <div class="min-h-0 flex flex-1">
      <nav class="w-1/3 flex-none border-r py-4 px-3 flex flex-col gap-2">
        <Button :variant="tab === 'general' ? 'default' : 'ghost'" class="justify-start" @click="tab = 'general'">Général</Button>
        <Button :variant="tab === 'extensions' ? 'default' : 'ghost'" class="justify-start" @click="tab = 'extensions'">Extensions</Button>
      </nav>

      <main class="flex-1 overflow-y-auto py-4.5 px-6">
        <section v-if="tab === 'general'">
          <div class="flex items-center justify-between gap-4 py-3.5 border-b">
            <div>
              <div class="text-[14px]">Démarrer avec Windows</div>
              <div class="text-[12px] opacity-50 mt-0.5">Lancer Island à l'ouverture de session</div>
            </div>
            <Switch v-model="autostart" @update:model-value="onAutostart" />
          </div>
          <div class="flex items-center justify-between gap-4 py-3.5 border-b">
            <div>
              <div class="text-[14px]">Thème</div>
              <div class="text-[12px] opacity-50 mt-0.5">Apparence de la fenêtre Réglages</div>
            </div>
            <Select v-model="theme" @update:modelValue="onTheme">
              <SelectTrigger>
                <SelectValue placeholder="Select theme" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem :value="el.value"
                  v-for="el in [{ value: 'dark', label: 'Sombre' }, { value: 'light', label: 'Clair' }]">
                  {{ el.label }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div class="flex items-center justify-between gap-4 py-3.5 border-b">
            <div>
              <div class="text-[14px]">Langue</div>
              <div class="text-[12px] opacity-50 mt-0.5">Traductions à venir (i18n)</div>
            </div>

              <Select v-model="lang" @update:modelValue="onLang">
              <SelectTrigger>
                <SelectValue placeholder="Select langue" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem :value="el.value"
                  v-for="el in [{ value: 'fr', label: 'Français' }, { value: 'en', label: 'English' }]">
                  {{ el.label }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
        </section>

        <section v-else>
          <div class="flex items-center justify-between gap-4 pb-3.5 border-b">
            <div>
              <div class="text-[14px]">Créer une extension</div>
              <div class="text-[12px] opacity-50 mt-0.5">Génère un projet à partir d'un template</div>
            </div>
            <Button @click="openCreate">Nouvelle…</Button>
          </div>
          <div class="flex items-center justify-between gap-4 py-3.5 border-b">
            <div>
              <div class="text-[14px]">Installer une extension</div>
              <div class="text-[12px] opacity-50 mt-0.5">Depuis un paquet .island</div>
            </div>
            <Button variant="outline" @click="pickAndInstall">Parcourir…</Button>
          </div>
          <div class="flex items-center justify-between gap-4 py-3.5 border-b">
            <div>
              <div class="text-[14px]">Associer les fichiers .island</div>
              <div class="text-[12px] opacity-50 mt-0.5">Double-clic d'un .island → ouvre Island (debug)</div>
            </div>
            <Button variant="outline" @click="associate">{{ associated ? "✓ Associé" : "Associer" }}</Button>
          </div>
          <p v-if="!rows.length" class="text-[13px] opacity-50">Aucune extension détectée.</p>
          <div v-for="r in rows" :key="r.id" class="flex items-center justify-between gap-4 py-3.5 border-b">
            <div>
              <div class="text-[14px]">
                {{ r.name }}
                <span v-if="r.dev" class="ml-1 rounded bg-foreground/10 px-1.5 py-0.5 align-middle text-[10px] opacity-70">dev</span>
              </div>
              <div class="text-[12px] opacity-50 mt-0.5">{{ r.id }}</div>
            </div>
            <div class="flex items-center gap-3">
              <Button v-if="r.dev" variant="outline" @click="packExt(r.id)">Packager</Button>
              <Switch v-model="r.enabled" @update:model-value="onToggleExt" />
            </div>
          </div>
          <p v-if="rows.length" class="text-[13px] opacity-50 mt-4">Redémarrez Island pour appliquer les changements.</p>
        </section>
      </main>
    </div>
  </div>
</template>
