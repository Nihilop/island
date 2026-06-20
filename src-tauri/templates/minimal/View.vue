<script setup lang="ts">
// Surface "view" de __EXT_NAME__. Édite ce fichier pour construire ton extension.
import { useIsland } from "@island/sdk";

const island = useIsland();
</script>

<template>
  <div class="flex h-full w-full flex-col items-center justify-center gap-2 p-3 text-foreground">
    <div class="text-[14px] font-semibold">__EXT_NAME__</div>
    <p class="text-center text-[12px] text-muted-foreground">Édite <code>View.vue</code> pour commencer.</p>
    <button
      class="mt-1 rounded-lg bg-primary px-3 py-1.5 text-[12px] font-medium text-primary-foreground transition hover:opacity-90"
      @click.stop="island.view.close()"
    >Fermer</button>
  </div>
</template>
