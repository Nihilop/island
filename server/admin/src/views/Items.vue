<script setup lang="ts">
// Éléments de tous statuts : recherche (nom, auteur, id), filtre, masquer / rétablir.
import { ref, watch } from "vue";
import { api, date } from "../api";
import { PAGE_SIZE, usePaged } from "../paged";
import Btn from "../Btn.vue";
import Pager from "../Pager.vue";

interface Item {
  id: string;
  kind: string;
  name: string;
  status: string;
  owner: string;
  ownerBanned: boolean;
  downloads: number;
  versions: number;
  openReports: number;
  updatedAt: string;
  /** URL de la vignette (CDN en production). */
  thumbnail: string | null;
}

const emit = defineEmits<{ changed: [] }>();
const status = ref("");
const typed = ref("");
const q = ref("");
let timer: number | undefined;
watch(typed, (v) => {
  window.clearTimeout(timer);
  timer = window.setTimeout(() => (q.value = v.trim()), 300);
});
const { data, page, loading, error, busy, act } = usePaged<Item>(
  () => `/v1/mod/items?status=${encodeURIComponent(status.value)}&q=${encodeURIComponent(q.value)}`,
);

const STATUS: Record<string, { label: string; cls: string }> = {
  published: { label: "publié", cls: "green" },
  hidden: { label: "masqué", cls: "orange" },
  removed: { label: "retiré", cls: "" },
};

const toggle = (i: Item) =>
  act(i.id, async () => {
    await api("POST", `/v1/mod/items/${i.id}/${i.status === "hidden" ? "restore" : "hide"}`);
    emit("changed");
  });
</script>

<template>
  <section>
    <div class="toolbar">
      <input v-model="typed" placeholder="Nom, auteur ou id" style="width: 260px" />
      <select v-model="status">
        <option value="">Tous</option>
        <option value="published">Publiés</option>
        <option value="hidden">Masqués</option>
        <option value="removed">Retirés</option>
      </select>
      <span v-if="loading" class="spinner muted" />
    </div>
    <div v-if="error" class="error">{{ error }}</div>
    <div v-if="data && !data.items.length" class="empty">Aucun élément.</div>
    <div v-else-if="data" class="table-wrap">
      <table>
        <thead>
          <tr>
            <th />
            <th>Élément</th>
            <th>Auteur</th>
            <th>Statut</th>
            <th>Versions</th>
            <th>Téléch.</th>
            <th>Mis à jour</th>
            <th />
          </tr>
        </thead>
        <tbody>
          <tr v-for="i in data.items" :key="i.id">
            <td style="width: 72px">
              <img
                v-if="i.thumbnail"
                :src="i.thumbnail"
                onerror="this.style.visibility = 'hidden'"
                alt=""
                loading="lazy"
                style="width: 64px; height: 36px; object-fit: cover; border-radius: 4px; background: var(--line)"
              />
            </td>
            <td>
              <b>{{ i.name }}</b> <span class="badge">{{ i.kind === "scene" ? "scène" : "extension" }}</span>
              <div class="muted small">
                {{ i.id }}<template v-if="i.openReports"> · <span style="color: var(--red)">{{ i.openReports }} signalement(s)</span></template>
              </div>
            </td>
            <td>{{ i.owner }} <span v-if="i.ownerBanned" class="badge red">banni</span></td>
            <td>
              <span class="badge" :class="STATUS[i.status]?.cls">{{ STATUS[i.status]?.label ?? i.status }}</span>
            </td>
            <td>{{ i.versions }}</td>
            <td>{{ i.downloads }}</td>
            <td class="small">{{ date(i.updatedAt) }}</td>
            <td>
              <Btn
                v-if="i.status !== 'removed'"
                :variant="i.status === 'hidden' ? undefined : 'danger'"
                :loading="busy === i.id"
                :disabled="!!busy"
                @click="toggle(i)"
              >
                {{ i.status === "hidden" ? "Rétablir" : "Masquer" }}
              </Btn>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
    <Pager v-if="data" :page="page" :total="data.total" :size="PAGE_SIZE" @go="page = $event" />
  </section>
</template>
