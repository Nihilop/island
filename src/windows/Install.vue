<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen, emit } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { Button, Switch, Progress } from "@island/sdk";

type Step = "cgu" | "details" | "progress" | "done" | "error";

interface Manifest {
  id: string;
  name?: string;
  version?: string;
  author?: string;
  description?: string;
  permissions?: string[];
}

const step = ref<Step>("cgu");
const accepted = ref(false);
const manifest = ref<Manifest | null>(null);
const path = ref<string>("");
const progress = ref(0);
const errorMsg = ref("");

// Libellés lisibles pour les permissions déclarées dans le manifeste.
const PERM_LABELS: Record<string, string> = {
  media: "Contrôle des médias (lecture, volume)",
  system: "Lecture des statistiques système (CPU, RAM)",
  storage: "Stockage local de réglages",
  idle: "Affichage d'un statut sur l'île",
  launcher: "Raccourci dans le lanceur",
  shortcuts: "Raccourcis clavier globaux",
  capture: "Capture d'écran / enregistrement vidéo",
  clipboard: "Lecture / écriture du presse-papiers",
  network: "Accès réseau",
  "native-encoder": "⚠ Exécute un programme natif (encodeur vidéo)",
  apps: "⚠ Liste et lance les applications installées",
  input: "⚠ Simule des frappes clavier dans l'application active",
  windows: "⚠ Voit les fenêtres ouvertes (titres, app) et peut les activer",
};

const name = computed(() => manifest.value?.name ?? manifest.value?.id ?? "Extension");
const perms = computed(() => manifest.value?.permissions ?? []);
const initial = computed(() => name.value.charAt(0).toUpperCase());

// La carte morphe sa taille entre les étapes (espace autour = fenêtre transparente).
const cardSize = computed(() => {
  switch (step.value) {
    case "cgu": return { width: "420px", minHeight: "300px" };
    case "details": return { width: "460px", minHeight: "380px" };
    case "progress": return { width: "380px", minHeight: "240px" };
    default: return { width: "360px", minHeight: "260px" };
  }
});

function reset() {
  step.value = "cgu";
  accepted.value = false;
  progress.value = 0;
  errorMsg.value = "";
}

