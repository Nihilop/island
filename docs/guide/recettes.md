# 6. Recettes

Des patterns concrets, tirés des extensions de référence (Flow, Capture, Aniplex,
Spotify, Monitoring).

## Saisie clavier dans une view

L'overlay ne prend **pas** le focus clavier par défaut (volontaire : il ne vole pas le
focus aux jeux). Si ta view a un `<input>` :

```ts
ctx.view.open(Search, COMPACT);
ctx.invoke("overlay_focus");   // donne le focus clavier à l'overlay
// puis, au montage du composant : inputEl.focus()
```

Quand l'overlay perd le focus (clic ailleurs), l'île se referme d'elle-même. C'est ce
que fait **Flow** (le launcher).

## Lancer des applications

*Permission : `apps`.*

```ts
const EXT_ID = "com.island.flow";
const apps = await useIsland().invoke<{name: string; path: string}[]>("list_apps", { extId: EXT_ID });
// …filtrer / scorer…
await useIsland().invoke("launch_path", { extId: EXT_ID, path: choisi });
```

`list_apps` renvoie les raccourcis du menu Démarrer ; `launch_path` exécute la cible via
ShellExecute. `app_icons({ extId, paths })` renvoie les icônes (PNG data URL) à la
demande. ⚠ Palier de confiance élevé (lance des exécutables).

## Appeler une API (réseau)

### Cas simple : `fetch` standard

L'hôte tourne avec `csp: null` → un `fetch()` vers une API qui renvoie du CORS marche
sans rien déclarer. Exemple : `https://meme-api.com/gimme` (CORS `*`).

### Appeler une API avec session cookie

*Permission : `network`.* Quand l'API utilise une **session par cookie** (login), un
`fetch` navigateur est bloqué par CORS/SameSite. Utilise le HTTP **natif** d'Island, qui
tient un cookie-jar par extension :

```ts
const EXT_ID = "com.island.aniplex";
const r = await useIsland().http.request({
  extId: EXT_ID,
  method: "POST",
  url: "https://api.exemple.com/auth/login",
  body: { user, pass },                 // objet → JSON automatique
  headers: { "X-Custom": "…" },
});
// r = { status: number, body: string }
```

Le cookie posé par une réponse est **rejoué** sur les requêtes suivantes, et le cookie-jar
est **persisté sur disque** → la session est conservée même après un redémarrage d'Island
(plus besoin de se reconnecter), exactement comme un client desktop.

## Embarquer un binaire natif

*Permission : `native-encoder`.* L'hôte ne décide pas du codec : une extension peut
fournir son propre binaire (ex. ffmpeg). C'est un **cran de confiance au-dessus du JS** :

```ts
const EXT_ID = "com.island.capture";
// 1) Télécharger le binaire DANS le dossier de l'extension (au 1er besoin)
await useIsland().capture.fetchBinary({
  extId: EXT_ID,
  url: "https://…/ffmpeg-win64.zip",
  dest: "binaries/ffmpeg.exe",
  zipEntry: "bin/ffmpeg.exe",           // si l'URL est un zip : entrée à extraire
});                                      // progression via event "encoder://download"

// 2) Enregistrer en fournissant l'encodeur
await useIsland().capture.startRecording({
  region, display, fps: 30,
  encoder: { extId: EXT_ID, bin: "binaries/ffmpeg.exe", args: ["-c:v","libx265","-crf","25"] },
});
```

- Le binaire et la destination sont **cantonnés** à `extensions/<id>/` (anti
  `..`/chemins absolus) → impossible de lancer un programme système.
- **Ne le mets pas dans le `.island`** : garde le paquet léger, télécharge au 1er besoin,
  `.gitignore` le dossier `binaries/`.

**Son système** : ajoute `audio: true` + `audioArgs` :

```ts
startRecording({ …, audio: true, encoder: { …, audioArgs: ["-c:a","aac","-b:a","160k"] } });
```

L'hôte capture le son du PC (WASAPI loopback), encode la vidéo, puis muxe les deux.

## Fenêtre flottante (lecteur, mini-outil)

```ts
const id = ctx.window.open(Player, { title: "Lecteur", width: 480, height: 270, resizable: true });
// déplaçable, indépendant de l'île ; ctx.window.close(id) pour fermer
```

## Ouvrir une URL dans le navigateur

```ts
await ctx.openExternal("https://exemple.com/watch/123");
```

Pratique quand la lecture/le rendu est mieux géré par le navigateur (c'est ce que fait
**Aniplex** : clic sur un épisode → ouvre le player web + `ctx.view.close()`).

## Alimenter la recherche du launcher (provider)

Le lanceur de l'île devient une **palette de commandes extensible** : enregistre un
provider, et dès que l'utilisateur tape, tes résultats s'affichent. Tant qu'aucune
extension n'enregistre de provider, le lanceur reste une simple grille (le champ de
recherche n'apparaît que s'il y a au moins un provider).

```ts
activate(ctx) {
  ctx.launcher.provider({
    onQuery: async (q) => {
      if (!q) return [];
      const hits = await chercher(q);                 // sync ou async (fetch, calcul…)
      return hits.map((h) => ({
        id: h.id,
        title: h.nom,
        subtitle: h.detail,
        icon: "<svg…>",                                // optionnel (icône par défaut sinon)
        onActivate: () => ouvrir(h),                   // Entrée ou clic
      }));
    },
  });
}
// nettoyage automatique à la désactivation ; ou ctx.launcher.removeProvider()
```

Les résultats de **tous** les providers actifs sont fusionnés. `Entrée` active le
premier résultat. Une extension = un provider.

## Communiquer entre extensions (bus)

Une extension « producteur » publie un événement ; d'autres s'y abonnent — sans se
connaître. Préfixe tes canaux pour éviter les collisions.

```ts
// Producteur (ex. un lecteur) :
ctx.bus.emit("nowplaying:update", { title, artist });

// Consommateur (ex. un widget) :
const off = ctx.bus.on("nowplaying:update", (p) => afficher(p));
// `off()` pour se désabonner ; sinon nettoyé automatiquement à la désactivation.
```

## Lire à voix haute / taper du texte

```ts
ctx.speak("Build terminé");                 // synthèse vocale (aucune permission)
await ctx.input.typeText("Bonjour 👋");      // tape dans l'app active (⚠ permission `input`)
```

## Notifier la fin d'une tâche

```ts
ctx.notify({
  title: "Enregistrement terminé", body: "0:42", color: "#ff453a",
  source: "Capture", onClick: () => island.invoke("reveal_path", { path }),  // ouvre le fichier
});
```

`reveal_path` (révéler dans l'explorateur) n'est pas gardé : c'est une commande d'app.
