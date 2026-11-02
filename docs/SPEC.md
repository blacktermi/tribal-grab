# Tribal Grab : spécification

Gestionnaire de téléchargement pour macOS, écrit en Rust : tous les types de fichiers, vidéos et audio de plus de 1 800 sites, pause et reprise, plusieurs connexions par fichier.

## 1. Règles non négociables

1. **Code original.** Tout le code, les textes, les sons et les icônes de ce dépôt sont écrits pour ce projet. Rien n'est copié d'un autre logiciel.
2. **Pas de DRM.** Aucun contenu Widevine, FairPlay ou PlayReady n'est déchiffré. Le refus ne s'applique qu'à ce qui est réellement protégé, et formats ou pistes concernés sont simplement écartés. Une vidéo ne bascule en échec, avec un message clair, que s'il ne reste aucun format libre. `has_drm: "maybe"` n'est pas un refus : le téléchargement est tenté. Le chiffrement HLS `AES-128` à clé publique fait partie du format et reste pris en charge.
   Hors DRM, aucune limite n'est imposée : ni liste de sites bloqués, ni quota quotidien, ni plafond de résolution par défaut.
3. **Pas de contournement anti-bot écrit ici.** Pas de générateur PO Token ou BotGuard, pas de réimplémentation de SABR, pas de résolution de CAPTCHA. YouTube passe par `yt-dlp`.
4. **Pas de télémétrie.** Aucune statistique, aucun envoi vers un serveur de Tribal ou d'un tiers.
5. **Secrets.** Mots de passe de proxy dans le trousseau macOS, jamais dans SQLite ni dans les journaux. Les cookies et les URL signées n'apparaissent jamais dans les journaux.

## 2. Pile technique

- Rust 1.94, Cargo workspace, édition 2024.
- Moteur : `tokio`, `reqwest` (rustls), `rusqlite` (bundled), `m3u8-rs`, `quick-xml` (DASH, TTML), `scraper`, `rquickjs`, `serde`, `thiserror`, `tracing`.
- Application : Tauri 2 stable (pas la 3 alpha), React 19, TypeScript, Vite, Tailwind CSS v4, shadcn/ui, Zustand, React Hook Form, Zod, pnpm uniquement.
- Outils externes : `yt-dlp`, `ffmpeg`, `ffprobe`, un runtime JS (`deno` ou `node`) pour `yt-dlp`.
- Interface en français uniquement.

## 3. Structure

```
Tribal Grab/
├── Cargo.toml              # workspace
├── crates/grab-core/       # moteur, sans dépendance à Tauri
├── crates/grab-cli/        # binaire tribal-grab
├── src-tauri/              # application de bureau
├── src/                    # interface React
├── extensions/chromium/    # extension Chrome, Brave, Edge, Arc
└── docs/SPEC.md
```

## 4. Moteur (`grab-core`)

### Extraction

Ordre de résolution d'une URL :

1. Intégration personnalisée dont la regex correspond.
2. Fichier direct : média reconnu, ou tout contenu qui n'est pas une page web. Un fichier de n'importe quel type est enregistré tel quel, sous son vrai nom.
3. Manifeste `.m3u8` ou `.mpd`.
4. `yt-dlp -J` (environ 1 800 sites, playlists avec `--flat-playlist`).
5. Détection native dans le HTML : `<video>`, `<source>`, `og:video`, JSON-LD, URL `.m3u8`, `.mpd`, `.mp4` dans les scripts.
6. Lien non pris en charge : ignoré avec avertissement, ou ouvert en extraction guidée selon le réglage.

### Téléchargeurs

- **HTTP** : plusieurs connexions avec `Range`, reprise, limite de débit, en-têtes et cookies fournis par l'extraction.
- **HLS** natif : variantes, audio séparé, `AES-128`, segments en parallèle, reprise.
- **DASH** natif : `SegmentTemplate`, `SegmentList`, `SegmentBase`, audio et vidéo séparés.
- **Repli** : `yt-dlp`, puis `ffmpeg`, si le natif échoue ou pour les protocoles inconnus.
- **Direct** : enregistrement de flux en direct avec `ffmpeg`, arrêt à une date, à la demande, ou si le disque est presque plein. Conteneur MPEG-TS en option.