async function close() {
  await getCurrentWindow().hide();
  reset();
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function doInstall() {
  step.value = "progress";
  progress.value = 0;
  // Petite progression "cosmétique" pour le ressenti d'install.
  const ticks = [12, 28, 46, 63, 81];
  for (const t of ticks) {
    progress.value = t;
    await sleep(260 + Math.random() * 220);
  }
  try {
    await invoke<string>("install_island", { path: path.value });
    progress.value = 100;
    await sleep(400);
    step.value = "done";
    // L'overlay réconcilie (chargement live arrivera avec le loader prod).
    emit("ext://reload").catch(() => {});
  } catch (e) {
    errorMsg.value = String(e);
    step.value = "error";
  }
}

function openFor(p: { manifest: Manifest; path: string }) {
  manifest.value = p.manifest;
  path.value = p.path;
  reset();
}
onMounted(async () => {
  // App déjà lancée : double-clic → l'hôte émet directement.
  listen<{ manifest: Manifest; path: string }>("install://open", (e) => openFor(e.payload));
  // Double-clic AU DÉMARRAGE : la webview n'écoutait pas encore → on récupère le pending.
  const pending = await invoke<{ manifest: Manifest; path: string } | null>("take_pending_install").catch(() => null);
  if (pending) openFor(pending);
});
</script>

<template>
  <div class="dark flex h-dvh w-dvw items-center justify-center bg-transparent select-none">
    <div
      class="relative flex flex-col overflow-hidden rounded-2xl border bg-background shadow-2xl transition-[width,min-height] duration-[450ms] ease-[cubic-bezier(0.22,1,0.36,1)]"
      :style="cardSize"
      data-tauri-drag-region
    >
      <!-- Bouton fermer -->
      <button
        class="absolute right-3.5 top-3.5 z-10 grid h-7 w-7 place-items-center rounded-full text-foreground/45 hover:bg-foreground/10 hover:text-foreground transition"
        @click="close"
        aria-label="Fermer"
      >
        <svg viewBox="0 0 24 24" class="h-4 w-4"><path fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" d="M6 6l12 12M18 6L6 18" /></svg>
      </button>

      <Transition name="step" mode="out-in">
        <!-- 1. CGU -->
        <div v-if="step === 'cgu'" key="cgu" class="flex flex-1 flex-col p-7">
          <h2 class="text-[17px] font-semibold">Conditions d'utilisation</h2>
          <p class="mt-1 text-[12.5px] text-foreground/55">Avant d'installer une extension tierce</p>
          <div class="mt-4 flex-1 overflow-y-auto rounded-lg border bg-foreground/[0.03] p-4 text-[12.5px] leading-relaxed text-foreground/70">
            Les extensions s'exécutent dans Island et peuvent accéder aux capacités
            qu'elles déclarent. N'installez que des extensions de confiance. Island
            ne peut être tenu responsable du comportement d'une extension tierce. En
            poursuivant, vous acceptez d'installer cette extension à vos risques.
          </div>
          <label class="mt-4 flex cursor-pointer items-center gap-3">
            <Switch v-model="accepted" />
            <span class="text-[13px]">J'accepte les conditions</span>
          </label>
          <div class="mt-4 flex justify-end gap-2">
            <Button variant="ghost" @click="close">Annuler</Button>
            <Button :disabled="!accepted" @click="step = 'details'">Suivant</Button>
          </div>
        </div>

        <!-- 2. Détails du manifeste -->
        <div v-else-if="step === 'details'" key="details" class="flex flex-1 flex-col p-7">
          <div class="flex items-center gap-4">
            <div class="grid h-14 w-14 flex-none place-items-center rounded-2xl bg-primary text-[22px] font-semibold text-primary-foreground">
              {{ initial }}
            </div>
            <div class="min-w-0">
              <div class="truncate text-[17px] font-semibold">{{ name }}</div>
              <div class="mt-0.5 text-[12px] text-foreground/50">
                <span v-if="manifest?.version">v{{ manifest.version }}</span>
                <span v-if="manifest?.author"> · {{ manifest.author }}</span>
              </div>
            </div>
          </div>

          <p v-if="manifest?.description" class="mt-4 text-[13px] leading-relaxed text-foreground/70">
            {{ manifest.description }}
          </p>

          <div class="mt-4 flex-1">
            <div class="text-[11px] font-medium uppercase tracking-wide text-foreground/40">Autorisations</div>
            <ul v-if="perms.length" class="mt-2 space-y-1.5">
              <li v-for="p in perms" :key="p" class="flex items-center gap-2 text-[12.5px] text-foreground/75">
                <svg viewBox="0 0 24 24" class="h-4 w-4 flex-none text-primary"><path fill="currentColor" d="M12 2 4 5v6c0 5 3.4 8.5 8 10 4.6-1.5 8-5 8-10V5z" opacity=".25"/><path fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="m9 12 2 2 4-4"/></svg>
                {{ PERM_LABELS[p] ?? p }}
              </li>
            </ul>
            <p v-else class="mt-2 text-[12.5px] text-foreground/45">Aucune autorisation particulière.</p>
          </div>

          <div class="mt-4 flex justify-between gap-2">
            <Button variant="ghost" @click="step = 'cgu'">Retour</Button>
            <Button @click="doInstall">Installer</Button>
          </div>
        </div>

        <!-- 3. Progression -->
        <div v-else-if="step === 'progress'" key="progress" class="flex flex-1 flex-col items-center justify-center gap-5 p-8 text-center">
          <div class="relative grid h-16 w-16 place-items-center">
            <span class="absolute inset-0 animate-spin rounded-full border-2 border-foreground/15 border-t-primary"></span>
            <span class="text-[20px] font-semibold">{{ initial }}</span>
          </div>
          <div class="w-full">
            <div class="mb-2 text-[13px]">Installation de {{ name }}…</div>
            <Progress :model-value="progress" />
          </div>
        </div>

        <!-- 4. Succès -->
        <div v-else-if="step === 'done'" key="done" class="flex flex-1 flex-col items-center justify-center gap-4 p-8 text-center">
          <div class="grid h-16 w-16 place-items-center rounded-full bg-primary/15">
            <svg viewBox="0 0 24 24" class="h-8 w-8 text-primary"><path fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" d="m5 13 4 4 10-11"/></svg>
          </div>
          <div>
            <div class="text-[16px] font-semibold">{{ name }} installée</div>
            <p class="mt-1 text-[12.5px] text-foreground/55">Disponible au prochain lancement d'Island.</p>
          </div>
          <Button class="mt-1" @click="close">Terminer</Button>
        </div>

        <!-- 5. Erreur -->
        <div v-else key="error" class="flex flex-1 flex-col items-center justify-center gap-4 p-8 text-center">
          <div class="grid h-16 w-16 place-items-center rounded-full bg-destructive/15">
            <svg viewBox="0 0 24 24" class="h-8 w-8 text-destructive"><path fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" d="M6 6l12 12M18 6 6 18"/></svg>
          </div>
          <div>
            <div class="text-[16px] font-semibold">Installation échouée</div>
            <p class="mt-1 max-w-[280px] text-[12px] text-foreground/55">{{ errorMsg }}</p>
          </div>
          <div class="flex gap-2">
            <Button variant="ghost" @click="close">Fermer</Button>
            <Button @click="step = 'details'">Réessayer</Button>
          </div>
        </div>
      </Transition>
    </div>
  </div>
</template>

<style scoped>
/* Classes de <Transition> Vue (appliquées par Vue → non inlinables). */
.step-enter-active,
.step-leave-active {
  transition: opacity 0.22s ease, transform 0.22s ease;
}
.step-enter-from {
  opacity: 0;
  transform: translateY(8px);
}
.step-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}
</style>
