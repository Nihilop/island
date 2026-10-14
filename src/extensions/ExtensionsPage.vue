<script setup lang="ts">
// Réglages › Extensions : extensions v2 installées (version, auteur, permissions) et leurs
// réglages générés depuis le manifeste.
import { onMounted, reactive, ref } from "vue";
import { PhPuzzlePiece } from "@phosphor-icons/vue";
import Group from "../ui/Group.vue";
import Row from "../ui/Row.vue";
import TextField from "../ui/TextField.vue";
import Select from "../ui/Select.vue";
import Empty from "../ui/Empty.vue";
import { Switch } from "@/sdk/ui/switch";
import { Button } from "@/sdk/ui/button";
import { Badge } from "@/sdk/ui/badge";
import { extUrl, installed, type Manifest, type SettingDecl } from "./runtime";
import { readSetting, writeSetting } from "./settings";
import { t } from "../notch/i18n";

/** Valeurs affichées, par `extension|clé` (les secrets ne sont jamais réaffichés). */
const values = reactive(new Map<string, unknown>());
const secretSet = reactive(new Set<string>());
const secretDraft = reactive(new Map<string, string>());
const saving = ref<string | null>(null);

const k = (ext: Manifest, key: string) => `${ext.id}|${key}`;

onMounted(async () => {
  for (const ext of installed.value) {
    for (const d of ext.contributes.settings) {
      const v = await readSetting(ext, d.key).catch(() => null);
      if (d.type === "secret") {
        if (v) secretSet.add(k(ext, d.key));
      } else values.set(k(ext, d.key), d.type === "list" && Array.isArray(v) ? v.join("\n") : v);
    }
  }
});

async function save(ext: Manifest, d: SettingDecl, value: unknown) {
  const id = k(ext, d.key);
  saving.value = id;
  try {
    const v = d.type === "list" ? String(value ?? "").split(/[\n,]/).map((s) => s.trim()).filter(Boolean) : d.type === "number" ? Number(value) : value;
    await writeSetting(ext, d.key, v);
    if (d.type === "secret") {
      if (value) secretSet.add(id);
      else secretSet.delete(id);
      secretDraft.delete(id);
    } else values.set(id, value);
  } finally {
    saving.value = null;
  }
}

/** Libellé d'une permission, lisible. */
function permLabel(p: string): string {
  if (p.startsWith("network:")) return t("ext.perm.network", { host: p.slice(8) });
  const key = `ext.perm.${p}`;
  const s = t(key);
  return s === key ? p : s;
}
</script>

<template>
  <div>
    <Empty v-if="!installed.length" :icon="PhPuzzlePiece" :title="t('ext.none')" :text="t('ext.noneHelp')" />
    <Group v-for="ext in installed" :key="ext.id" :title="ext.name" :note="ext.description ?? undefined">
      <Row :label="`${ext.name} ${ext.version}`" :help="ext.author ?? ext.id">
        <span
          v-if="ext.icon"
          class="size-5 bg-label-2 [mask-position:center] [mask-repeat:no-repeat] [mask-size:contain]"
          :style="{ maskImage: `url('${extUrl(ext.id, ext.icon)}')` }"
        />
      </Row>
      <Row v-if="ext.permissions.length" :label="t('ext.permissions')">
        <div class="flex max-w-[300px] flex-wrap justify-end gap-1">
          <Badge v-for="p in ext.permissions" :key="p" variant="secondary">{{ permLabel(p) }}</Badge>
        </div>
      </Row>
      <Row v-for="d in ext.contributes.settings" :key="d.key" :label="d.label" :help="d.help">
        <Switch
          v-if="d.type === 'switch'"
          :model-value="!!values.get(k(ext, d.key))"
          @update:model-value="save(ext, d, $event)"
        />
        <Select
          v-else-if="d.type === 'select'"
          :model-value="(values.get(k(ext, d.key)) as string) ?? ''"
          :options="d.options ?? []"
          @update:model-value="save(ext, d, $event)"
        />
        <template v-else-if="d.type === 'secret'">
          <span v-if="secretSet.has(k(ext, d.key)) && !secretDraft.has(k(ext, d.key))" class="type-caption text-label-3">{{ t("ext.secretSaved") }}</span>
          <div v-else class="w-[200px]">
            <TextField
              :model-value="secretDraft.get(k(ext, d.key)) ?? ''"
              secret
              @update:model-value="secretDraft.set(k(ext, d.key), $event)"
              @enter="save(ext, d, secretDraft.get(k(ext, d.key)))"
            />
          </div>
          <Button
            v-if="secretSet.has(k(ext, d.key)) && !secretDraft.has(k(ext, d.key))"
            :loading="saving === k(ext, d.key)"
            @click="save(ext, d, '')"
          >{{ t("ext.clear") }}</Button>
          <Button
            v-else
            variant="default"
            :disabled="!secretDraft.get(k(ext, d.key))"
            :loading="saving === k(ext, d.key)"
            @click="save(ext, d, secretDraft.get(k(ext, d.key)))"
          >{{ t("ext.save") }}</Button>
        </template>
        <div v-else class="w-[220px]">
          <TextField
            :model-value="String(values.get(k(ext, d.key)) ?? '')"
            :type="d.type === 'number' ? 'number' : 'text'"
            :placeholder="d.type === 'list' ? t('ext.listHelp') : ''"
            @update:model-value="values.set(k(ext, d.key), $event)"
            @enter="save(ext, d, values.get(k(ext, d.key)))"
            @blur="save(ext, d, values.get(k(ext, d.key)))"
          />
        </div>
      </Row>
    </Group>
  </div>
</template>
