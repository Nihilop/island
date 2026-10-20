<script setup lang="ts">
// Relecture des extensions : file des versions en attente (les plus anciennes d'abord), puis
// une version : permissions (ajoutées en orange), fichiers et différences avec la version
// approuvée, lecture d'un fichier (coloré, et mis en forme s'il est minifié). « Approuver » signe le paquet avec la clé du
// magasin et le publie ; « Refuser » demande un motif, montré à l'auteur.
import { computed, onMounted, ref, watch } from "vue";
import { api, date, message } from "../api";
import { langOf, looksMinified, render } from "../code";
import "highlight.js/styles/github-dark.css";
import Btn from "../Btn.vue";

interface Review {
  id: number;
  itemId: string;
  name: string;
  owner: string;
  version: string;
  size: number;
  scan: string;
  permissions: string[];
  previousVersion: string | null;
  previousPermissions: string[] | null;
  createdAt: string;
}

interface FileChange {
  path: string;
  size: number;
  change: "added" | "modified" | "unchanged" | "removed";
}

interface Detail {
  review: Review;
  manifest: { id: string; name: string; version: string; description?: string; permissions: string[]; author?: string };
  files: FileChange[];
  addedPermissions: string[];
  authorSignature: "signed" | "unsigned";
  authorKey: string | null;
  meta: { summary?: string; description?: string; homepage?: string; repository?: string };
  changelog: string;
}

const emit = defineEmits<{ changed: [] }>();
const queue = ref<Review[] | null>(null);
const loading = ref(false);
const error = ref<string | null>(null);

async function load() {
  loading.value = true;
  error.value = null;
  try {
    queue.value = await api<Review[]>("GET", "/v1/mod/reviews");
  } catch (e) {
    error.value = message(e);
  } finally {
    loading.value = false;
  }
}

// --- Une version -------------------------------------------------------------------------

const detail = ref<Detail | null>(null);
const opening = ref<number | null>(null);
async function open(r: Review) {
  if (opening.value) return;
  opening.value = r.id;
  error.value = null;
  try {
    detail.value = await api<Detail>("GET", `/v1/mod/reviews/${r.id}`);
    viewed.value = null;
    showUnchanged.value = r.previousVersion === null;
  } catch (e) {
    error.value = message(e);
  } finally {
    opening.value = null;
  }
}

const showUnchanged = ref(false);
const files = computed(() => (detail.value?.files ?? []).filter((f) => showUnchanged.value || f.change !== "unchanged"));
const counts = computed(() => {
  const c = { added: 0, modified: 0, removed: 0, unchanged: 0 };
  for (const f of detail.value?.files ?? []) c[f.change]++;
  return c;
});

const viewed = ref<{ path: string; text?: string; image?: string; error?: string } | null>(null);
/** Mise en forme du code minifié (activée d'office quand le fichier en a l'air). */
const formatted = ref(true);
const html = ref<string | null>(null);
const minified = computed(() => !!viewed.value?.text && looksMinified(viewed.value.text));
const canFormat = computed(() => !!viewed.value && ["javascript", "css", "xml", "json"].includes(langOf(viewed.value.path)));
watch([viewed, formatted], async () => {
  const v = viewed.value;
  html.value = null;
  if (!v?.text) return;
  const out = await render(v.text, v.path, formatted.value && canFormat.value && minified.value).catch(() => null);
  if (viewed.value === v) html.value = out;
});
const reading = ref<string | null>(null);
async function view(f: FileChange) {
  if (!detail.value || f.change === "removed" || reading.value) return;
  reading.value = f.path;
  const url = `/v1/mod/reviews/${detail.value.review.id}/file?path=${encodeURIComponent(f.path)}`;
  try {
    const res = await fetch(url, { credentials: "same-origin" });
    if (!res.ok) {
      const code = (await res.json().catch(() => null))?.error;
      viewed.value = { path: f.path, error: code === "binary" ? "Fichier binaire (non affichable)." : code === "tooLargeToView" ? "Trop gros pour être affiché." : message(code) };
    } else if (res.headers.get("content-type")?.startsWith("image/")) {
      viewed.value = { path: f.path, image: URL.createObjectURL(await res.blob()) };
    } else {
      viewed.value = { path: f.path, text: await res.text() };
    }
  } finally {
    reading.value = null;
  }
}

