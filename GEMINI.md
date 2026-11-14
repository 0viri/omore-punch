# GEMINI.md - Directives Strictes du Projet Omore Punch

Ce fichier définit les règles d'architecture, de DSP temps réel, d'interface graphique, de versioning et de workflow Git/Spec-Kit du projet **Omore Punch** (`omore-punch`).
**IMPÉRATIF :** Tu dois lire et respecter intégralement ce document avant toute modification du code source.
**Référence :** La constitution du projet (`.specify/memory/constitution.md`) fait foi. Ce document la détaille et doit rester cohérent avec elle : une règle modifiée ici doit l'être aussi dans la constitution (via `/speckit-constitution`), et inversement. Never modify `.specify/memory/constitution.md` outside an explicit invocation of `/speckit-constitution` by the user.

## 1. Identité & Stack Technique
- **Nom du plugin :** `Omore Punch` (`omore-punch`)
- **Rôle :** Compresseur parallèle et Clipper de Bus Batterie Stéréo (Stereo Drum Bus).
- **Stack :** Rust (Edition 2021), `nih-plug` et `nih_plug_egui`.
- **Formats cibles :** Export simultané **VST3** (`nih_export_vst3!`) et **CLAP** (`nih_export_clap!`).
- **OS & DAW cibles :** 100 % compatible avec **Reaper** sous **Linux (Debian 13)** et **Windows 11**.

## 2. Règles Architecturales Absolues
1. **Architecture Monolithique (`src/lib.rs`) & Économie de Tokens :**
   - Tout le code du plugin (moteur DSP, paramètres, télémétrie atomique, UI `egui`, exports VST3/CLAP et tests unitaires) DOIT rester dans le fichier unique `src/lib.rs`. Ne crée pas de sous-modules éparpillés (`dsp.rs`, `gui.rs`, etc.).
   - Les outils de build (`xtask/`, `build.sh`, `build.ps1`) ne sont pas du code du plugin et ne sont pas concernés par cette règle.
   - `src/lib.rs` est organisé en quatre blocs isolés (voir `ARCHITECTURE.md`) : tout nouveau code DOIT rejoindre le bloc auquel il appartient. Le moteur DSP (`StereoPunchDsp`, `DspConfig`) ne DOIT dépendre ni de l'UI ni du wrapper hôte.
   - **Économie de tokens sur `src/lib.rs` :** Ne lis jamais `src/lib.rs` en entier d'un bloc. Utilise `grep -n` pour localiser les structures ou fonctions à modifier, puis lis uniquement des fenêtres ciblées (100 à 250 lignes).
2. **100 % Pure ASCII dans `src/lib.rs` (Zéro Emoji / Zéro Unicode non-ASCII) :**
   - Il est strictement interdit d'utiliser des emojis ou des caractères accentués/Unicode non-ASCII dans `src/lib.rs` (commentaires et chaînes UI inclus) afin d'éviter l'affichage de carrés vides (`□`) dans `egui` sous Linux.
   - Vérifie systématiquement avec : `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs` (qui ne doit rien retourner). La CI exécute ce contrôle à chaque push et pull request vers `main`.
   - Cette règle ne vise que `src/lib.rs` : la documentation Markdown (`*.md`, specs Spec-Kit comprises) peut contenir des accents.
