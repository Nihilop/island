<script setup lang="ts">
// Bouton d'action d'un élément du magasin, selon ce qui est installé : Obtenir / Mettre à jour
// / Appliquer (scène) / Installée (extension). Chargement local, pas de double clic.
import { computed } from "vue";
import { PhCheck, PhDownloadSimple, PhArrowsClockwise, PhMonitor } from "@phosphor-icons/vue";
import { Button } from "@/sdk/ui/button";
import { Spinner } from "@/sdk/ui/spinner";
import { apply, get, local, stateOf } from "./local";
import type { Card } from "../market/api";
import { t } from "../notch/i18n";

const props = withDefaults(defineProps<{ item: Pick<Card, "id" | "kind">; size?: "sm" | "default" }>(), { size: "sm" });
const state = computed(() => stateOf(props.item));
const busy = computed(() => local.busy === props.item.id);
</script>

<template>
  <Button v-if="state === 'update'" :size="size" variant="default" :disabled="!!local.busy" @click.stop="get(item)">
    <Spinner v-if="busy" /><PhArrowsClockwise v-else weight="bold" />{{ t("store.action.update") }}
  </Button>
  <Button v-else-if="state === 'installed' && item.kind === 'scene'" :size="size" :disabled="!!local.busy" @click.stop="apply(item.id)">
    <Spinner v-if="busy" /><PhMonitor v-else weight="bold" />{{ t("wall.apply") }}
  </Button>
  <span v-else-if="state === 'installed'" class="type-caption flex items-center gap-1 px-2 text-sys-green"><PhCheck weight="bold" />{{ t("market.installed") }}</span>
  <Button v-else :size="size" variant="secondary" :disabled="!!local.busy" @click.stop="get(item)">
    <Spinner v-if="busy" /><PhDownloadSimple v-else weight="bold" />{{ t("store.action.get") }}
  </Button>
</template>
