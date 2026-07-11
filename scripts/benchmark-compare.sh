#!/usr/bin/env bash
set -euo pipefail

# Compare base/master rtk against the current rtk-next branch.
# Builds both binaries, runs scripts/benchmark.sh against each, and prints
# a side-by-side summary.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BASE_WORKTREE="$PROJECT_ROOT/.worktrees/rtk-base"
REPORT_DIR="$PROJECT_ROOT/scripts/benchmark-compare"
REPORT_FILE="$PROJECT_ROOT/benchmark-compare-report.md"

cd "$PROJECT_ROOT"

# Ensure we are on rtk-next with a clean-ish working tree (allow untracked task files)
CURRENT_BRANCH="$(git rev-parse --abbrev-ref HEAD)"
if [[ "$CURRENT_BRANCH" != "rtk-next" ]]; then
  echo "Error: must be run from the rtk-next branch (current: $CURRENT_BRANCH)" >&2
  exit 1
fi

# Clean previous report artifacts
rm -rf "$REPORT_DIR" "$REPORT_FILE"
mkdir -p "$REPORT_DIR"

# ---------------------------------------------------------------------------
# Build base/master binary
# ---------------------------------------------------------------------------
echo "Building base/master binary..."
rm -rf "$BASE_WORKTREE"
git worktree add --detach "$BASE_WORKTREE" base/master >/dev/null 2>&1
(
  cd "$BASE_WORKTREE"
  cargo build --release 2>&1 | tail -5
)
cp "$BASE_WORKTREE/target/release/rtk" "$PROJECT_ROOT/target/release/rtk-base"

# ---------------------------------------------------------------------------
# Build rtk-next binary
# ---------------------------------------------------------------------------
echo "Building rtk-next binary..."
cargo build --release 2>&1 | tail -5
cp "$PROJECT_ROOT/target/release/rtk" "$PROJECT_ROOT/target/release/rtk-next"

# ---------------------------------------------------------------------------
# Run benchmark for a given binary label
# ---------------------------------------------------------------------------
run_benchmark() {
  local label="$1"
  local binary="$2"
  local out_dir="$REPORT_DIR/$label"
  mkdir -p "$out_dir"

  echo "Running benchmark for $label..."
  # RTK is invoked inside benchmark.sh; we override the binary choice by placing
  # our binary at target/release/rtk temporarily.
  cp "$binary" "$PROJECT_ROOT/target/release/rtk"
  # CI=1 prevents benchmark.sh from writing per-test markdown files and keeps output concise.
  CI=1 "$PROJECT_ROOT/scripts/benchmark.sh" > "$out_dir/summary.txt" 2>&1 || true
}

run_benchmark "base" "$PROJECT_ROOT/target/release/rtk-base"
run_benchmark "rtk-next" "$PROJECT_ROOT/target/release/rtk-next"

# ---------------------------------------------------------------------------
# New-feature benchmark (exercises rtk-next-only capabilities)
# ---------------------------------------------------------------------------
echo "Running new-feature benchmark..."
"$PROJECT_ROOT/scripts/benchmark-new-features.sh" \
  "$PROJECT_ROOT/target/release/rtk-base" \
  "$PROJECT_ROOT/target/release/rtk-next" \
  > "$REPORT_DIR/new-features.txt" 2>&1

# ---------------------------------------------------------------------------
# Generate comparison report
# ---------------------------------------------------------------------------
{
  echo "# RTK Benchmark Comparison: base/master vs rtk-next"
  echo ""
  echo "- base/master: \`$(git rev-parse base/master)\`"
  echo "- rtk-next:    \`$(git rev-parse rtk-next)\`"
  echo ""
  echo "## base/master summary"
  echo '```'
  cat "$REPORT_DIR/base/summary.txt"
  echo '```'
  echo ""
  echo "## rtk-next summary"
  echo '```'
  cat "$REPORT_DIR/rtk-next/summary.txt"
  echo '```'
  echo ""
  echo "## New-feature benchmark"
  echo '```'
  cat "$REPORT_DIR/new-features.txt"
  echo '```'
  echo ""
  echo "## Raw outputs"
  echo "- base:         \`$REPORT_DIR/base/summary.txt\`"
  echo "- rtk-next:     \`$REPORT_DIR/rtk-next/summary.txt\`"
  echo "- new-features: \`$REPORT_DIR/new-features.txt\`"
} > "$REPORT_FILE"

# Clean up worktree
rm -rf "$BASE_WORKTREE"
git worktree prune >/dev/null 2>&1 || true

# Restore rtk-next binary as the default target/release/rtk
cp "$PROJECT_ROOT/target/release/rtk-next" "$PROJECT_ROOT/target/release/rtk"

echo ""
echo "Report written to: $REPORT_FILE"
