<script setup lang="ts">
// Fenêtre d'une extension (libellé `ext-<id>__<fenêtre>`, cf. detached.rs) : barre de titre
// maison + la page déclarée (`contributes.windows`) dans une iframe isolée, reliée à l'hôte
// comme dans le notch (runtime.ts). Plein écran et lecture automatique permis. Quand la
// page passe un élément en plein écran, la fenêtre passe en plein écran Windows (et la
// barre disparaît). Méthodes propres à une fenêtre : arguments d'ouverture, titre, fermer.
import { onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { PhCornersIn, PhCornersOut, PhMinus, PhX } from "@phosphor-icons/vue";
import IconButton from "../ui/IconButton.vue";
import { addMethods, connect, emitTo, extUrl, initExtBus, parseWindowKey, sandboxFrame, type Installed, type Manifest } from "./runtime";
import { t } from "../notch/i18n";

const win = getCurrentWindow();
const key = parseWindowKey(win.label.slice("ext-".length));

const ext = ref<Manifest | null>(null);
const title = ref("");
const error = ref<string | null>(null);
const fullscreen = ref(false);
const maximized = ref(false);
let args: unknown = null;
let disconnect: (() => void) | undefined;
const unlisten: UnlistenFn[] = [];
const host = ref<HTMLElement>();

function applyTheme(v: "dark" | "light") {
  const root = document.documentElement;
  root.classList.toggle("light", v === "light");
  root.classList.toggle("dark", v === "dark");
  root.style.colorScheme = v;
  if (ext.value) emitTo(ext.value.id, "theme", v);
}

addMethods({
  "windows.args": () => args,
  "windows.setTitle": (_e, text: unknown) => {
    title.value = String(text ?? "").slice(0, 120);
    void win.setTitle(title.value);
  },
  "windows.close": () => void win.close(),
});

async function onFullscreen() {
  fullscreen.value = !!document.fullscreenElement;
  // Par Island (et non `setFullscreen`) : l'île sait alors qu'une de ses fenêtres est en
  // plein écran et se masque (réglage « Masquer en plein écran »).
  await invoke("window_fullscreen", { on: fullscreen.value }).catch(() => {});
}

onMounted(async () => {
  unlisten.push(await listen<"dark" | "light">("theme://changed", (e) => applyTheme(e.payload)));
  void emit("theme://get");
  unlisten.push(await win.onResized(async () => (maximized.value = await win.isMaximized())));
  document.addEventListener("fullscreenchange", onFullscreen);
  initExtBus();

  if (!key) return void (error.value = "invalidWindow");
  const list = await invoke<Installed[]>("ext2_list").catch(() => []);
  const m = list.find((x) => x.manifest.id === key.extId)?.manifest;
  const decl = m?.contributes.windows?.find((w) => w.id === key.winId);
  if (!m || !decl || !m.permissions.includes("windows")) return void (error.value = "notFound");
  ext.value = m;
  title.value = decl.title ?? m.name;
  args = await invoke("window_args").catch(() => null);
  // Réouverture avec d'autres arguments (autre épisode…) : la page est prévenue.
  unlisten.push(
    await listen("window://args", (e) => {
      args = e.payload;
      emitTo(m.id, "window.args", args);
    }),
  );

  const frame = sandboxFrame(extUrl(m.id, decl.view), "fullscreen; autoplay");
  frame.className = "block size-full border-0 bg-transparent";
  frame.addEventListener("load", () => {
    disconnect?.();
    disconnect = connect(m, frame);
    frame.focus();
  });
  host.value?.appendChild(frame);
});

onBeforeUnmount(() => {
  disconnect?.();
  document.removeEventListener("fullscreenchange", onFullscreen);
  unlisten.forEach((u) => u());
});
</script>

<template>
  <div class="flex h-screen flex-col overflow-hidden bg-notch text-label">
    <header
      v-show="!fullscreen"
      class="flex h-9 shrink-0 items-center gap-1 border-b border-separator pr-1 pl-3"
      data-tauri-drag-region
      @dblclick.self="win.toggleMaximize()"
    >
      <span class="type-footnote min-w-0 flex-1 truncate font-medium" data-tauri-drag-region>{{ title }}</span>
      <IconButton :icon="PhMinus" :label="t('notes.minimize')" @click="win.minimize()" />
      <IconButton :icon="maximized ? PhCornersIn : PhCornersOut" :label="t('notes.maximize')" @click="win.toggleMaximize()" />
      <IconButton :icon="PhX" :label="t('common.close')" class="hover:bg-sys-red! hover:text-white!" @click="win.close()" />
    </header>
    <div v-if="error" class="type-footnote grid flex-1 place-items-center text-label-3">{{ t("ext.window.error") }}</div>
    <div v-else ref="host" class="min-h-0 flex-1" />
  </div>
</template>
