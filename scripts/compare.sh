#!/bin/bash
# Aggregate results from TSV files into a comparison table
set -euo pipefail

RESULTS_DIR="${1:-results}"
OUTPUT="$RESULTS_DIR/comparison.md"

echo "# Ablation Study Results" > "$OUTPUT"
echo "" >> "$OUTPUT"

# Aggregate summary.tsv if it exists
if [ -f "$RESULTS_DIR/summary.tsv" ]; then
    echo "## Full Summary" >> "$OUTPUT"
    echo "" >> "$OUTPUT"
    echo '```' >> "$OUTPUT"
    cat "$RESULTS_DIR/summary.tsv" >> "$OUTPUT"
    echo '```' >> "$OUTPUT"
    echo "" >> "$OUTPUT"
fi

# Aggregate per-method seed files
for f in "$RESULTS_DIR"/*_seeds.tsv; do
    [ -f "$f" ] || continue
    method=$(basename "$f" | sed 's/_seeds.tsv//')
    echo "## $method — Seed Stability" >> "$OUTPUT"
    echo "" >> "$OUTPUT"
    echo '```' >> "$OUTPUT"
    cat "$f" >> "$OUTPUT"
    echo '```' >> "$OUTPUT"
    echo "" >> "$OUTPUT"

    # Compute stats
    solved_vals=$(tail -n +2 "$f" | awk -F'\t' '{print $4}')
    n=$(echo "$solved_vals" | wc -l)
    sum=$(echo "$solved_vals" | awk '{s+=$1} END {print s}')
    mean=$(echo "$sum $n" | awk '{printf "%.1f", $1/$2}')
    echo "Mean solved: $mean / $n seeds" >> "$OUTPUT"
    echo "" >> "$OUTPUT"
done

# Fast comparison
if [ -f "$RESULTS_DIR/fast_comparison.tsv" ]; then
    echo "## Fast Comparison (30 tasks)" >> "$OUTPUT"
    echo "" >> "$OUTPUT"
    echo '```' >> "$OUTPUT"
    cat "$RESULTS_DIR/fast_comparison.tsv" >> "$OUTPUT"
    echo '```' >> "$OUTPUT"
fi

echo "" >> "$OUTPUT"
echo "Generated: $(date)" >> "$OUTPUT"
echo "Comparison written to $OUTPUT"
