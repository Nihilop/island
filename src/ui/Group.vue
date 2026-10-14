<script setup lang="ts">
// Groupe de réglages : titre en petites capitales au-dessus d'une boîte fill-1 rayon 12.
// `section` sert d'ancre aux liens profonds (« réglages + section »).
defineProps<{ title?: string; section?: string; note?: string }>();
</script>

<template>
  <section :data-section="section" class="[&+&]:mt-[18px]">
    <h3 v-if="title" class="type-caption mb-1.5 px-3 font-medium uppercase text-label-3">{{ title }}</h3>
    <div class="rounded-md bg-fill-1"><slot /></div>
    <p v-if="note" class="type-caption mt-1.5 px-3 text-label-3">{{ note }}</p>
  </section>
</template>
