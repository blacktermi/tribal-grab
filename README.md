# Tribal Grab

Gestionnaire de téléchargement pour macOS, écrit en Rust par [Tribal Studio](https://tribalstudio.dev).

Il remplace le téléchargeur de votre navigateur pour **tous les types de fichiers**, avec plusieurs connexions par fichier, la pause et la reprise. Il sait aussi récupérer la vidéo et l'audio de plus de 1 800 sites.

*A macOS download manager written in Rust: any file type, multi-connection, pause and resume, plus video and audio from 1,800+ sites through `yt-dlp`. The interface is in French.*

## Fonctions

- **Tous les fichiers** : archives, PDF, installeurs, images, vidéos. Chaque fichier est enregistré tel quel, sous son vrai nom.
- **Pause et reprise** : un téléchargement mis en pause, coupé ou interrompu repart à l'octet près, si le serveur accepte les requêtes par plage. Sinon, il repart de zéro.
- **Progression lisible** : taille reçue, taille totale, pourcentage, débit et temps restant.
- **Rapidité** : jusqu'à 8 connexions par fichier. Si un serveur limite les connexions, le moteur s'adapte au lieu d'échouer.
- **Meilleure qualité** : toujours la plus haute résolution disponible. Ce que QuickTime ne lit pas est converti à la même résolution par l'encodeur matériel du Mac, et l'original reste à côté dans un fichier `.mkv`.
- **Vidéo et audio** : `yt-dlp`, HLS et DASH natifs, directs, sous-titres, audio seul.
- **Extraction guidée** : navigateur intégré qui détecte les médias d'une page et permet de se connecter aux sites.
- **Lancement au démarrage** : en option, l'application démarre sans fenêtre, dans la barre de menus.
- **Automatisation** : ligne de commande `tribal-grab`, schéma `tribalgrab://add?url=…`, scripts personnalisés, intégrations en JavaScript.
- **Extension Chromium** : confie les téléchargements de Chrome, Brave, Edge ou Arc à l'application.

Aucune télémétrie : l'application n'envoie rien à personne.

## Installation depuis les sources

Il faut macOS 13 ou plus récent, Rust stable, Node.js 24, pnpm et Xcode.

```bash
brew install ffmpeg node
pnpm install
pnpm tauri build --bundles app
```

L'application se trouve dans `target/release/bundle/macos/`. Au premier lancement, elle installe `yt-dlp` depuis ses publications officielles, avec vérification SHA256.

L'application compilée a une signature locale : sur un autre Mac, macOS la bloquera tant qu'elle n'est pas signée et notariée.

## Extension Chromium

1. Ouvrez `chrome://extensions` et activez le mode développeur.
2. Cliquez sur « Charger l'extension non empaquetée » et choisissez `extensions/chromium`.
3. Autorisez Chrome à ouvrir Tribal Grab à la première demande.

Un téléchargement qui exige une session ouverte sur un site peut échouer, car l'extension ne transmet pas les cookies du navigateur. Désactivez alors « Confier les téléchargements de Chrome » dans les options de l'extension, ou connectez-vous par le navigateur intégré.

## Ligne de commande

```bash
tribal-grab get https://exemple.com/fichier.zip
tribal-grab text "un texte contenant des liens"
tribal-grab queue
tribal-grab help
```

## Limites

- **DRM** : les contenus protégés par Widevine, FairPlay ou PlayReady ne sont pas déchiffrés et ne le seront pas.
- **macOS uniquement**, interface en français.

## Usage responsable

Tribal Grab est un outil. Vous restez responsable de ce que vous téléchargez : respectez le droit d'auteur et les conditions d'utilisation des sites concernés.

## Développement

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm tauri dev
```

- `crates/grab-core` : moteur, sans dépendance à Tauri. Son API est décrite dans [docs/API-CORE.md](docs/API-CORE.md).
- `crates/grab-cli` : ligne de commande.
- `src-tauri` et `src` : application de bureau.
- `extensions/chromium` : extension de navigateur.
- [docs/SPEC.md](docs/SPEC.md) : spécification.

## Versionnage

Le projet suit le [versionnage sémantique](https://semver.org/lang/fr/). Une seule commande aligne Cargo, npm, Tauri et l'extension, puis date le [journal des changements](CHANGELOG.md) :

```bash
pnpm version:bump patch
git commit -am "chore(release): v0.1.1"
git tag v0.1.1
```

- `main` : version stable, chaque version porte une étiquette `vX.Y.Z`.
- `develop` : intégration.
- `feature/*`, `fix/*` : travail en cours.
- Messages de commit au format [Conventional Commits](https://www.conventionalcommits.org/fr/).

## Dépendances externes

`yt-dlp` (Unlicense) et FFmpeg (LGPL ou GPL selon la compilation) sont des programmes séparés, lancés par l'application et non inclus dans ce dépôt. Les polices Inter et JetBrains Mono sont sous licence SIL OFL 1.1.

## Licence

[MIT](LICENSE) © 2026 Tribal Studio
