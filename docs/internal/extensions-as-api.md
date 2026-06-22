# Extensions comme API entrante (`island.serve`)

> Statut : **design / non implémenté**. Note de faisabilité + plan sécurisé.

## Contexte

Aujourd'hui une extension peut **consommer** des API (sortant : `fetch`, ou via
l'hôte pour contourner CORS). On veut aussi qu'une extension puisse **être une
API** : recevoir des appels de services externes (ex. un relais de notifications
pingé par un webhook, Home Assistant, un CI « build done »…).

## Le verrou

Les extensions tournent dans le **WebView** (contexte navigateur). Un navigateur
peut faire des requêtes **sortantes** mais **ne peut pas ouvrir un port d'écoute**.
➡️ Donc le JS de l'extension ne peut pas être pingé directement. **C'est l'hôte
(Rust) qui doit tenir le serveur** et router vers le handler JS de l'extension —
même pattern d'aller-retour event/commande que le reste du SDK.

## Les possibilités

| Approche | Origine des appels | Faisabilité | Notes |
|---|---|---|---|
| **Serveur HTTP local (hôte) → route vers l'extension** | même machine / LAN | ✅ **recommandé** | `127.0.0.1:<port>`, l'hôte gère, l'extension enregistre un handler |
| ⤷ mode **fire-and-forget** | idem | ✅ **le plus sûr** | l'hôte répond `202` direct + émet un event ; pas de corrélation req/résp |
| ⤷ mode **request/response** | idem | ✅ (opt-in) | `reqId` + `oneshot` + timeout côté Rust ; pour un vrai body en retour |
| **Connexion sortante vers un relais cloud** (WS / SSE) | Internet | ✅ | l'extension se connecte en sortie, le service externe pousse via le relais → **sans ouvrir de port**. Le webview sait le faire seul |
| **Serveur WebSocket local (hôte)** | même machine | ✅ | pour du streaming bidirectionnel ; même garde-fous que HTTP |
| **Named pipe / socket local** | même machine | ✅ | IPC inter-process, pas de port TCP ; plus « natif » mais moins universel |
| **Exposer un port au réseau/Internet** (`0.0.0.0` / port-forward) | Internet direct | ⚠️ **à éviter** | surface d'attaque énorme ; préférer le relais cloud |

## Recommandation

1. **Inbound même machine / LAN** → serveur **HTTP local tenu par l'hôte** +
   primitive `island.serve`, en **mode fire-and-forget + token** (le plus
   défensif, surface minimale). C'est ce qu'il faut pour le relais de notifs.
2. **Inbound depuis Internet** → **connexion sortante** de l'extension vers un
   relais (WS/SSE), jamais d'ouverture de port local.

## Modèle de menace & garde-fous (non négociables)

Menaces : process local malveillant • page web (DNS rebinding vers `127.0.0.1`) •
autre extension • abus de ressources.

| Règle | Pourquoi |
|---|---|
| **Bind `127.0.0.1` uniquement** | aucune machine du réseau n'atteint le port |
| **Token par extension** (haute entropie, généré par l'hôte) | une page web / un tiers ne le connaît pas → 401. Défense principale |
| **Vérifier `Host` == `127.0.0.1:<port>`** | tue le DNS rebinding (le navigateur enverrait `Host: evil.com` → rejet) |
| **Pas de CORS** (aucun `Access-Control-Allow-*`) | le navigateur ne peut pas lire les réponses |
| **Permission manifeste `serve`** → consentement à l'install | ouvrir un port est sensible |
| **Routes scopées `/ext/<id>/…`** | une extension ne voit que ses routes |
| **Limite de payload (~64 Ko) + rate-limit léger** | anti-DoS |
| **Cleanup au unload** (token + routes révoqués) | même pattern que `unregisterShortcutsFor` |

Token : header `Authorization: Bearer …` (préféré) **ou** `?token=…` (pragmatique
pour les webhooks qui ne configurent qu'une URL). Loopback en HTTP clair (pas de
TLS : non sniffable hors machine, et évite les galères de certificats).

## Archi hôte (clean)

- **Un seul** serveur Rust (axum / tiny_http), démarré **à la demande**, port
  `127.0.0.1:0` (choisi par l'OS) ou fixe configurable.
- Registre `extId → { token, prefix }`.
- **Middleware** : check `Host` + token AVANT de réveiller le JS → sinon 401/403.
- Routage : requête valide → `serve://request { reqId, extId, method, path, body }`
  → handler JS → (fire-and-forget : rien ; req/résp : `serve_respond(reqId, …)`).
- Cleanup dans `deactivateExtension` (révoque l'entrée du registre).

## Forme SDK (esquisse)

```ts
// manifest : permissions: ["serve"]
const { url, token } = island.serve((req) => {
  island.notify({ title: req.body.title, body: req.body.body, source: "Webhook" });
});
// url = http://127.0.0.1:<port>/ext/<id>  → à afficher dans la config de l'extension
island.unserve();
```

## Phases

1. **Fire-and-forget + token** (serveur Rust + middleware + `island.serve` +
   permission `serve`). Premier exemple : extension « relais de notifications ».
2. Mode **request/response** (corrélation `reqId` + timeout).
3. **WebSocket** local (streaming) si besoin.
4. Helper **relais cloud sortant** (pour l'inbound depuis Internet).