Téléchargements partiels : dossier `<nom>.tgpart` à côté de la destination, avec `manifest.json` et les parties. Ils survivent à un arrêt et se reprennent. Fichier final écrit de façon atomique.

### File d'attente

États : `Queued`, `Extracting`, `NeedsInput`, `Scheduled`, `Downloading`, `Recording`, `PostProcessing`, `Paused`, `Finished`, `Failed`, `Cancelled`.

- Démarrage automatique ou manuel, nombre de téléchargements simultanés, connexions par téléchargement (avertissement au-delà de 8), limite en ko/s par téléchargement.
- Priorité haute, normale, basse. Tri par ordre, site, playlist, progression, nom, ascendant ou descendant.
- Pause, reprise, relance, annulation, individuelles ou globales. Retrait des terminés ou des inactifs. Réinitialisation de la file.
- Date de début différée, date d'arrêt pour le direct.
- Nouvelles tentatives avec attente croissante.
- Doublon déjà téléchargé : demander, retélécharger ou ignorer.
- Mode automatisé : aucune question, valeurs par défaut.
- À la fin de la file : rien, quitter l'application ou mettre le Mac en veille.
- Restauration de la file au lancement.

### Qualité et formats

Résolution maximale (144p, 240p, 360p, 480p, 720p, 1080p, 1440p, 4K, 8K, libre), meilleure qualité, ou fenêtre de choix. Préférer MP4, forcer MP4, format d'origine. Audio seul en M4A, MP3 ou format d'origine. Piste audio par langue et libellés. Si aucune qualité ne respecte la limite, prendre la plus petite.

### Post-traitement

Valeurs par défaut et par téléchargement : `Default`, `None`, `Mp4`, `AudioOnly`, `SendToApp`, `Custom`.

- Fusion audio et vidéo, conversion MP4, ré-encodage forcé, suppression ou conservation de l'original.
- Toujours la meilleure qualité disponible, jamais un repli vers le bas. Ce que QuickTime ne lit pas (VP9, AV1, Opus) est converti à la même résolution, et l'original est gardé à côté dans `<titre> (original).mkv` pour VLC.
- Métadonnées : titre, auteur, date, genre, type de média, miniature intégrée.
- Miniature séparée en JPEG. JSON de métadonnées séparé.
- Script personnalisé (`/bin/zsh`) par fichier ou en fin de file, avec les variables `TG_FILE`, `TG_TITLE`, `TG_URL`, `TG_SITE`, `TG_PLAYLIST`.
- Envoi à une application par `open -a`. Ajout à Music ou TV par `osascript`.

### Sous-titres

Modes : aucun, si disponibles, obligatoires, sous-titres seuls. Un seul fichier ou plusieurs. Langues préférées ordonnées, libellés, regex, inclusion des sous-titres automatiques. Lecture de WebVTT, SRT, TTML, DFXP, SMPTE-TT et sortie en `.srt` ou `.vtt`, ou intégrés au MP4 et au MKV.

### Noms de fichiers

Modèle avec variables : `{title}` (obligatoire), `{id}`, `{site}`, `{host}`, `{uploader}`, `{upload_date}`, `{timestamp}`, `{playlist}`, `{playlist_index}`, `{playlist_count}`, `{resolution}`, `{ext}`, `{original_name}`, `{last_path}`, `{download_id}`. Caractères interdits retirés, longueur limitée, collisions résolues. Destination par défaut, par téléchargement, ou demandée à chaque fois. Vérification de l'espace disque.

### Réseau

Proxy HTTPS ou SOCKS avec identifiants, agent utilisateur, délais, téléchargement fragmenté activable, option `--xff` de `yt-dlp` pour les vidéos limitées à un pays, cookies d'un navigateur par `--cookies-from-browser`, et cookies de la session de connexion intégrée au format Netscape.

### Historique

