<script setup lang="ts">
// Panneau d'administration du magasin : session (cookie GitHub) vérifiée par /v1/me ;
// onglets par `#` — signalements, éléments, relecture (extensions en attente),
// comptes et journal (admin seulement). En tête : l'interrupteur d'urgence des publications
// (modérateurs et admin).
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { api, ApiError, date, message, type Me, type Publishing } from "./api";
import Btn from "./Btn.vue";
import Reports from "./views/Reports.vue";
import Items from "./views/Items.vue";
import Users from "./views/Users.vue";
import Journal from "./views/Journal.vue";
import Reviews from "./views/Reviews.vue";

type Tab = "reports" | "items" | "extensions" | "users" | "audit";

const me = ref<Me | null>(null);
const state = ref<"loading" | "anonymous" | "forbidden" | "ready" | "error">("loading");
const error = ref<string | null>(null);
const summary = ref({ openReports: 0, hiddenItems: 0, pendingVersions: 0 });

const isAdmin = computed(() => me.value?.role === "admin");
const tabs = computed(() =>
  [
    { id: "reports" as Tab, label: "Signalements", count: summary.value.openReports },
    { id: "items" as Tab, label: "Éléments", count: summary.value.hiddenItems, hint: "masqués" },
    { id: "extensions" as Tab, label: "Relecture", count: summary.value.pendingVersions, hint: "extensions à relire" },
    ...(isAdmin.value ? [{ id: "users" as Tab, label: "Comptes", count: 0 }, { id: "audit" as Tab, label: "Journal", count: 0 }] : []),
  ],
);

const tab = ref<Tab>("reports");
function readHash() {
  const h = location.hash.slice(1) as Tab;
  tab.value = tabs.value.some((t) => t.id === h) ? h : "reports";
}
function go(t: Tab) {
  location.hash = t;
}

async function refreshSummary() {
  summary.value = await api<typeof summary.value>("GET", "/v1/mod/summary").catch(() => summary.value);
}

async function init() {
  state.value = "loading";
  try {
    me.value = await api<Me>("GET", "/v1/me");
    if (me.value.role === "user") {
      state.value = "forbidden";
      return;
    }
    state.value = "ready";
    readHash();
    await Promise.all([refreshSummary(), loadPublishing()]);
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) state.value = "anonymous";
    else {
      error.value = message(e);
      state.value = "error";
    }
  }
}

const publishing = ref<Publishing | null>(null);
const switching = ref(false);
const switchError = ref<string | null>(null);
async function loadPublishing() {
  publishing.value = await api<Publishing>("GET", "/v1/mod/publishing").catch(() => publishing.value);
}
async function setPublishing(open: boolean) {
  let msg = "";
  if (!open) {
    const answer = window.prompt(
      "Suspendre TOUTES les publications (nouveaux éléments, versions, médias, modifications de fiches) ?\n\nMessage affiché aux auteurs (facultatif) :",
      "Les publications sont suspendues temporairement.",
    );
    if (answer === null) return;
    msg = answer;
  }
  switching.value = true;
  switchError.value = null;
  try {
    publishing.value = await api<Publishing>("PUT", "/v1/mod/publishing", { open, message: msg });
  } catch (e) {
    switchError.value = message(e);
  } finally {
    switching.value = false;
  }
}

const loggingOut = ref(false);
async function logout() {
  loggingOut.value = true;
  await api("POST", "/admin/logout").catch(() => {});
  location.href = "/admin";
}

onMounted(() => {
  window.addEventListener("hashchange", readHash);
  void init();
});
onBeforeUnmount(() => window.removeEventListener("hashchange", readHash));
</script>