const busy = ref<"approve" | "reject" | null>(null);
async function approve() {
  if (!detail.value || busy.value) return;
  if (!window.confirm(`Approuver ${detail.value.review.name} v${detail.value.review.version} ? Elle sera signée par le magasin et publiée.`)) return;
  busy.value = "approve";
  try {
    await api("POST", `/v1/mod/reviews/${detail.value.review.id}/approve`);
    done();
  } catch (e) {
    error.value = message(e);
  } finally {
    busy.value = null;
  }
}
async function reject() {
  if (!detail.value || busy.value) return;
  const reason = window.prompt("Motif du refus (montré à l'auteur) :");
  if (!reason?.trim()) return;
  busy.value = "reject";
  try {
    await api("POST", `/v1/mod/reviews/${detail.value.review.id}/reject`, { reason });
    done();
  } catch (e) {
    error.value = message(e);
  } finally {
    busy.value = null;
  }
}
function done() {
  detail.value = null;
  emit("changed");
  void load();
}

const size = (b: number) => (b < 1024 ? `${b} o` : b < 1 << 20 ? `${(b / 1024).toFixed(1)} Ko` : `${(b / (1 << 20)).toFixed(1)} Mo`);
const CHANGE: Record<FileChange["change"], [string, string]> = {
  added: ["ajouté", "green"],
  modified: ["modifié", "orange"],
  removed: ["supprimé", "red"],
  unchanged: ["inchangé", ""],
};

onMounted(load);
</script>

