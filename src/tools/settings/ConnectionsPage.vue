<script setup lang="ts">
// Réglages › Connexions : Discord (Client ID + Client Secret, procédure du portail).
// Le secret n'est jamais relu : on ne sait que s'il est enregistré.
import { useDraft } from "../../stores/drafts";
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { PhCheckCircle } from "@phosphor-icons/vue";
import Group from "../../ui/Group.vue";
import Row from "../../ui/Row.vue";
import TextField from "../../ui/TextField.vue";
import { Button } from "@/sdk/ui/button";
import { settings } from "../../notch/settings";
import { discord, reconnect } from "../discord/store";
import { t } from "../../notch/i18n";

const clientId = useDraft("settings.discordClientId", () => settings.discordClientId);
const secret = ref("");
const hasSecret = ref(false);
const saved = ref(false);

onMounted(async () => (hasSecret.value = await invoke<boolean>("discord_has_secret").catch(() => false)));

const idDirty = computed(() => clientId.value.trim() !== settings.discordClientId);
const statusColor = computed(() =>
  discord.status === "ready" ? "var(--sys-green)" : discord.status === "error" ? "var(--sys-red)" : "var(--label-3)",
);

function saveId() {
  settings.discordClientId = clientId.value.trim();
}

async function saveSecret() {
  await invoke("discord_set_secret", { secret: secret.value });
  hasSecret.value = !!secret.value.trim();
  secret.value = "";
  saved.value = true;
  window.setTimeout(() => (saved.value = false), 1400);
  reconnect();
}

async function removeSecret() {
  await invoke("discord_set_secret", { secret: "" });
  hasSecret.value = false;
  reconnect();
}
</script>

<template>
  <div>
    <Group title="Discord" section="discord" :note="t('connections.note')">
      <Row :label="t('connections.status')">
        <span class="type-footnote" :style="{ color: statusColor }">{{ t(`discord.status.${discord.status}`) }}</span>
      </Row>
      <Row label="Client ID">
        <TextField v-model="clientId" class="w-[220px]" inputmode="numeric" @enter="saveId" />
        <Button v-if="idDirty" variant="default" @click="saveId">{{ t("common.save") }}</Button>
      </Row>
      <Row label="Client Secret">
        <PhCheckCircle v-if="hasSecret && !secret" :size="14" weight="fill" class="text-sys-green" />
        <TextField v-model="secret" secret class="w-[220px]" :placeholder="hasSecret ? t('connections.saved') : t('connections.paste')" @enter="saveSecret" />
        <Button v-if="secret" variant="default" @click="saveSecret">{{ saved ? "✓" : t("common.save") }}</Button>
        <Button v-else-if="hasSecret" @click="removeSecret">{{ t("connections.remove") }}</Button>
      </Row>
      <div class="type-caption border-t border-separator px-3 pt-2 pb-2.5 text-label-3">
        <ol class="flex list-decimal flex-col gap-0.5 pl-4">
          <li>{{ t("connections.discord.step1") }}</li>
          <li>{{ t("connections.discord.step2") }}</li>
          <li>{{ t("connections.discord.step3") }}</li>
          <li>{{ t("connections.discord.step4") }}</li>
        </ol>
        <button type="button" class="mt-1.5 text-sys-link" @click="invoke('open_url', { url: 'https://discord.com/developers/applications' })">
          discord.com/developers/applications
        </button>
      </div>
    </Group>
  </div>
</template>
