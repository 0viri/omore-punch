# Phase 0 Research: Remplacement du bouton rapide Kick Safe 90Hz par Snare Safe 200Hz

**Feature**: `001-snare-safe-200hz`
**Date**: 2026-11-14

## Decisions & Findings

### 1. Substitution directe du bouton rapide dans `draw_omore_punch_ui`
- **Decision**: Remplacer le bouton `[Kick Safe 90Hz]` par `[Snare Safe 200Hz]` dans `src/lib.rs` (bloc 4 `draw_omore_punch_ui`, autour des lignes 1161-1175) et régler la valeur cible transmise à `setter.set_parameter(&params.sc_hpf_hz, ...)` sur `200.0` (au lieu de `90.0`), encadrée par `begin_set_parameter` et `end_set_parameter`.
- **Rationale**: Répond exactement à la demande utilisateur (« Changer le bouton "Kick safe 90Hz" en "Snare Safe 200Hz" (technique Cristian Kohle) »), conserve la disposition horizontale compacte des 3 boutons rapides (`[Punch 3ms / 80ms]`, `[Snare Safe 200Hz]`, `[Auto Makeup: ON/OFF]`) dans la colonne de gauche (`480 px`), et respecte le protocole d'automation `nih-plug`.
- **Alternatives considered**:
  - *Ajouter un quatrième bouton à côté de `[Kick Safe 90Hz]`* : Rejeté car la demande spécifie explicitement de remplacer le bouton existant et l'ajout d'un 4e bouton sur la même ligne dépasserait la largeur de la colonne de gauche (`480 px`).

### 2. Libellé et infobulle 100 % ASCII (Principe II de la Constitution)
- **Decision**:
  - Libellé du bouton : `"[Snare Safe 200Hz]"` (police `monospace().strong()`).
  - Texte d'infobulle (`on_hover_text`) : `"Set Sidechain HPF to 200.0 Hz so Kick and Snare body (up to 200 Hz) do not pump the Drum Bus"`.
- **Rationale**: 100 % ASCII imprimable (aucun accent ni caractère spécial), explique clairement l'intérêt musical de la coupure à `200.0 Hz` sur un bus batterie (technique Cristian Kohle : laisser passer à la fois le grave de la grosse caisse et le corps fondamental de la caisse claire sans écraser le compresseur).
- **Alternatives considered**:
  - *Mentionner un nom propre avec caractères spéciaux ou un texte trop long* : Rejeté pour garder une infobulle concise, purement technique et garantie 100 % ASCII.

### 3. Conservation intégrale du paramètre `sc_hpf_hz` et du moteur DSP
- **Decision**: Conserver sans modification l'identifiant `#[id = "sc_hpf_hz"]`, la plage `20.0..=250.0 Hz` (`FloatRange::Skewed`) et la valeur par défaut `20.0 Hz`, ainsi que l'implémentation DSP du filtre passe-haut de sidechain 1er ordre (`6 dB/oct`) dans `StereoPunchDsp`.
- **Rationale**: `200.0 Hz` est déjà compris dans la plage `[20.0, 250.0]`. Ne pas toucher à l'ID ni à la plage garantit 100 % de rétrocompatibilité avec les sessions Reaper existantes (VST3 et CLAP). Aucune allocation mémoire, aucun verrou et aucun calcul trigonométrique par échantillon n'est introduit sur le thread audio (Principes III et IV).
- **Alternatives considered**:
  - *Modifier la valeur par défaut du paramètre `SC HPF` à `200.0 Hz`* : Rejeté car la demande porte uniquement sur le bouton d'action rapide, et le filtre de détection doit rester désactivé (`20.0 Hz`) par défaut à l'insertion initiale du plugin.

### 4. Couverture par les tests unitaires DSP (Principe V de la Constitution)
- **Decision**: Vérifier numériquement dans le module `tests` de `src/lib.rs` le comportement du filtre de sidechain à `200.0 Hz` face à un signal basse fréquence (`50 Hz`, grosse caisse) et bas-médium (`100 Hz` - `150 Hz`, corps de caisse claire), en s'assurant que `sc_hpf_hz: 200.0` réduit significativement la réduction de gain par rapport à `sc_hpf_hz: 20.0` tout en préservant l'amplitude du signal en sortie.
- **Rationale**: Pour un filtre passe-haut RC du 1er ordre à $f_c = 200\text{ Hz}$, l'atténuation dans le détecteur est de $-12{,}3\text{ dB}$ à $50\text{ Hz}$, $-7{,}0\text{ dB}$ à $100\text{ Hz}$ et $-4{,}4\text{ dB}$ à $150\text{ Hz}$. Avec un seuil à $-18\text{ dB}$ et un ratio de $4:1$, cette atténuation diminue nettement la réduction de gain et préserve le punch du couple Kick/Snare.
- **Alternatives considered**:
  - *Aucun changement dans les tests* : Moins pertinent ; tester explicitement la fréquence cible de `200.0 Hz` du préréglage `[Snare Safe 200Hz]` documente et verrouille le comportement attendu par la spécification (SC-003).

### 5. Synchronisation de la documentation utilisateur
- **Decision**: Mettre à jour `README.md`, `USER_GUIDE.md`, `GEMINI.md` (ligne 62, section 4) et la section `## [Unreleased]` de `CHANGELOG.md` pour refléter `[Snare Safe 200Hz]`. Ne pas modifier `.specify/memory/constitution.md` (interdit hors `/speckit-constitution`).
- **Rationale**: Garantit la cohérence entre l'interface du plugin et la documentation livrée aux utilisateurs tout en respectant les règles de gouvernance du dépôt.
