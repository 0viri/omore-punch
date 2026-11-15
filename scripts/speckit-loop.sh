#!/usr/bin/env bash
set -euo pipefail

# --- Configuration ---
AGY_BIN="${AGY_BIN:-agy}"
MODEL_STANDARD="gemini-4-hydrogen-high"
MODEL_DSP="gemini-4-hydrogen-xhigh"
WIREPROXY_URL="${WIREPROXY_URL:-socks5h://127.0.0.1:1080}"

MAX_RETRIES=2
GIT_AUTO_COMMIT=true
DRY_RUN=false
TASKS_ARG=""

# Mots-clés déclenchant automatiquement gemini-4-hydrogen-xhigh + cargo test
DSP_KEYWORDS='dsp|compressor|compresseur|eq|biquad|oversampling|process\(\)|filter|filtre|envelope|enveloppe|smoother|sidechain'

# --- Analyse des arguments ---
while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run|-n)
            DRY_RUN=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [--dry-run] [chemin/vers/tasks.md]"
            exit 0
            ;;
        *)
            TASKS_ARG="$1"
            shift
            ;;
    esac
done

TASKS_FILE="${TASKS_ARG:-$(find specs -maxdepth 2 -name "tasks.md" 2>/dev/null | sort -V | tail -n 1)}"

if [[ -z "$TASKS_FILE" || ! -f "$TASKS_FILE" ]]; then
    echo "❌ Erreur : impossible de trouver tasks.md. Passe-le en argument : $0 [--dry-run] specs/003-drum-bus-eq/tasks.md"
    exit 1
fi

SPEC_DIR="$(dirname "$TASKS_FILE")"
echo "📂 Dossier Spec-Kit détecté : $SPEC_DIR"

# Export des variables proxy pour agy (sauf en dry-run)
if [[ "$DRY_RUN" == "false" ]]; then
    export ALL_PROXY="$WIREPROXY_URL"
    export HTTPS_PROXY="$WIREPROXY_URL"
    export HTTP_PROXY="$WIREPROXY_URL"
    export all_proxy="$WIREPROXY_URL"
    export https_proxy="$WIREPROXY_URL"
    export http_proxy="$WIREPROXY_URL"
    echo "🌐 Proxy SOCKS5 actif : $WIREPROXY_URL"
else
    echo "🧪 [MODE DRY-RUN] Simulation active : aucune session agy, compilation cargo ou commit git ne sera exécuté."
fi

mapfile -t PENDING_STORIES < <(
    grep -E '^\s*-\s*\[\s\]' "$TASKS_FILE" \
    | grep -oE '\[US[0-9]+\]' \
    | tr -d '[]' \
    | awk '!seen[$0]++'
)

