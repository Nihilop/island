<script setup lang="ts">
// Réglages › Outils › Coffre : compte connecté, coffre web, déconnexion (le délai de
// verrouillage et Windows Hello sont dans le groupe « Verrouillage »).
import { computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Group from "../../ui/Group.vue";
import Row from "../../ui/Row.vue";
import { Button } from "@/sdk/ui/button";
import { busy, logout, refreshStatus, vault } from "./store";
import { t } from "../../notch/i18n";

onMounted(refreshStatus);

const connected = computed(() => vault.state === "locked" || vault.state === "unlocked");
const webVault = computed(() => vault.server || "https://vault.bitwarden.com");
</script>

<template>
  <Group :title="t('vault.settings')" section="vault" :note="t('vault.settings.note')">
    <Row :label="t('vault.settings.account')" :help="connected ? (vault.email ?? '') : t('vault.settings.notConnected')">
      <Button @click="invoke('open_url', { url: webVault })">{{ t("vault.settings.web") }}</Button>
      <Button v-if="connected" :loading="busy === 'logout'" @click="logout">{{ t("vault.logout") }}</Button>
    </Row>
  </Group>
</template>