<template>
  <section>
    <div v-if="error" class="error">{{ error }}</div>

    <!-- Une version -->
    <template v-if="detail">
      <div class="toolbar">
        <button type="button" class="btn" @click="detail = null">← File de relecture</button>
        <div style="flex: 1" />
        <Btn variant="danger" :loading="busy === 'reject'" :disabled="!!busy" @click="reject">Refuser…</Btn>
        <Btn variant="primary" :loading="busy === 'approve'" :disabled="!!busy" @click="approve">Approuver et signer</Btn>
      </div>

      <div class="card">
        <h2 style="margin: 0">
          {{ detail.review.name }} <span class="muted">v{{ detail.review.version }}</span>
          <span v-if="detail.review.previousVersion" class="badge" style="margin-left: 6px">mise à jour de v{{ detail.review.previousVersion }}</span>
          <span v-else class="badge blue" style="margin-left: 6px">nouvelle extension</span>
        </h2>
        <div class="muted small">
          {{ detail.manifest.id }} · par {{ detail.review.owner }} · {{ size(detail.review.size) }} · déposée le {{ date(detail.review.createdAt) }} ·
          <span :class="detail.review.scan === 'clean' ? '' : 'orange-text'">antivirus : {{ detail.review.scan === "clean" ? "sain" : "non analysé" }}</span>
          · {{ detail.authorSignature === "signed" ? `signée par l'auteur (clé ${detail.authorKey})` : "non signée par l'auteur" }}
        </div>
        <p v-if="detail.meta.summary" style="margin: 10px 0 0">{{ detail.meta.summary }}</p>
        <p v-if="detail.changelog" class="small" style="white-space: pre-wrap"><b>Nouveautés :</b> {{ detail.changelog }}</p>
      </div>

      <h3>Permissions</h3>
      <div class="row" style="flex-wrap: wrap">
        <span v-if="!detail.manifest.permissions.length" class="muted small">Aucune.</span>
        <span
          v-for="p in detail.manifest.permissions"
          :key="p"
          class="badge"
          :class="detail.addedPermissions.includes(p) ? 'orange' : ''"
          :title="detail.addedPermissions.includes(p) ? 'Nouvelle permission' : ''"
        >
          {{ p }}<template v-if="detail.addedPermissions.includes(p) && detail.review.previousVersion"> · nouvelle</template>
        </span>
      </div>
      <p class="muted small">À vérifier : chaque permission est justifiée par la description, et le code ne fait rien d'autre que ce qu'annonce la fiche.</p>

      <h3>
        Fichiers
        <span class="muted small" style="font-weight: normal">
          <template v-if="detail.review.previousVersion">
            {{ counts.added }} ajoutés · {{ counts.modified }} modifiés · {{ counts.removed }} supprimés · {{ counts.unchanged }} inchangés
          </template>
          <template v-else>{{ detail.files.length }} fichiers</template>
        </span>
      </h3>
      <label v-if="detail.review.previousVersion" class="small"><input v-model="showUnchanged" type="checkbox" /> afficher les fichiers inchangés</label>
      <div class="review-grid">
        <div class="table-wrap">
          <table>
            <tbody>
              <tr v-for="f in files" :key="f.path" class="file" :class="{ on: viewed?.path === f.path }" @click="view(f)">
                <td>
                  <code>{{ f.path }}</code>
                  <span v-if="reading === f.path" class="spinner muted" style="margin-left: 6px" />
                </td>
                <td class="small muted">{{ f.change === "removed" ? "" : size(f.size) }}</td>
                <td><span v-if="CHANGE[f.change][0] !== 'inchangé' || !detail.review.previousVersion" class="badge" :class="CHANGE[f.change][1]">{{ detail.review.previousVersion ? CHANGE[f.change][0] : "nouveau" }}</span></td>
              </tr>
            </tbody>
          </table>
        </div>
        <div class="viewer">
          <div v-if="!viewed" class="muted small">Clique sur un fichier pour le lire.</div>
          <template v-else>
            <div class="small row" style="margin-bottom: 6px; justify-content: space-between">
              <code>{{ viewed.path }}</code>
              <label v-if="minified && canFormat" class="muted"><input v-model="formatted" type="checkbox" /> mettre en forme (code minifié)</label>
            </div>
            <div v-if="viewed.error" class="muted">{{ viewed.error }}</div>
            <img v-else-if="viewed.image" :src="viewed.image" alt="" style="max-width: 100%" />
            <pre v-else-if="html !== null" class="hljs"><code v-html="html" /></pre>
            <pre v-else>{{ viewed.text }}</pre>
          </template>
        </div>
      </div>
    </template>

    <!-- File -->
    <template v-else>
      <div class="toolbar">
        <span class="muted small">Versions d'extensions en attente de relecture, les plus anciennes d'abord.</span>
        <span v-if="loading" class="spinner muted" />
      </div>
      <div v-if="queue && !queue.length" class="empty">Aucune extension à relire.</div>
      <div v-else-if="queue" class="table-wrap">
        <table>
          <thead>
            <tr>
              <th>Extension</th>
              <th>Auteur</th>
              <th>Version</th>
              <th>Permissions</th>
              <th>Déposée</th>
              <th />
            </tr>
          </thead>
          <tbody>
            <tr v-for="r in queue" :key="r.id">
              <td>
                <b>{{ r.name }}</b>
                <div class="muted small">{{ r.itemId }}</div>
              </td>
              <td>{{ r.owner }}</td>
              <td>
                v{{ r.version }}
                <div class="muted small">{{ r.previousVersion ? `après v${r.previousVersion}` : "nouvelle" }}</div>
              </td>
              <td>
                <span
                  v-for="p in r.permissions"
                  :key="p"
                  class="badge"
                  :class="r.previousPermissions && !r.previousPermissions.includes(p) ? 'orange' : ''"
                  style="margin: 0 4px 4px 0"
                  >{{ p }}</span
                >
                <span v-if="!r.permissions.length" class="muted small">aucune</span>
              </td>
              <td class="small">{{ date(r.createdAt) }}</td>
              <td><Btn :loading="opening === r.id" :disabled="!!opening" @click="open(r)">Relire</Btn></td>
            </tr>
          </tbody>
        </table>
      </div>
    </template>
  </section>
</template>

<style scoped>
.card {
  padding: 14px;
  border: 1px solid var(--line);
  border-radius: 10px;
  margin-bottom: 8px;
}
h3 {
  margin: 18px 0 8px;
  font-size: 14px;
}
.review-grid {
  display: grid;
  grid-template-columns: minmax(280px, 2fr) 3fr;
  gap: 12px;
  margin-top: 8px;
}
.file {
  cursor: pointer;
}
.file:hover,
.file.on {
  background: var(--panel);
}
.viewer {
  min-height: 200px;
  max-height: 70vh;
  overflow: auto;
  padding: 10px;
  border: 1px solid var(--line);
  border-radius: 10px;
}
.viewer pre {
  margin: 0;
  font-size: 12px;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
}
.viewer pre.hljs {
  padding: 0;
  background: transparent;
}
.orange-text {
  color: var(--orange, #f5a524);
}
</style>