if [[ ${#PENDING_STORIES[@]} -eq 0 ]]; then
    echo "✅ Toutes les User Stories de $TASKS_FILE sont déjà terminées !"
    exit 0
fi

echo "📋 User Stories en attente : ${PENDING_STORIES[*]}"
echo "------------------------------------------------------------"

for US in "${PENDING_STORIES[@]}"; do
    US_CONTEXT=$(grep -iE "(#.*\[$US\]|\[$US\])" "$TASKS_FILE" || true)
    TASK_COUNT=$(grep -cE "^\s*-\s*\[\s\].*\[$US\]" "$TASKS_FILE" || true)

    if echo "$US_CONTEXT" | grep -qiE "$DSP_KEYWORDS"; then
        SELECTED_MODEL="$MODEL_DSP"
        REASON="mots-clés DSP détectés"
        VERIFY_MODE="cargo check + cargo test"
    else
        SELECTED_MODEL="$MODEL_STANDARD"
        REASON="tâche standard / UI / câblage"
        VERIFY_MODE="cargo check"
    fi

    if [[ "$DRY_RUN" == "true" ]]; then
        printf "🔹 %-6s | Tâches: %-2s | Modèle: %-24s | Vérif: %-24s | Motif: %s\n" \
            "$US" "$TASK_COUNT" "$SELECTED_MODEL" "$VERIFY_MODE" "$REASON"
        continue
    fi

    echo ""
    echo "🚀 [Session vierge] $US -> Modèle: $SELECTED_MODEL ($REASON)"

    PROMPT="/speckit-implement Implémente UNIQUEMENT les tâches non cochées liées à [$US] dans $TASKS_FILE.
Règles strictes :
1. Lis constitution.md, spec.md et plan.md dans $SPEC_DIR.
2. Ne modifie aucune tâche hors de [$US].
3. Coche [x] uniquement les tâches de [$US] terminées dans $TASKS_FILE.
4. Vérifie la compilation avec 'cargo check --message-format=short' (et 'cargo test' si la tâche touche au DSP).
5. Applique le mode Caveman pour tes explications (code Rust complet et non compressé)."

    "$AGY_BIN" --model "$SELECTED_MODEL" --dangerously-skip-permissions -p "$PROMPT"

    # Boucle de vérification locale (cargo check + cargo test conditionnel sur xhigh)
    RETRY=0
    while true; do
        echo "🔍 Vérification ($VERIFY_MODE) pour $US (tentative $((RETRY + 1))/$((MAX_RETRIES + 1)))..."

        VALIDATION_OK=true
        FAIL_TYPE=""
        FAIL_OUTPUT=""

        if ! CARGO_OUT=$(cargo check --message-format=short 2>&1); then
            VALIDATION_OK=false
            FAIL_TYPE="compilation ('cargo check --message-format=short')"
            FAIL_OUTPUT="$CARGO_OUT"
        elif [[ "$SELECTED_MODEL" == "$MODEL_DSP" ]]; then
            echo "🧪 Exécution des tests unitaires DSP ('cargo test')..."
            if ! TEST_OUT=$(cargo test --message-format=short 2>&1); then
                VALIDATION_OK=false
                FAIL_TYPE="tests unitaires ('cargo test')"
                FAIL_OUTPUT="$TEST_OUT"
            fi
        fi

        if [[ "$VALIDATION_OK" == "true" ]]; then
            echo "✅ Validation réussie ($VERIFY_MODE) pour $US !"
            break
        fi

        if [[ $RETRY -ge $MAX_RETRIES ]]; then
            echo "❌ Échec persistant ($FAIL_TYPE) après $MAX_RETRIES tentatives de correction sur $US."
            echo "$FAIL_OUTPUT"
            exit 1
        fi

        RETRY=$((RETRY + 1))
        echo "⚠️ Échec de $FAIL_TYPE détecté. Correction ciblée avec $SELECTED_MODEL..."

        FIX_PROMPT="La vérification de $FAIL_TYPE a échoué après l'implémentation de [$US].
Corrige uniquement le code ou les tests associés sans toucher aux autres User Stories :

$FAIL_OUTPUT"

        "$AGY_BIN" --model "$SELECTED_MODEL" --dangerously-skip-permissions -p "$FIX_PROMPT"
    done

    if [[ "$GIT_AUTO_COMMIT" == "true" ]] && git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
        git add -A
        git commit -m "feat(speckit): implement $US ($SELECTED_MODEL)" --allow-empty >/dev/null
        echo "💾 Checkpoint Git enregistré : feat(speckit): implement $US ($SELECTED_MODEL)"
    fi
done

echo ""
echo "------------------------------------------------------------"

if [[ "$DRY_RUN" == "true" ]]; then
    echo "🔎 Étape finale prévue : /speckit-analyze | Modèle: $MODEL_DSP"
    echo "✅ Fin de la simulation (--dry-run)."
    exit 0
fi

echo "🔎 Audit final /speckit-analyze ($MODEL_DSP) dans une session vierge..."
"$AGY_BIN" --model "$MODEL_DSP" --mode "plan" -p "/speckit-analyze Audite l'ensemble du projet par rapport à $SPEC_DIR (constitution.md, spec.md, plan.md, tasks.md). Affiche uniquement un tableau concis des éventuels écarts critiques ou règles temps réel violées."

echo "🎉 Workflow Spec-Kit terminé avec succès !"
