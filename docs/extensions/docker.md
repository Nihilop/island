# Extension — Docker (à venir)

Centre de contrôle Docker glanceable dans l'île. Statut ambiant + actions.

## But
Voir l'état des conteneurs sans alt-tab, et agir : list / start / stop / restart /
kill / logs / stats. Contrôler des **projets compose existants** (up/down/restart d'un
groupe) — pas réimplémenter `docker compose up` (build/depends_on) : pour ça, lancer le
CLI via le service `pty` (cf. dev-palette).

## Slots Island
- **idle** : point/badge = nb de conteneurs up (rouge si un service clé est down).
- **view** : liste des conteneurs (nom, image, statut, ports) + CPU/RAM (poll stats).
- **window** : logs streamés d'un conteneur (xterm ou texte).
- **notify** : un conteneur meurt / redémarre.
- **launcher** : actions rapides (restart `<service>`, up/down d'un projet).

## Accès à Docker
- **MVP (sans toucher l'hôte)** : activer `tcp://localhost:2375` dans Docker Desktop →
  `island.http` tape l'**Engine API** (`GET /containers/json`, `POST /containers/{id}/
  start|stop|restart`, `GET /containers/{id}/logs?tail=`, `GET /containers/{id}/stats`).
- **Propre (service hôte `docker`)** : parler au **named pipe** `//./pipe/docker_engine`
  (pas de toggle TCP non-sécurisé) + streaming logs/stats. ~150 lignes Rust. À évaluer
  après le MVP.

## Compose
Les conteneurs portent le label `com.docker.compose.project` → filtrer dessus = up/down/
restart d'un projet entier. Le `up` initial (création) = `docker compose up -d` via `pty`.

## Permission
`network` (MVP http) ; un éventuel service hôte `docker` aurait sa propre permission.

## Reste à décider
- Format exact de la view (table dense vs cartes).
- Logs : streaming (service hôte) vs poll `?tail=` (http).
- Multi-contexte Docker (WSL2 vs Windows engine).