SQLite. Grille ou tableau, tri, recherche, effacement manuel ou automatique (1 semaine, 1, 3 ou 6 mois, 1 an, jamais), retrait des fichiers supprimés, retéléchargement, ouverture, affichage dans le Finder. Export XML, CSV, JSON. Import CSV, XML.

### Recherche et sites

Recherche par les préfixes de `yt-dlp` (YouTube, SoundCloud, autres si disponibles), moteurs activables un par un. Liste des sites depuis `yt-dlp --list-extractors`, avec filtre et compteur.

### Intégrations personnalisées

Fichier `.tgi` (JSON) : nom, regex d'URL, code JavaScript, délai, compte requis, lien d'exemple, commentaire. Exécution dans `rquickjs`, sans accès au disque ni aux processus. API : `grab.fetch`, `grab.log`, `grab.result`. Installation, activation, édition, console, import et export de fichiers.

### Outils

Recherche de `yt-dlp`, `ffmpeg`, `ffprobe`, `deno` et `node` : réglage explicite, puis `~/Library/Application Support/Tribal Grab/bin`, puis `/opt/homebrew/bin`, `/usr/local/bin`, `~/.local/node/*/bin`, puis `PATH`. Une application lancée depuis le Finder n'hérite pas du `PATH`. Mise à jour de `yt-dlp` depuis ses publications officielles GitHub, avec vérification SHA256.

## 5. Application (`src-tauri` et `src`)

- **Fenêtre principale** : file avec miniature, titre, site, état, progression, débit, temps restant, compteurs. Glisser-déposer, collage direct, menu contextuel, barre d'ajout.
- **Dialogues** : choix de qualité, filtre de liens (tout, rien, inverser, N premiers, N derniers, plage, plus gros fichier, reconnus, recherche, inverser l'ordre), choix des sous-titres, ajout en playlist ou individuel, import de fichier, détails d'un lien.
- **Extraction guidée** : navigateur intégré avec adresse, précédent, suivant, recharger, muet, barre latérale des médias détectés, images, marque-pages, historique, envoi à la file. Les pages chargées ne peuvent appeler qu'une seule commande, `guided_report`.
- **Connexion aux sites** : navigateur de connexion, cookies récupérés, gestion des données par site (cookies, stockage local, stockage de session).
- **Préférences** : Général, Fichiers, Qualité, Post-traitement, Sous-titres, Réseau, Sons et notifications, Historique, Intégrations, Extensions, Recherche, Moteurs, Avancé, Apparence.
- **macOS** : icône de barre de menus avec popover, icône du Dock masquable, notifications, sons système (`/System/Library/Sounds`), thème clair, sombre ou système, instance unique, ouverture de `.tgpart` et `.tgi`, schéma `tribalgrab://add?url=…`.
- **Automatisation** : `tribal-grab get <url>`, `tribal-grab text <texte>`, `tribal-grab queue start`, utilisables depuis Raccourcis avec « Exécuter un script shell ».
- **Extension Chromium** : bouton d'envoi, option de fermeture de l'onglet.

## 6. Choix de périmètre

| Sujet | Décision | Raison |
|---|---|---|
| YouTube et sites de vidéo | Passent par `yt-dlp` | Environ 1 800 sites maintenus en continu, et règle 3 |
| Statistiques, classements communautaires | Absents | Règle 4 |
| Registre d'intégrations en ligne | Import de fichiers | Exige un serveur |
| Synchronisation entre Mac | Export et import | Exige un compte développeur Apple |
| Extension Safari, AppleScript, Raccourcis natifs | Extension Chromium, schéma d'URL et ligne de commande | Exigent des cibles Xcode natives |
| Langues | Français | Portée actuelle du projet |

## 7. Critères de fin

- `cargo test --workspace` et `cargo clippy --workspace -- -D warnings` passent.
- Vraies URL téléchargées et lues avec `ffprobe` : fichier MP4 direct, HLS, DASH, vidéo YouTube, audio seul en MP3, sous-titres, reprise après coupure.
- `pnpm tauri build` produit une application qui s'ouvre, télécharge et affiche sa progression.
