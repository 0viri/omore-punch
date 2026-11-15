# Bug Fix: Suppression de la persistance de taille fixe et filet ScrollArea pour l'UI

- **Slug**: ui-bottom-clipping
- **Fixed**: 2026-11-14
- **Assessment**: ./assessment.md
- **Status**: applied

## Summary

Déplacement de `editor_state: Arc<EguiState>` hors de `OmorePunchParams` (suppression de `#[persist = "editor-state"]`) vers `OmorePunch` afin qu'aucune session DAW sauvegardée avec une version antérieure (`820x560` ou `820x480`) ne puisse écraser la hauteur `820x620` requise par `3. DRUM BUS EQ` et les vu-mètres. Un `egui::ScrollArea::vertical().auto_shrink([false, false])` a également été ajouté dans le `CentralPanel` en filet de sécurité si un hôte contraint la hauteur en dessous de `620` px.

## Changes

| File | Change | Notes |
|------|--------|-------|
| `src/lib.rs` | modified | Retrait de `#[persist = "editor-state"]` / `editor_state` de `OmorePunchParams`, ajout de `editor_state` sur `OmorePunch` (`820x620`), `ScrollArea::vertical()` dans `draw_omore_punch_ui`, et ajout du test unitaire `test_editor_size_ignores_legacy_persisted_editor_state` |
| `CHANGELOG.md` | modified | Ajout de l'entrée `ui-bottom-clipping` sous `## [Unreleased]` -> `### Corrigé` |

## Diff Highlights

```rust
pub struct OmorePunch {
    params: Arc<OmorePunchParams>,
    editor_state: Arc<EguiState>,
    dsp: StereoPunchDsp,
    telemetry: Arc<MeterTelemetry>,
}
```

```rust
    egui::CentralPanel::default().show(egui_ctx, |ui| {
        let panel_height = ui.available_height();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ...
```

## Tests Added or Updated

- `src/lib.rs::tests::test_editor_size_ignores_legacy_persisted_editor_state` — Vérifie que `OmorePunch::default().editor_state.size()` vaut `(820, 620)`, que `serialize_fields()` n'émet plus `"editor-state"`, et que `deserialize_fields(...)` avec `{"size":[820,560]}` ou `{"size":[820,480]}` conserve `(820, 620)`.
- `src/lib.rs::tests::test_ui_layout_column_widths_fit_window` — Re-exécuté avec `ScrollArea::vertical()` dans `820x620` : aucun débordement horizontal ni vertical.

## Local Verification

- Commands run: `LC_ALL=C grep -n '[^[:print:][:space:]]' src/lib.rs` → exit code `1` (aucun caractère non-ASCII dans `src/lib.rs`)
- Commands run: `cargo test --release` → `16 passed; 0 failed` (tous les tests DSP, header, géométrie UI et non-régression `editor-state`)
- Commands run: `cargo run --package xtask --release -- bundle omore-punch --release` → compilation et génération réussies de `target/bundled/omore-punch.clap` et `target/bundled/omore-punch.vst3`
- Manual checks: Aucune vérification visuelle dans Reaper effectuée dans cette étape (à valider lors de `/speckit-bug-test` ou après installation locale).

## Deviations from Assessment

Aucune déviation : l'Option A (`editor_state` déplacé hors de `OmorePunchParams` vers `OmorePunch`) et le filet de sécurité `ScrollArea::vertical()` recommandés dans `assessment.md` ont été appliqués tels quels.

## Follow-ups

- Exécuter `/speckit-bug-test slug=ui-bottom-clipping` et tester le bundle réinstallé dans Reaper sous Linux (Debian 13) sur la session existante pour confirmer l'affichage complet de `3. DRUM BUS EQ` et des labels `-inf dB`.
