# Journal des changements

Toutes les évolutions notables de Tribal Grab sont notées ici. Le format suit
[Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) et les numéros de version suivent
[le versionnage sémantique](https://semver.org/lang/fr/).

## [Non publié]

## [0.1.0] - 2026-11-02

Première version publique.

### Ajouté

- Téléchargement de tous les types de fichiers, enregistrés tels quels sous leur vrai nom.
- Vidéo et audio de plus de 1 800 sites par `yt-dlp`, HLS et DASH natifs, enregistrement des directs.
- Plusieurs connexions par fichier, avec adaptation automatique aux serveurs qui les limitent.
- Pause et reprise à l'octet près, y compris après une coupure ou un arrêt brutal.
- Taille reçue et taille totale affichées pendant le téléchargement, y compris quand la vidéo et l'audio sont séparés.
- Toujours la meilleure qualité disponible. Ce que QuickTime ne lit pas est converti à la même résolution par
  l'encodeur matériel du Mac, et l'original est gardé à côté dans un fichier `.mkv`.
- Sous-titres, métadonnées, miniatures, post-traitement, scripts personnalisés.
- Historique, recherche, doublons, import et export, intégrations personnalisées en JavaScript.
- Navigateur intégré d'extraction guidée, connexion aux sites.
- Application de bureau avec barre de menus, ligne de commande `tribal-grab`, schéma `tribalgrab://`.
- Extension Chromium qui confie les téléchargements de Chrome à l'application.

[Non publié]: https://github.com/blacktermi/tribal-grab/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/blacktermi/tribal-grab/releases/tag/v0.1.0
