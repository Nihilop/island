<script setup lang="ts">
// Surface "view" de __EXT_NAME__ : compteur persistant + notif + accès aux réglages.
import { useIsland } from "@island/sdk";
import { state, bump, reset } from "./store";
import Config from "./Config.vue";

const island = useIsland();

// Ouvre la surface "config" comme une modal centrée (grand island).
function openConfig() {
  island.openModal({ title: "__EXT_NAME__", subtitle: "Réglages", component: Config });
}
</script>

<template>
  <div class="flex h-full w-full flex-col gap-3 p-3.5 text-foreground">
    <div class="flex items-center justify-between">
      <span class="text-[14px] font-semibold">__EXT_NAME__</span>
      <div class="flex items-center gap-1.5">
        <button class="grid h-7 w-7 place-items-center rounded-lg text-muted-foreground transition hover:bg-foreground/10 hover:text-foreground" @click.stop="openConfig" aria-label="Réglages">
          <svg viewBox="0 0 24 24" class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
        </button>
        <button class="grid h-7 w-7 place-items-center rounded-lg text-muted-foreground transition hover:bg-foreground/10 hover:text-foreground" @click.stop="island.view.close()" aria-label="Fermer">✕</button>
      </div>
    </div>

    <div class="flex flex-1 flex-col items-center justify-center gap-1">
      <div class="text-[44px] font-bold tabular-nums leading-none">{{ state.count }}</div>
      <div class="text-[12px] text-muted-foreground">incréments (persistés)</div>
    </div>

    <div class="flex gap-2">
      <button class="flex-1 rounded-lg bg-primary px-3 py-2 text-[13px] font-medium text-primary-foreground transition hover:opacity-90" @click.stop="bump">Incrémenter</button>
      <button class="rounded-lg border px-3 py-2 text-[13px] transition hover:bg-foreground/10" @click.stop="reset">Réinitialiser</button>
    </div>
  </div>
</template>
