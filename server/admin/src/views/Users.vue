<script setup lang="ts">
// Comptes (admin) : recherche, filtre par rôle, ajout d'un modérateur par son nom GitHub,
// retrait, bannissement / rétablissement. L'admin n'est ni modifiable ni bannissable.
import { ref, watch } from "vue";
import { api, date, message } from "../api";
import { PAGE_SIZE, usePaged } from "../paged";
import Btn from "../Btn.vue";
import Pager from "../Pager.vue";

interface Account {
  id: number;
  githubId: number | null;
  login: string;
  avatarUrl: string | null;
  role: "user" | "moderator" | "admin";
  banned: boolean;
  items: number;
  createdAt: string;
  lastSeenAt: string | null;
}

const props = defineProps<{ me: number }>();
const role = ref("");
const typed = ref("");
const q = ref("");
let timer: number | undefined;
watch(typed, (v) => {
  window.clearTimeout(timer);
  timer = window.setTimeout(() => (q.value = v.trim()), 300);
});
const { data, page, loading, error, busy, act, load } = usePaged<Account>(
  () => `/v1/admin/users?role=${encodeURIComponent(role.value)}&q=${encodeURIComponent(q.value)}`,
);

// Ajout d'un modérateur.
const login = ref("");
const adding = ref(false);
const addError = ref<string | null>(null);
async function addModerator() {
  const l = login.value.trim();
  if (!l || adding.value) return;
  adding.value = true;
  addError.value = null;
  try {
    await api("POST", "/v1/admin/moderators", { login: l });
    login.value = "";
    await load();
  } catch (e) {
    addError.value = message(e);
  } finally {
    adding.value = false;
  }
}

const ROLE: Record<string, { label: string; cls: string }> = {
  admin: { label: "admin", cls: "blue" },
  moderator: { label: "modérateur", cls: "green" },
  user: { label: "utilisateur", cls: "" },
};

const demote = (a: Account) => act(`${a.id}:demote`, () => api("DELETE", `/v1/admin/moderators/${a.id}`));
function ban(a: Account) {
  if (!a.banned && !window.confirm(`Bannir ${a.login} ? Ses jetons sont révoqués et ses éléments disparaissent du catalogue.`)) return;
  return act(`${a.id}:ban`, () => api("POST", `/v1/admin/users/${a.id}/${a.banned ? "unban" : "ban"}`));
}
const editable = (a: Account) => a.role !== "admin" && a.id !== props.me;
</script>

<template>
  <section>
    <form class="toolbar" @submit.prevent="addModerator">
      <input v-model="login" placeholder="Nom GitHub du nouveau modérateur" style="width: 280px" :disabled="adding" />
      <Btn variant="primary" :loading="adding" :disabled="!login.trim()" @click="addModerator">Ajouter un modérateur</Btn>
      <span v-if="addError" style="color: var(--red)">{{ addError }}</span>
    </form>
    <div class="toolbar">
      <input v-model="typed" placeholder="Rechercher un compte" style="width: 240px" />
      <select v-model="role">
        <option value="">Tous les rôles</option>
        <option value="admin">Admin</option>
        <option value="moderator">Modérateurs</option>
        <option value="user">Utilisateurs</option>
        <option value="banned">Bannis</option>
      </select>
      <span v-if="loading" class="spinner muted" />
    </div>
    <div v-if="error" class="error">{{ error }}</div>
    <div v-if="data && !data.items.length" class="empty">Aucun compte.</div>
    <div v-else-if="data" class="table-wrap">
      <table>
        <thead>
          <tr>
            <th>Compte</th>
            <th>Rôle</th>
            <th>Éléments</th>
            <th>Inscrit</th>
            <th>Vu</th>
            <th />
          </tr>
        </thead>
        <tbody>
          <tr v-for="a in data.items" :key="a.id">
            <td>
              <div class="row">
                <img v-if="a.avatarUrl" :src="a.avatarUrl" alt="" class="avatar" />
                <span v-else class="avatar" />
                <div>
                  <b>{{ a.login }}</b>
                  <div class="muted small">{{ a.githubId ? `GitHub ${a.githubId}` : "compte local" }}</div>
                </div>
              </div>
            </td>
            <td>
              <span class="badge" :class="ROLE[a.role]?.cls">{{ ROLE[a.role]?.label ?? a.role }}</span>
              <span v-if="a.banned" class="badge red" style="margin-left: 4px">banni</span>
            </td>
            <td>{{ a.items }}</td>
            <td class="small">{{ date(a.createdAt) }}</td>
            <td class="small">{{ date(a.lastSeenAt) }}</td>
            <td>
              <div v-if="editable(a)" class="row">
                <Btn v-if="a.role === 'moderator'" :loading="busy === `${a.id}:demote`" :disabled="!!busy" @click="demote(a)">Retirer modérateur</Btn>
                <Btn :variant="a.banned ? undefined : 'danger'" :loading="busy === `${a.id}:ban`" :disabled="!!busy" @click="ban(a)">
                  {{ a.banned ? "Rétablir" : "Bannir" }}
                </Btn>
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
    <Pager v-if="data" :page="page" :total="data.total" :size="PAGE_SIZE" @go="page = $event" />
  </section>
</template>
