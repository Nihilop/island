<script setup lang="ts">
// Le CLI Bitwarden n'est pas encore là : explication + téléchargement (avec progression).
import { PhShieldCheck } from "@phosphor-icons/vue";
import Empty from "../../ui/Empty.vue";
import { Button } from "@/sdk/ui/button";
import { install, vault } from "./store";
import { t } from "../../notch/i18n";
</script>

<template>
  <Empty :icon="PhShieldCheck" :title="t('vault.install.title')" :text="t('vault.install.text')">
    <div v-if="vault.installing !== null" class="flex items-center gap-2">
      <div class="h-1.5 w-28 overflow-hidden rounded-full bg-fill-2"><div class="h-full origin-left bg-label-2 transition-transform duration-200 ease-out" :style="{ transform: `scaleX(${vault.installing / 100})` }" /></div>
      <span class="type-caption tabular text-label-3">{{ vault.installing >= 100 ? t("vault.install.busy") : `${vault.installing}%` }}</span>
    </div>
    <Button v-else variant="default" :loading="vault.installing !== null" @click="install">{{ t("vault.install.action") }}</Button>
  </Empty>
</template>
