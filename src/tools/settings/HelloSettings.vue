<script setup lang="ts">
// Réglages › Outils › Verrouillage : Windows Hello (configuré une fois, partagé par les
// outils), outils qui s'y branchent, délai de verrouillage automatique.
import { onMounted, ref } from "vue";
import { PhFingerprint } from "@phosphor-icons/vue";
import Group from "../../ui/Group.vue";
import Row from "../../ui/Row.vue";
import Segmented from "../../ui/Segmented.vue";
import { Button } from "@/sdk/ui/button";
import { Switch } from "@/sdk/ui/switch";
import { settings } from "../../notch/settings";
import { disableHello, enrollHello, hello, helloError, refreshHello, setScope, type HelloScope } from "../../notch/hello";
import { t } from "../../notch/i18n";

onMounted(refreshHello);

const error = ref<string | null>(null);
/** Interrupteur en attente de confirmation Hello. */
const pendingScope = ref<HelloScope | null>(null);

async function run(fn: () => Promise<unknown>) {
  error.value = null;
  try {
    await fn();
  } catch (e) {
    error.value = helloError(t, e);
  }
}

function toggle(scope: HelloScope, on: boolean) {
  if (pendingScope.value) return;
  pendingScope.value = scope;
  void run(() => setScope(scope, on)).finally(() => (pendingScope.value = null));
}

const scopes: { id: HelloScope; label: string; help: string }[] = [
  { id: "vault", label: "hello.scope.vault", help: "hello.scope.vaultHelp" },
  { id: "notes", label: "hello.scope.notes", help: "hello.scope.notesHelp" },
  { id: "budget", label: "hello.scope.budget", help: "hello.scope.budgetHelp" },
];
</script>

<template>
  <Group :title="t('hello.settings')" section="hello" :note="t('hello.settings.note')">
    <Row
      :label="t('hello.title')"
      :help="error ?? (!hello.supported ? t('hello.unsupported') : hello.enrolled ? t('hello.enrolled') : t('hello.notEnrolled'))"
    >
      <template v-if="hello.supported">
        <Button v-if="hello.enrolled" :loading="hello.busy" :disabled="hello.prompting" @click="run(disableHello)">{{ t("hello.disable") }}</Button>
        <Button v-else variant="default" :loading="hello.busy" @click="run(enrollHello)"><PhFingerprint weight="bold" />{{ t("hello.enroll") }}</Button>
      </template>
    </Row>
    <template v-if="hello.enrolled">
      <Row v-for="s in scopes" :key="s.id" :label="t(s.label)" :help="t(s.help)">
        <Switch
          :model-value="settings.hello[s.id]"
          :disabled="!!pendingScope || hello.prompting"
          @update:model-value="toggle(s.id, $event)"
        />
      </Row>
    </template>
    <Row :label="t('vault.settings.timeout')" :help="t('hello.timeoutHelp')">
      <Segmented
        v-model="settings.vaultTimeout"
        size="sm"
        :options="[
          { value: 120, label: '2 min' },
          { value: 900, label: '15 min' },
          { value: 3600, label: '1 h' },
        ]"
      />
    </Row>
  </Group>
</template>
