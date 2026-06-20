<script setup lang="ts">
import { ref, computed } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { emit } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@island/sdk";

type Step = "form" | "creating" | "done" | "error";
type Tpl = "complete" | "minimal";

const step = ref<Step>("form");
const template = ref<Tpl>("complete");
const name = ref("");
const errorMsg = ref("");
const createdDir = ref("");
const createdId = ref("");

// Aperçu de l'id généré (doit matcher slugify() côté Rust : alphanum minuscules).
const slug = computed(() => name.value.toLowerCase().replace(/[^a-z0-9]/g, ""));
const previewId = computed(() => (slug.value ? `com.island.${slug.value}` : "com.island.…"));
const canCreate = computed(() => slug.value.length > 0);

async function close() {
  await getCurrentWindow().hide();
  step.value = "form";
  name.value = "";
  errorMsg.value = "";
  template.value = "complete";
}

async function create() {
  if (!canCreate.value) return;
  step.value = "creating";
  errorMsg.value = "";
  try {
    const res = await invoke<{ id: string; dir: string }>("scaffold_extension", {
      name: name.value,
      template: template.value,
    });
    createdId.value = res.id;
    createdDir.value = res.dir;
    step.value = "done";
    // Rafraîchit la liste des Réglages (l'extension apparaîtra une fois buildée).
    emit("ext://reload").catch(() => {});
  } catch (e) {
    errorMsg.value = String(e);
    step.value = "error";
  }
}

function reveal() {
  invoke("reveal_path", { path: createdDir.value }).catch(() => {});
}
</script>

<template>
  <div class="dark flex h-dvh w-dvw items-center justify-center bg-transparent select-none">
    <div
      class="relative flex w-[480px] flex-col overflow-hidden rounded-2xl border bg-background shadow-2xl"
      data-tauri-drag-region
    >
      <!-- Fermer -->
      <button
        class="absolute right-3.5 top-3.5 z-10 grid h-7 w-7 place-items-center rounded-full text-foreground/45 transition hover:bg-foreground/10 hover:text-foreground"
        @click="close"
        aria-label="Fermer"
      >
        <svg viewBox="0 0 24 24" class="h-4 w-4"><path fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" d="M6 6l12 12M18 6L6 18" /></svg>
      </button>

      <Transition name="step" mode="out-in">
        <!-- 1. Formulaire -->
        <div v-if="step === 'form'" key="form" class="flex flex-col p-7">
          <h2 class="text-[17px] font-semibold">Créer une extension</h2>
          <p class="mt-1 text-[12.5px] text-foreground/55">Génère un projet prêt à coder dans le dossier des extensions.</p>

          <!-- Choix du template -->
          <div class="mt-5 grid grid-cols-2 gap-3">
            <button
              type="button"
              class="rounded-xl border p-4 text-left transition"
              :class="template === 'complete' ? 'border-primary bg-primary/[0.06]' : 'hover:bg-foreground/[0.04]'"
              @click="template = 'complete'"
            >
              <div class="flex items-center gap-2">
                <span class="text-[14px] font-semibold">Complet</span>
                <span class="rounded bg-primary/15 px-1.5 py-0.5 text-[10px] font-medium text-primary">conseillé</span>
              </div>
              <p class="mt-1.5 text-[11.5px] leading-relaxed text-foreground/55">View + réglages, compteur persistant, notif et statut sur l'île.</p>
            </button>
            <button
              type="button"
              class="rounded-xl border p-4 text-left transition"
              :class="template === 'minimal' ? 'border-primary bg-primary/[0.06]' : 'hover:bg-foreground/[0.04]'"
              @click="template = 'minimal'"
            >
              <div class="text-[14px] font-semibold">Minimal</div>
              <p class="mt-1.5 text-[11.5px] leading-relaxed text-foreground/55">Le strict nécessaire : une seule view, prête à éditer.</p>
            </button>
          </div>

          <!-- Nom -->
          <label class="mt-5 block">
            <span class="text-[12px] font-medium text-foreground/60">Nom de l'extension</span>
            <input
              v-model="name"
              type="text"
              placeholder="Mon extension"
              class="mt-1.5 w-full rounded-lg border bg-foreground/[0.03] px-3 py-2 text-[13.5px] outline-none transition focus:border-primary"
              @keyup.enter="create"
            />
          </label>
          <div class="mt-1.5 text-[11.5px] text-foreground/45">
            Identifiant : <code class="text-foreground/70">{{ previewId }}</code>
          </div>

          <div class="mt-6 flex justify-end gap-2">
            <Button variant="ghost" @click="close">Annuler</Button>
            <Button :disabled="!canCreate" @click="create">Créer</Button>
          </div>
        </div>

        <!-- 2. Création -->
        <div v-else-if="step === 'creating'" key="creating" class="flex flex-col items-center justify-center gap-4 p-10 text-center">
          <span class="h-10 w-10 animate-spin rounded-full border-2 border-foreground/15 border-t-primary"></span>
          <div class="text-[13px]">Génération du projet…</div>
        </div>

        <!-- 3. Succès -->
        <div v-else-if="step === 'done'" key="done" class="flex flex-col p-7">
          <div class="flex items-center gap-3">
            <div class="grid h-11 w-11 flex-none place-items-center rounded-full bg-primary/15">
              <svg viewBox="0 0 24 24" class="h-6 w-6 text-primary"><path fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" d="m5 13 4 4 10-11"/></svg>
            </div>
            <div class="min-w-0">
              <div class="text-[16px] font-semibold">Extension créée</div>
              <div class="truncate text-[12px] text-foreground/50">{{ createdId }}</div>
            </div>
          </div>

          <div class="mt-5 rounded-lg border bg-foreground/[0.03] p-3.5">
            <div class="text-[11px] font-medium uppercase tracking-wide text-foreground/40">Pour démarrer</div>
            <pre class="mt-2 overflow-x-auto text-[12px] leading-relaxed text-foreground/75"><code>cd "{{ createdDir }}"
pnpm install
pnpm dev</code></pre>
          </div>
          <p class="mt-3 text-[12.5px] leading-relaxed text-foreground/55">
            Une fois buildée, active-la dans <span class="text-foreground/80">Réglages → Extensions</span>.
          </p>

          <div class="mt-6 flex justify-between gap-2">
            <Button variant="ghost" @click="reveal">Ouvrir le dossier</Button>
            <Button @click="close">Terminer</Button>
          </div>
        </div>

        <!-- 4. Erreur -->
        <div v-else key="error" class="flex flex-col items-center justify-center gap-4 p-9 text-center">
          <div class="grid h-14 w-14 place-items-center rounded-full bg-destructive/15">
            <svg viewBox="0 0 24 24" class="h-7 w-7 text-destructive"><path fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" d="M6 6l12 12M18 6 6 18"/></svg>
          </div>
          <div>
            <div class="text-[15px] font-semibold">Création impossible</div>
            <p class="mt-1 max-w-[300px] text-[12px] text-foreground/55">{{ errorMsg }}</p>
          </div>
          <div class="flex gap-2">
            <Button variant="ghost" @click="close">Fermer</Button>
            <Button @click="step = 'form'">Réessayer</Button>
          </div>
        </div>
      </Transition>
    </div>
  </div>
</template>

<style scoped>
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
