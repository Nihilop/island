# Le magasin

Le magasin d'Island distribue des **fonds animés** (scènes `.islandscene`) et des
**extensions** (`.island`). L'app y accède par sa fenêtre Magasin, les auteurs y publient
depuis l'app ou avec le CLI du SDK (`island publish`, cf. [Distribution](/guide/distribution)).

- Les **scènes** sont publiées tout de suite, après vérification du fichier.
- Les **extensions** passent par une **relecture humaine** : la modération lit le code et les
  permissions, puis le magasin les **signe avec sa propre clé**. Island les installe alors
  comme « Vérifiées par le magasin ».
- Les installations se mettent à jour toutes seules : vérification au démarrage, puis
  chaque jour.

## Héberger son propre magasin

Le serveur est libre, comme l'app : un fork d'Island peut avoir son magasin. Le guide
complet, en anglais, est
[`server/INSTALL.md`](https://github.com/Nihilop/island/blob/main/server/INSTALL.md). En
résumé :

| Pièce | Rôle | Exemple |
|---|---|---|
| Serveur (image Docker) | API et panneau de modération `/admin` | `ghcr.io/<toi>/island-store` |
| Postgres | comptes, catalogue, versions, journal | Postgres du compose, ou Neon |
| Cloudflare R2 | fichiers : médias **publics** (CDN), paquets **privés** (liens signés) | deux compartiments |
| App OAuth GitHub | connexion des auteurs et des modérateurs | *Enable Device Flow* coché |
| Clé du magasin | signe les extensions approuvées | `island keygen` |

1. **Les fichiers** : un compartiment R2 public relié à un domaine (les médias), un second
   **privé**, sans domaine (les paquets), et un jeton *Object Read & Write* limité aux deux.
2. **La clé du magasin** :
   `node packages/sdk/bin/island.mjs keygen secrets/store-signing-key.json`. Elle reste
   **hors du dépôt**, sauvegardée.
3. **Le serveur** : le dossier `server/deploy/` contient un `docker-compose.yml` prêt (profils
   optionnels : tunnel Cloudflare, ClamAV, Postgres local) et un `.env.example` commenté.
   `docker compose up -d`, puis `/health` doit répondre `ok`.
4. **L'app** : l'adresse et la clé du magasin sont des réglages **de compilation**. Dans ton
   fork, *Settings › Secrets and variables › Actions › Variables* :
   - `ISLAND_STORE_URL` : l'adresse publique du serveur ;
   - `ISLAND_STORE_KEYS` : sa clé publique, affichée au démarrage du serveur et renvoyée par
     `GET /v1/store/status` (`signingKey`).

   Pour un build local, les mêmes lignes vont dans `.env` à la racine (cf. `.env.example`).

## Sécurité

- **Ne commite jamais** `.env`, `secrets/` ni une clé privée : le dépôt les ignore, garde-le
  ainsi. L'image Docker n'embarque aucun secret, le `.env` et la clé sont montés au
  lancement.
- **Un jeton R2 de production à part** : celui de développement voit aussi les dossiers de
  production.
- **La clé du magasin est précieuse** : Island ne reconnaît que les clés compilées dans l'app.
  La perdre, c'est publier une nouvelle version d'Island avec une nouvelle clé.
- En cas de vague d'envois impossible à modérer, le panneau propose **Suspendre les
  publications** : le catalogue et les téléchargements continuent, plus rien n'est accepté.
