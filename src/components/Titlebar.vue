<script setup lang="ts">
import { Maximize, Minimize, X } from '@lucide/vue';
import { Button } from '@island/sdk'
import { getCurrentWindow  } from '@tauri-apps/api/window';

const appWindow = getCurrentWindow();

// Fonctions pour contrôler la fenêtre Tauri
const minimizeWindow = () => {
  appWindow.minimize();
};

const toggleMaximize = async () => {
  appWindow.toggleMaximize();
};

// On CACHE (pas close → ça détruirait la fenêtre et open_settings ne la
// retrouverait plus). La fenêtre persiste et se rouvre via le launcher → Réglages.
const closeWindow = () => {
  appWindow.hide();
};
</script>

<template>
  <div data-tauri-drag-region>
    <div class="flex items-center gap-0">
      <Button 
        @click="minimizeWindow"
        aria-label="Minimiser"
        variant="ghost"
        size="icon"
        class="[&>svg]:size-3.5!"
      >
        <Minimize />
      </Button>
      <Button 
        @click="toggleMaximize"
        aria-label="Maximiser"
        variant="ghost"
        size="icon"
        class="[&>svg]:size-3.5!"
      >
        <Maximize />
      </Button>
      <Button 
        @click="closeWindow"
        aria-label="Fermer"
        variant="ghost"
        size="icon"
        class="[&>svg]:size-3.5!"
      >
        <X />
      </Button>
    </div>
  </div>
</template>