# Intégrations entrantes & temps réel (plan, non implémenté)

> Capture la conception de deux primitives complémentaires : `island.serve` (serveur
> local **entrant**) et le client **temps réel** (WebSocket/SSE, **sortant**). Motivé par
> le cas « intégrer Claude Code (terminal) / un agent IA » dans une extension.

## Le besoin déclencheur : piloter un agent IA local (ex. Claude Code)

Une extension qui :
- **écoute les process** d'un agent (plusieurs instances de Claude Code en parallèle) ;
- **notifie** l'utilisateur quand l'agent a **fini** ;
- affiche la **liste** des process/sessions en cours ;
- quand l'agent pose une **question / demande de permission (choix multiple)**, monte une
  **UI visuelle** pour répondre, et **renvoie** la réponse à l'agent.

Générique : « Claude » = n'importe quel outil/IA capable de faire un `curl localhost`.

## Le point clé : le SENS du flux

- **`island.serve` (ENTRANT)** — Island héberge un petit **serveur local** ; des process
  locaux **poussent** vers lui et **lisent sa réponse**. → c'est CE qu'il faut pour Claude
  Code (l'agent émet des événements, Island répond).
- **Client temps réel WS/SSE (SORTANT)** — l'extension **se connecte** à un serveur
  **distant** pour consommer un flux. → utile pour **autre chose** (voir plus bas), pas
  pour écouter des process locaux.

> ⚠️ Ne pas confondre : pour Claude Code, WS/SSE-client n'est PAS le bon outil. C'est
> `island.serve` (entrant) qui débloque ce cas.

## Architecture Claude Code (via les *hooks*)

Claude Code lance une commande sur des événements (`settings.json` → `hooks`). La commande
fait un `curl` vers le serveur local d'Island :

| Hook Claude Code | Island (handler `serve`) |
|---|---|
| `SessionStart` | enregistre la session (dossier, id) → **liste des process** |
| `Stop` (réponse terminée) | **notification** « terminé » |
| `Notification` (besoin d'attention) | bannière |
| `PreToolUse` (permission/outil) | le hook **POST** la demande puis **attend**/poll la réponse → **UI de choix** dans Island → le hook renvoie la décision (allow/deny) |

La partie subtile = « répondre à l'agent » : le hook **bloque** et **poll** Island jusqu'à
la réponse de l'utilisateur (faisable ; c'est le morceau le plus délicat à câbler).

## `island.serve` — design (sécurité d'abord)

Reprend [extensions-as-api.md](extensions-as-api.md). Serveur HTTP local hôte, **un seul**
pour toutes les extensions, routage par extension. Garde-fous :
- **Loopback only** (`127.0.0.1`), jamais exposé sur le réseau.
- **Token** par extension (généré au `serve.start`, requis en query/header) → un autre
  process local ne peut pas appeler à l'aveugle.
- **Vérif `Host`** (anti DNS-rebinding).
- Permission **`serve`** (⚠ ouvre un port local) déclarée au manifeste.
- **Cleanup** : route retirée à la désactivation de l'extension.

API SDK envisagée :
```ts
const ep = await ctx.serve.start();              // { port, token, url } ; ouvre la route
ctx.serve.on(async (req) => {                    // req = { method, path, query, body }
  // …afficher une notif / une view, attendre une réponse utilisateur…
  return { status: 200, body: { decision: "allow" } };
});
ctx.serve.stop();
```

## Client temps réel WS/SSE — design (sortant)

Permission **`network`** (réutilise l'existant). À quoi ça sert (≠ Claude Code) :
- **Streamer une API IA directement** (Claude/OpenAI renvoient du **SSE**) → afficher la
  réponse token par token dans une view.
- Tickers crypto/bourse, scores, **chat** (Discord/Slack via WS), présence, dashboards
  live, notifs d'un service web.

API SDK envisagée :
```ts
const sock = ctx.realtime.ws(url, { headers });
sock.onMessage((data) => …); sock.send("…"); sock.close();
const es = ctx.realtime.sse(url);   // flux SSE → es.onMessage(...)
```
Côté hôte : connexion native (hors webview → pas de souci CORS/origin), gérée sur un
thread/loop, événements relayés au front par `event` Tauri.

## Priorités

1. **`island.serve`** — débloque l'intégration Claude Code/agents locaux ; primitive la
   plus stratégique (Island = **hub d'intégration** pour tous les outils locaux).
2. **WS/SSE client** — pour les extensions qui consomment un flux distant (API IA en
   streaming, données live).

Les deux sont complémentaires : une même extension « copilote IA » pourrait utiliser
`serve` (recevoir les events de Claude Code) **et** WS/SSE (streamer la réponse d'une API
IA distante).
