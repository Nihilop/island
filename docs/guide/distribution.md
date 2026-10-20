# Distribution

Une extension se distribue en **paquet `.island`** : un zip qui contient le manifeste, les
fichiers construits (`dist/`, `icons/`, `locales/`, `assets/`) et une signature.

## Empaqueter

L'outil `packages/sdk/bin/island.mjs` ne demande que Node :

```bash
node packages/sdk/bin/island.mjs pack examples/extensions/github --out github.island
```

Sans clé, le paquet n'est **pas signé** : il ne s'installe qu'en mode développeur, après un
avertissement.

## Signer

```bash
node packages/sdk/bin/island.mjs keygen ma-cle.json
node packages/sdk/bin/island.mjs pack . --key ma-cle.json
```

- La signature (ed25519) porte sur l'empreinte de chaque fichier du paquet : un paquet
  modifié après signature est refusé, même en mode développeur.
- **Gardez la clé privée secrète et sauvegardée** : sans elle, plus de mise à jour signée.
- Seules les clés reconnues par Island donnent le statut « Signé par Island ». Un paquet
  signé par une autre clé est traité comme non officiel (mode développeur).

## Installer

L'utilisateur peut :

- glisser le `.island` sur l'île ;
- double-cliquer sur le fichier (association créée à la première installation) ;
- passer par **Réglages › Extensions › Installer…**.

Une fiche s'ouvre dans l'île : nom, auteur, état de la signature, permissions en clair,
ce que l'extension ajoute. Après acceptation, Island vérifie à nouveau le paquet, l'installe
d'un coup et enregistre les permissions consenties.

## Mettre à jour

Même chemin qu'une installation, avec une version plus récente et le même `id` : la fiche
indique la version installée et met en évidence les **nouvelles** permissions. Le stockage
et les secrets de l'extension sont conservés.

## Désinstaller

Réglages › Extensions › corbeille (deux clics). Island supprime le dossier, les permissions,
le stockage et les secrets de l'extension.

## Publier au magasin

Le même outil publie sur le magasin d'Island. Pour l'instant, le magasin accepte les
**scènes de fond animé** (`.islandscene`, exportées depuis l'éditeur avec « Partager »).
Les extensions arrivent ensuite.

```bash
node packages/sdk/bin/island.mjs login      # connexion GitHub : un code à saisir sur github.com
node packages/sdk/bin/island.mjs whoami
node packages/sdk/bin/island.mjs publish aurore.islandscene --summary "Aube boréale" --tags nuit,ciel
node packages/sdk/bin/island.mjs publish aurore.islandscene --changelog "Étoiles plus fines"
node packages/sdk/bin/island.mjs logout
```

- La première publication crée un élément. Republier **le même fichier** envoie une
  **nouvelle version** de cet élément ; `--new` crée un autre élément, et `--item <id>`
  vise un élément précis.
- Le résumé, les étiquettes et les liens peuvent aussi venir du paquet (manifeste et
  `README`). Les options de la commande passent devant.
- Le jeton du magasin est rangé dans `~/.island/credentials.json`, lisible par vous seul.
  `logout` le révoque côté magasin.
- `--store <url>` (ou la variable `ISLAND_STORE_URL`) vise un autre magasin, par exemple un
  serveur de développement : `--store http://127.0.0.1:8080`.
- Si les publications sont suspendues par la modération, la commande le dit et n'envoie
  rien. Réessayez plus tard.

Depuis l'app, le bouton **Publier** de l'éditeur de fonds fait la même chose, sans passer
par un fichier.

## Limites

- 2 000 fichiers et 64 Mo décompressés au plus.
- Chemins relatifs sans `..` ; tous les fichiers référencés par le manifeste doivent être
  présents.