<template>
  <div v-if="state === 'loading'" class="center"><span class="spinner" /></div>

  <div v-else-if="state === 'anonymous'" class="center">
    <h1>Magasin d'Island</h1>
    <p class="muted">Panneau réservé aux modérateurs.</p>
    <a class="btn primary" href="/admin/login">Se connecter avec GitHub</a>
  </div>

  <div v-else-if="state === 'forbidden'" class="center">
    <h1>Accès refusé</h1>
    <p class="muted">Connecté en tant que <b>{{ me?.login }}</b>, mais le panneau est réservé aux modérateurs.</p>
    <Btn :loading="loggingOut" @click="logout">Se déconnecter</Btn>
  </div>

  <div v-else-if="state === 'error'" class="center">
    <h1>Erreur</h1>
    <p class="muted">{{ error }}</p>
    <Btn @click="init">Réessayer</Btn>
  </div>

  <div v-else class="layout">
    <header>
      <div class="brand">Magasin d'Island <span class="muted">· administration</span></div>
      <nav>
        <button v-for="t in tabs" :key="t.id" type="button" class="tab" :class="{ on: tab === t.id }" @click="go(t.id)">
          {{ t.label }}
          <span v-if="t.count" class="count" :title="t.hint">{{ t.count }}</span>
        </button>
      </nav>
      <div class="row">
        <Btn v-if="publishing?.open" variant="danger" :loading="switching" title="Bloque tout envoi au magasin" @click="setPublishing(false)">
          Suspendre les publications
        </Btn>
        <img v-if="me?.avatarUrl" :src="me.avatarUrl" alt="" class="avatar" />
        <span>{{ me?.login }}</span>
        <span class="badge" :class="isAdmin ? 'blue' : 'green'">{{ isAdmin ? "admin" : "modérateur" }}</span>
        <Btn :loading="loggingOut" @click="logout">Déconnexion</Btn>
      </div>
    </header>
    <div v-if="publishing && !publishing.open" class="paused">
      <div>
        <b>Publications suspendues</b>
        <span class="muted small">
          · {{ date(publishing.updatedAt) }}<template v-if="publishing.updatedBy"> par {{ publishing.updatedBy }}</template>
        </span>
        <div v-if="publishing.message" class="small">« {{ publishing.message }} »</div>
        <div class="small muted">Aucun envoi ni modification de fiche n'est accepté ; le catalogue et les téléchargements continuent.</div>
      </div>
      <Btn variant="primary" :loading="switching" @click="setPublishing(true)">Rouvrir les publications</Btn>
    </div>
    <p v-if="switchError" class="error">{{ switchError }}</p>
    <main>
      <Reports v-if="tab === 'reports'" @changed="refreshSummary" />
      <Items v-else-if="tab === 'items'" @changed="refreshSummary" />
      <Reviews v-else-if="tab === 'extensions'" @changed="refreshSummary" />
      <Users v-else-if="tab === 'users' && me" :me="me.id" />
      <Journal v-else-if="tab === 'audit'" />
    </main>
  </div>
</template>

<style scoped>
.center {
  min-height: 100vh;
  display: grid;
  place-content: center;
  justify-items: center;
  gap: 8px;
  text-align: center;
}
.layout {
  max-width: 1200px;
  margin: 0 auto;
  padding: 0 16px 40px;
}
header {
  display: flex;
  align-items: center;
  gap: 16px;
  flex-wrap: wrap;
  padding: 14px 0;
  border-bottom: 1px solid var(--line);
  margin-bottom: 18px;
}
.brand {
  font-weight: 600;
}
nav {
  display: flex;
  gap: 4px;
  flex: 1;
  flex-wrap: wrap;
}
.tab {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 30px;
  padding: 0 12px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--muted);
}
.tab:hover {
  color: var(--text);
  background: var(--panel);
}
.tab.on {
  color: var(--text);
  background: var(--panel);
}
.paused {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 12px 14px;
  margin-bottom: 18px;
  border: 1px solid var(--red);
  border-radius: 10px;
  background: color-mix(in srgb, var(--red) 12%, transparent);
}
.count {
  min-width: 18px;
  padding: 0 6px;
  border-radius: 999px;
  background: var(--red);
  color: #fff;
  font-size: 11px;
  line-height: 18px;
}
</style>
