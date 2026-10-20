<script setup lang="ts">
// Journal (admin) : qui a fait quoi, quand.
import { date } from "../api";
import { PAGE_SIZE, usePaged } from "../paged";
import Pager from "../Pager.vue";

interface Entry {
  id: number;
  actor: string | null;
  action: string;
  target: string;
  details: Record<string, unknown>;
  createdAt: string;
}

const { data, page, loading, error } = usePaged<Entry>(() => "/v1/admin/audit");

const ACTIONS: Record<string, string> = {
  "moderator.add": "a ajouté le modérateur",
  "moderator.remove": "a retiré le modérateur",
  "user.ban": "a banni",
  "user.unban": "a rétabli",
  "item.hide": "a masqué",
  "item.restore": "a rétabli l'élément",
  "item.remove": "a retiré",
  "report.dismiss": "a classé un signalement sur",
  "report.hide": "a masqué (signalement)",
};
</script>

<template>
  <section>
    <div v-if="loading && !data" class="empty"><span class="spinner" /></div>
    <div v-if="error" class="error">{{ error }}</div>
    <div v-if="data && !data.items.length" class="empty">Journal vide.</div>
    <div v-else-if="data" class="table-wrap">
      <table>
        <thead>
          <tr>
            <th>Date</th>
            <th>Action</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="e in data.items" :key="e.id">
            <td class="small" style="width: 150px">{{ date(e.createdAt) }}</td>
            <td>
              <b>{{ e.actor ?? "?" }}</b> {{ ACTIONS[e.action] ?? e.action }} <b>{{ e.target }}</b>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
    <Pager v-if="data" :page="page" :total="data.total" :size="PAGE_SIZE" @go="page = $event" />
  </section>
</template>
