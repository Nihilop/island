<script setup lang="ts">
// Signalements : ouverts ou traités. « Classer » clôt ce signalement ; « Masquer l'élément »
// le retire du catalogue et clôt tous ses signalements ouverts.
import { ref } from "vue";
import { api, date, REASONS } from "../api";
import { PAGE_SIZE, usePaged } from "../paged";
import Btn from "../Btn.vue";
import Pager from "../Pager.vue";

interface Report {
  id: number;
  itemId: string;
  itemName: string;
  itemKind: string;
  itemStatus: string;
  owner: string;
  reason: string;
  details: string;
  reporter: string | null;
  createdAt: string;
  resolvedBy: string | null;
  resolvedAt: string | null;
  openForItem: number;
}

const emit = defineEmits<{ changed: [] }>();
const status = ref<"open" | "resolved">("open");
const { data, page, loading, error, busy, act } = usePaged<Report>(() => `/v1/mod/reports?status=${status.value}`);

const resolve = (r: Report, action: "dismiss" | "hide") =>
  act(`${r.id}:${action}`, async () => {
    await api("POST", `/v1/mod/reports/${r.id}/resolve`, { action });
    emit("changed");
  });
</script>

<template>
  <section>
    <div class="toolbar">
      <select v-model="status">
        <option value="open">Ouverts</option>
        <option value="resolved">Traités</option>
      </select>
      <span v-if="loading" class="spinner muted" />
    </div>
    <div v-if="error" class="error">{{ error }}</div>
    <div v-if="data && !data.items.length" class="empty">{{ status === "open" ? "Aucun signalement ouvert." : "Aucun signalement traité." }}</div>
    <div v-else-if="data" class="table-wrap">
      <table>
        <thead>
          <tr>
            <th>Élément</th>
            <th>Motif</th>
            <th>Signalé par</th>
            <th>Date</th>
            <th v-if="status === 'open'" />
            <th v-else>Traité par</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="r in data.items" :key="r.id">
            <td>
              <div>
                <b>{{ r.itemName }}</b>
                <span class="badge" style="margin-left: 6px">{{ r.itemKind === "scene" ? "scène" : "extension" }}</span>
                <span v-if="r.itemStatus !== 'published'" class="badge orange" style="margin-left: 4px">{{ r.itemStatus === "hidden" ? "masqué" : "retiré" }}</span>
              </div>
              <div class="muted small">
                {{ r.itemId }} · par {{ r.owner }}<template v-if="r.openForItem > 1"> · {{ r.openForItem }} signalements ouverts</template>
              </div>
            </td>
            <td>
              <span class="badge" :class="r.reason === 'illegal' || r.reason === 'malware' ? 'red' : ''">{{ REASONS[r.reason] ?? r.reason }}</span>
              <div v-if="r.details" class="small" style="margin-top: 4px; max-width: 360px; white-space: pre-wrap">{{ r.details }}</div>
            </td>
            <td>{{ r.reporter ?? "anonyme" }}</td>
            <td class="small">{{ date(r.createdAt) }}</td>
            <td v-if="status === 'open'">
              <div class="row">
                <Btn :loading="busy === `${r.id}:dismiss`" :disabled="!!busy" @click="resolve(r, 'dismiss')">Classer</Btn>
                <Btn variant="danger" :loading="busy === `${r.id}:hide`" :disabled="!!busy" @click="resolve(r, 'hide')">Masquer l'élément</Btn>
              </div>
            </td>
            <td v-else class="small">
              {{ r.resolvedBy ?? "—" }}<br /><span class="muted">{{ date(r.resolvedAt) }}</span>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
    <Pager v-if="data" :page="page" :total="data.total" :size="PAGE_SIZE" @go="page = $event" />
  </section>
</template>
