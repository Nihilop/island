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

- glisser le `.island` sur le notch ;
- double-cliquer sur le fichier (association créée à la première installation) ;
- passer par **Réglages › Extensions › Installer…**.

Une fiche s'ouvre dans le notch : nom, auteur, état de la signature, permissions en clair,
ce que l'extension ajoute. Après acceptation, Island vérifie à nouveau le paquet, l'installe
d'un coup et enregistre les permissions consenties.

## Mettre à jour

Même chemin qu'une installation, avec une version plus récente et le même `id` : la fiche
indique la version installée et met en évidence les **nouvelles** permissions. Le stockage
et les secrets de l'extension sont conservés.

## Désinstaller

Réglages › Extensions › corbeille (deux clics). Island supprime le dossier, les permissions,
le stockage et les secrets de l'extension.

## Limites

- 2 000 fichiers et 64 Mo décompressés au plus.
- Chemins relatifs sans `..` ; tous les fichiers référencés par le manifeste doivent être
  présents.
