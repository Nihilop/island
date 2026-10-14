<script setup lang="ts">
// Vue provisoire d'un outil pas encore porté en v2.
import { computed } from "vue";
import { tools } from "./registry";
import { activeTool } from "../notch/state";
import Empty from "../ui/Empty.vue";
import { t } from "../notch/i18n";

const tool = computed(() => tools.value.find((x) => x.id === activeTool.value));
</script>

<template>
  <Empty :icon="tool?.icon" :title="t('notch.soon.title')" :text="t('notch.soon.text')" />
</template>
