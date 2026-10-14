<script setup lang="ts">
// Réglages › IA : Ask (fournisseur, clé, modèle, effort, base URL si Custom) et Quotas IA
// (fournisseurs affichés). Les clés vont au Gestionnaire d'identifiants, jamais relues.
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { PhCheckCircle } from "@phosphor-icons/vue";
import Group from "../../ui/Group.vue";
import Row from "../../ui/Row.vue";
import Segmented from "../../ui/Segmented.vue";
import TextField from "../../ui/TextField.vue";
import { Button } from "@/sdk/ui/button";
import { Switch } from "@/sdk/ui/switch";
import { settings } from "../../notch/settings";
import { t } from "../../notch/i18n";

const cfg = computed(() => settings.ask.byProvider[settings.ask.provider]);
const key = ref("");
const baseUrl = computed({
  get: () => cfg.value.baseUrl ?? "",
  set: (v: string) => (cfg.value.baseUrl = v),
});
const hasKey = ref(false);

async function refreshKey() {
  hasKey.value = await invoke<boolean>("ai_has_key", { provider: settings.ask.provider }).catch(() => false);
}
watch(() => settings.ask.provider, refreshKey, { immediate: true });

async function saveKey(value: string) {
  await invoke("ai_set_key", { provider: settings.ask.provider, key: value });
  key.value = "";
  await refreshKey();
}
</script>

<template>
  <div>
    <Group :title="t('tool.ask')" section="ask" :note="t('connections.note')">
      <Row :label="t('ask.providerLabel')">
        <Segmented
          v-model="settings.ask.provider"
          size="sm"
          :options="(['anthropic', 'openai', 'openrouter', 'custom'] as const).map((p) => ({ value: p, label: t(`ask.provider.${p}`) }))"
        />
      </Row>
      <Row v-if="settings.ask.provider === 'custom'" :label="t('ask.baseUrl')" :help="t('ask.baseUrlHelp')">
        <TextField v-model="baseUrl" placeholder="https://…/v1" class="w-[240px]" />
      </Row>
      <Row :label="t('ask.key')">
        <PhCheckCircle v-if="hasKey && !key" :size="14" weight="fill" class="text-sys-green" />
        <TextField v-model="key" secret class="w-[220px]" :placeholder="hasKey ? t('connections.saved') : t('connections.paste')" @enter="saveKey(key)" />
        <Button v-if="key" variant="default" @click="saveKey(key)">{{ t("common.save") }}</Button>
        <Button v-else-if="hasKey" @click="saveKey('')">{{ t("connections.remove") }}</Button>
      </Row>
      <Row :label="t('ask.model')" :help="t(`ask.modelHint.${settings.ask.provider}`)">
        <TextField v-model="cfg.model" class="w-[240px]" />
      </Row>
      <Row :label="t('ask.effort')">
        <Segmented
          v-model="cfg.effort"
          size="sm"
          :options="(['low', 'medium', 'high', 'xhigh'] as const).map((e) => ({ value: e, label: t(`ask.effort.${e}`) }))"
        />
      </Row>
    </Group>

    <Group :title="t('tool.usage')" section="usage" :note="t('usage.note')">
      <Row :label="t('usage.provider.claude')" :help="t('usage.source.claude')">
        <Switch v-model="settings.usageProviders.claude" />
      </Row>
      <Row :label="t('usage.provider.chatgpt')" :help="t('usage.source.chatgpt')">
        <Switch v-model="settings.usageProviders.chatgpt" />
      </Row>
      <Row :label="t('usage.provider.gemini')" :help="t('usage.unavailable')" />
      <Row :label="t('usage.provider.cursor')" :help="t('usage.unavailable')" />
    </Group>
  </div>
</template>