3. **Versioning Dynamique & Cycle de Release :**
   - Le numéro de version est défini uniquement dans `Cargo.toml`.
   - Utilise toujours `env!("CARGO_PKG_VERSION")` dans les métadonnées du plugin et dans l'en-tête de l'interface graphique `egui`. Ne code jamais un numéro de version en dur dans `src/lib.rs`.
   - **Une version par release, pas par correctif :** Un fix (`/speckit-bug-fix`) ou une feature (`/speckit-implement`) ne change jamais la version dans `Cargo.toml`.
   - **Une entrée `CHANGELOG.md` par changement :** Chaque fix et chaque feature ajoute son entrée sous `## [Unreleased]` dans `CHANGELOG.md` (catégories Keep a Changelog : `Added` / `Ajouté`, `Changed` / `Modifié`, `Fixed` / `Corrigé`, `Removed` / `Supprimé`).
   - **Versions de développement (`-dev`) :** Entre deux releases, la version dans `Cargo.toml` porte le suffixe `-dev` (ex. `0.2.1-dev`). L'UI l'affiche telle quelle, ce qui distingue immédiatement un build de test d'une release publiée.
   - **Procédure de release (uniquement à la demande explicite de l'utilisateur) :**
     1. Choisir le numéro SemVer :
        - `patch` si `[Unreleased]` ne contient que des correctifs ;
        - `minor` s'il contient une fonctionnalité ou, tant qu'on est en `0.x`, un changement cassant (état DAW, IDs de paramètres, VST3 class ID / CLAP ID).
     2. Préparer les fichiers dans une branche `release/vX.Y.Z` :
        - renommer `## [Unreleased]` en `## [X.Y.Z] - YYYY-MM-DD` et ouvrir une nouvelle section `## [Unreleased]` vide en haut de `CHANGELOG.md` ;
        - passer `Cargo.toml` à `X.Y.Z` (et mettre à jour `Cargo.lock`).
     3. Ouvrir et fusionner la PR `chore(release): vX.Y.Z` dans `main`, puis poser un **tag annoté** `vX.Y.Z` sur le commit de fusion (`git tag -a vX.Y.Z -m "Omore Punch X.Y.Z" <commit>`).
        - Pousser le tag (`git push origin vX.Y.Z`) déclenche le job CI `release` (`.github/workflows/build.yml`) qui vérifie la cohérence tag / `Cargo.toml` / `CHANGELOG.md`, compile Linux et Windows, génère les `.zip` + `SHA256SUMS.txt` et publie la GitHub Release.
     4. Passer ensuite à la version de développement suivante `X.Y.(Z+1)-dev` dans un commit/PR séparé (`chore: start X.Y.(Z+1)-dev`).

## 3. Contraintes Strictes du Thread Audio Temps Réel (`process`)
1. **Zéro Allocation et Zéro Désallocation Mémoire :**
   - Interdiction absolue d'utiliser `Vec`, `String`, `Box`, `format!`, `.to_string()`, ou `.clone()` sur des types alloués sur le tas dans `process()` et dans tout code qu'il appelle (dont `StereoPunchDsp`). Aucune désallocation implicite (`drop`) de valeur sur le tas ne doit non plus avoir lieu sur le thread audio.
   - La feature `assert_process_allocs` de `nih-plug` DOIT rester activée dans `Cargo.toml`. Elle fait planter le plugin en cas d'allocation, mais uniquement en build debug et à l'intérieur du wrapper `process` de `nih-plug` (par exemple un bundle compilé sans `--release` et chargé dans Reaper).
   - Les tests unitaires appellent le DSP directement et la CI tourne en `--release` : `cargo test --release` ne détecte donc aucune allocation (d'où la règle 3.5).
2. **Zéro Verrou Bloquant :**
   - Interdiction absolue d'utiliser `Mutex`, `RwLock` ou des canaux bloquants dans `process()` et dans tout code qu'il appelle (dont `StereoPunchDsp`).
   - Toute communication entre le thread audio et l'UI doit passer par les paramètres `nih-plug` ou par `MeterTelemetry` (`AtomicU32` via `to_bits`/`from_bits` et `AtomicBool` avec `Ordering::Relaxed`).
3. **Précalcul des Fonctions Coûteuses :**
   - N'appelle jamais `.exp()`, `.sin()`, `.cos()` ou `.tan()` à chaque échantillon si les paramètres (`sample_rate`, `attack_ms`, `release_ms`, fréquences de filtres) n'ont pas changé. Mets en cache la dernière valeur connue et ne recalcule les coefficients que lors d'une variation.
4. **Sanitisation & Safety Clamp Anti Auto-Mute Reaper :**
   - Chaque échantillon entrant et sortant (ainsi que les variables d'état internes d'enveloppe et de filtres) doit être protégé contre `NaN`, `Inf` et les dénormaux.
   - La sortie finale doit toujours passer par `sanitize_sample()` qui remplace toute valeur non-finie par `0.0` et borne le signal dans `[-4.0, 4.0]`.
   - Un test unitaire DOIT continuer à couvrir ce comportement (actuellement `test_nan_inf_and_safety_clamp`).
5. **Relecture Obligatoire :**
   - Tout changement dans `process()` ou `StereoPunchDsp` DOIT être relu au regard des règles 3.1 à 3.4.

## 4. Standards de l'Interface Graphique (`egui`)
- **Style & Palette (`apply_omore_dark_theme`) :**
  - Thème sombre inspiré d'`Omore Sampler` : fond de panneau `rgb(18, 19, 24)`, fond de fenêtre `rgb(24, 26, 32)`, fond extrême/jauges `rgb(10, 11, 14)`, widgets inactifs `rgb(38, 41, 52)`, survol `rgb(56, 61, 78)`, accentuation ambre/or `rgb(225, 145, 25)` / `rgb(245, 185, 35)`.
- **Texte & Typographie :**
  - 100 % ASCII strict (règle 2.2) avec police `monospace()` pour les étiquettes de paramètres, valeurs numériques et boutons d'action entre crochets (`[Punch 3ms / 80ms]`, `[Kick Safe 90Hz]`, `[Auto Makeup: ON]`, `[Soft Clip]`, `[Reset Peak]`, `[CLIP: OK]`).
- **Structure en 2 Colonnes & En-tête :**
  - Bandeau supérieur (`TopBottomPanel::top`) affichant `OMORE PUNCH v<VERSION>  |  STEREO DRUM BUS COMPRESSOR & CLIPPER` à gauche et le témoin/bouton d'acquittement `[CLIP: OK]` / `[CLIP: ACTIVE - CLICK TO RESET]` à droite.
  - Colonne gauche dédiée aux contrôles (`1. COMPRESSOR & ENVELOPE BALLISTICS` et `2. OUTPUT CLIPPER & PARALLEL MIX`), chaque paramètre utilisant `draw_param_row` (label aligné, bouton `-`, slider, bouton `+`, lecture numérique formatée, bouton `Reset` encadré par `begin_set_parameter` / `set_parameter` / `end_set_parameter`).
  - Colonne droite dédiée au metering (`3. GAIN REDUCTION & STEREO BUS METERING`) : afficheurs numériques `GR` et `Max GR`, bouton `[Reset Peak]`, vu-mètres stéréo `In L/R` et `Out L/R`, et jauge verticale `GR (0 to -24 dB)` descendante style Reaper avec ligne de maintien de crête (`Peak GR Hold`).
- **Rafraîchissement :**
  - Appel à `egui_ctx.request_repaint()` en fin de rendu pour garantir un affichage fluide (60 FPS) des vu-mètres tant que la fenêtre de l'éditeur est ouverte.

## 5. Tests DSP Obligatoires
- Tout comportement DSP DOIT être vérifié par des tests unitaires dans le module `tests` de `src/lib.rs`. Les tests existants (compression stéréo liée, HPF de sidechain, plafond du clipper, mix dry/wet, sanitisation et safety clamp) DOIVENT continuer à passer.
- Toute fonctionnalité DSP nouvelle ou modifiée DOIT ajouter ou mettre à jour au moins un test qui vérifie son comportement numérique (exemple : `test_sidechain_hpf_kick_immunity`, livré avec le HPF de sidechain en v0.2.0).
- `cargo test --release` DOIT passer sous Linux et Windows avant toute arrivée sur `main` et avant toute release.

## 6. Contrôles Qualité & Livraison
- Avant toute arrivée sur `main` et avant tout tag de release (`v*`), ces contrôles DOIVENT passer (ils reprennent `.github/workflows/build.yml`) :
  1. Contrôle ASCII de `src/lib.rs` (règle 2.2) :
     ~~~bash
     LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs
     ~~~
  2. Tests unitaires DSP en mode Release (section 5) :
     ~~~bash
     cargo test --release
     ~~~
  3. Génération des bundles VST3 et CLAP sous Linux x86_64 et Windows x86_64 :
     ~~~bash
     cargo run --package xtask --release -- bundle omore-punch --release
     ~~~
     DOIT produire `omore-punch.vst3` et `omore-punch.clap` dans `target/bundled/`.
- Chaque release DOIT incrémenter la version dans `Cargo.toml` (règle 2.3) et consigner ses changements notables dans `CHANGELOG.md`.

## 7. Gestion des Bugs : `/speckit-bug-assess` Obligatoire
- **Diagnostic d'abord :** Aucun bug, régression, crash, glitch audio ou comportement inattendu ne doit être corrigé (aucune modification de code, de doc ou de commit) tant que `/speckit-bug-assess` n'a pas produit `.specify/bugs/<slug>/assessment.md`.
- **Pas de correctif sauvage :** Si l'utilisateur signale un bug en texte libre, ne modifie aucun fichier et invite-le à lancer `/speckit-bug-assess`.
- **Chemin unique de correction :** Les changements de code qui corrigent un bug passent par `/speckit-bug-fix` (qui consomme `assessment.md` et produit `fix.md`), puis par `/speckit-bug-test` (`test.md`).
- **Exception (code en cours d'écriture uniquement) :** Les erreurs de compilation et tests en échec introduits par le code écrit pendant la tâche `/speckit-implement` en cours (non encore livrée) font partie de l'implémentation. Tout défaut dans du code déjà livré sur `main` relève de `/speckit-bug-assess`.

## 8. Communication & Langue
- **Conversation en français avec tutoiement :** Adresse-toi toujours à l'utilisateur avec « tu », en français, avec un ton direct, collaboratif et technique.
- **Documentation et messages Git :** Les messages de commit, titres/descriptions de PR et spécifications suivent un format clair et structuré (Conventional Commits). Le code Rust, ses commentaires et les textes UI dans `src/lib.rs` sont 100 % en anglais et en ASCII pur.

## 9. Git : Scripts de Build sans Git, Commits Atomiques & PRs
- **Scripts de build sans Git :** `build.sh` (Linux) et `build.ps1` (Windows) compilent, exécutent les tests et installent les bundles `.vst3` et `.clap` localement. N'y ajoute jamais de commandes Git (`git add`, `git commit`, `git tag`, `git push`) : installer un build de test et publier l'historique sont deux actions distinctes.
- **Commits uniquement sur demande :** L'agent ne fait de commit et de push que lorsque l'utilisateur le demande explicitement, et jamais de `git push --force` sur `main`.
- **Branches et Pull Requests (pas de commit direct sur `main`) :**
  - Chaque travail part d'une branche créée depuis `main` à jour (`git switch main`, `git pull --ff-only`, `git switch -c <branch>`). Noms de branches :
    - `fix/<slug>` pour un bug Spec-Kit ;
    - `feat/<specs/ folder>` pour une fonctionnalité ;
    - `docs/<topic>`, `chore/<topic>`, `ci/<topic>` ;
    - `release/vX.Y.Z` pour une release.
  - Une branche = une intention = une Pull Request, fusionnée en « Squash and merge ».
  - Une PR qui ne touche que la documentation (`*.md`, `.specify/`, `specs/`, `.claude/`, `.agents/`) ne déclenche pas la compilation CI (`paths-ignore`).
- **Staging explicite :** Ajoute les fichiers par leur nom (`git add <fichier>...`) après avoir vérifié `git status` et `git diff --staged`. N'utilise jamais `git add -A` ni `git add .`.
- **Messages Conventional Commits :** `<type>: <summary>` (types : `fix`, `feat`, `perf`, `refactor`, `test`, `docs`, `chore`, `ci`). Résumé à l'impératif, 72 caractères max, sans point final.

## 10. Spec-Kit sur Deux PC (Linux & Windows)
L'utilisateur travaille sur deux PC (un PC sous Windows et un PC sous Linux). Chaque machine a son propre CLI `specify`, tandis que le dépôt enregistre une seule version Spec-Kit (`.specify/init-options.json`, `.specify/integration.json`, `.specify/integrations/*.manifest.json`).
- **CLI figé sur un tag de release publié :** `specify` s'installe avec `uv tool` depuis un tag de release Spec-Kit (`@vX.Y.Z`), identique sur les deux PC et correspondant à la version enregistrée dans le dépôt :
  ~~~bash
  uv tool install specify-cli --force --from git+https://github.com/github/spec-kit.git@vX.Y.Z
  ~~~
  - Ne jamais installer depuis la branche `main` de Spec-Kit sans tag, ni utiliser `uv tool upgrade specify-cli` sans tag.
  - Vérifier avec `specify version` et `specify integration status`.
- **Bascule d'agent :** Spec-Kit est installé pour un seul agent à la fois (`specify integration switch agy` ou `specify integration switch claude`).
- **Pointeur de feature courante :** `.specify/feature.json` (ignoré par Git) indique le dossier `specs/<feature>/` actif sur chaque PC. Avant d'exécuter une commande `/speckit-*` sur une feature reprise depuis l'autre PC, vérifier que `.specify/feature.json` pointe bien vers le bon dossier `specs/<folder>`.
