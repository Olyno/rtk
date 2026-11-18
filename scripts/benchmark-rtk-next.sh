#!/usr/bin/env bash
set -euo pipefail

# Compare base/master rtk with the current rtk-next branch.
# Builds both release binaries into separate directories and runs a shared
# benchmark scenario. Outputs a markdown report in scripts/benchmark-rtk-next/.

RTK_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BENCH_DIR="$RTK_ROOT/scripts/benchmark-rtk-next"
BASE_DIR="$BENCH_DIR/base"
NEXT_DIR="$BENCH_DIR/next"
REPORT="$BENCH_DIR/report.md"

mkdir -p "$BASE_DIR" "$NEXT_DIR"

count_tokens() {
  local input="$1"
  local len=${#input}
  echo $(((len + 3) / 4))
}

build_binary() {
  local name="$1"
  local out_dir="$2"
  echo "Building $name ..."
  cargo build --release --target-dir "$out_dir" >/dev/null 2>&1
  echo "  -> $out_dir/release/rtk"
}

# Build base/master from a clean worktree clone.
build_base() {
  if [ ! -f "$BASE_DIR/release/rtk" ]; then
    echo "Fetching base/master into temporary worktree ..."
    local tmp_src
    tmp_src="$(mktemp -d)"
    git clone --quiet --branch master --single-branch "$RTK_ROOT" "$tmp_src" 2>/dev/null || {
      echo "Falling back to git archive for base/master"
      mkdir -p "$tmp_src"
      git -C "$RTK_ROOT" archive base/master | tar -x -C "$tmp_src"
    }
    # If base remote is newer, use it; otherwise the local clone already tracks it.
    if [ -d "$tmp_src/.git" ]; then
      git -C "$tmp_src" remote add base https://github.com/rtk-ai/rtk.git 2>/dev/null || true
      git -C "$tmp_src" fetch --quiet base master 2>/dev/null || true
      git -C "$tmp_src" checkout --quiet base/master 2>/dev/null || true
    fi
    cargo build --release --manifest-path "$tmp_src/Cargo.toml" --target-dir "$BASE_DIR" >/dev/null 2>&1
    rm -rf "$tmp_src"
  fi
}

# Build the current branch (rtk-next) using the existing checkout.
build_next() {
  if [ ! -f "$NEXT_DIR/release/rtk" ]; then
    cargo build --release --target-dir "$NEXT_DIR" >/dev/null 2>&1
  fi
}

RTK_BASE="$BASE_DIR/release/rtk"
RTK_NEXT="$NEXT_DIR/release/rtk"

TOTAL_BASE=0
TOTAL_NEXT=0
TOTAL_TESTS=0
WINS_BASE=0
WINS_NEXT=0
TIES=0

bench() {
  local name="$1"
  local cmd_args="$2"

  local base_out next_out
  base_out=$("$RTK_BASE" $cmd_args 2>&1 || true)
  next_out=$("$RTK_NEXT" $cmd_args 2>&1 || true)

  local base_tokens next_tokens
  base_tokens=$(count_tokens "$base_out")
  next_tokens=$(count_tokens "$next_out")

  TOTAL_BASE=$((TOTAL_BASE + base_tokens))
  TOTAL_NEXT=$((TOTAL_NEXT + next_tokens))
  TOTAL_TESTS=$((TOTAL_TESTS + 1))

  local icon=""
  if [ "$next_tokens" -lt "$base_tokens" ]; then
    icon="✅"
    WINS_NEXT=$((WINS_NEXT + 1))
  elif [ "$next_tokens" -gt "$base_tokens" ]; then
    icon="🔴"
    WINS_BASE=$((WINS_BASE + 1))
  else
    icon="⚠️"
    TIES=$((TIES + 1))
  fi

  local pct=0
  if [ "$base_tokens" -gt 0 ]; then
    pct=$(((base_tokens - next_tokens) * 100 / base_tokens))
  fi

  printf "%s %-28s │ %6d │ %6d │ %+4d%%\n" "$icon" "$name" "$base_tokens" "$next_tokens" "$pct"
  printf "| %s | %d | %d | %d%% |\n" "$name" "$base_tokens" "$next_tokens" "$pct" >> "$REPORT"

  # Save detailed outputs for inspection.
  local safe_name
  safe_name=$(echo "$name" | tr ' /' '_-' | tr -cd 'a-zA-Z0-9_-')
  printf "# %s\n\`\`\`bash\n$ %s\n\`\`\`\n\n\`\`\`\n%s\n\`\`\`\n" \
    "$name" "$cmd_args" "$base_out" > "$BASE_DIR/${safe_name}.md"
  printf "# %s\n\`\`\`bash\n$ %s\n\`\`\`\n\n\`\`\`\n%s\n\`\`\`\n" \
    "$name" "$cmd_args" "$next_out" > "$NEXT_DIR/${safe_name}.md"
}

# Run a raw shell command and return stdout (stderr discarded).
run_raw() {
  bash -c "$1" 2>/dev/null || true
}

# Compare an equivalent raw command sequence with an rtk-next command.
# Usage: bench_pair "name" "raw command string" "rtk args"
bench_pair() {
  local name="$1"
  local raw_cmd="$2"
  local rtk_cmd="$3"

  local raw_out rtk_out raw_tokens rtk_tokens pct icon
  raw_out=$(run_raw "$raw_cmd")
  rtk_out=$($RTK_NEXT $rtk_cmd 2>/dev/null || true)

  raw_tokens=$(count_tokens "$raw_out")
  rtk_tokens=$(count_tokens "$rtk_out")

  if [ "$raw_tokens" -gt 0 ]; then
    pct=$(((raw_tokens - rtk_tokens) * 100 / raw_tokens))
  else
    pct=0
  fi

  if [ "$rtk_tokens" -lt "$raw_tokens" ]; then
    icon="✅"
  elif [ "$rtk_tokens" -gt "$raw_tokens" ]; then
    icon="🔴"
  else
    icon="⚠️"
  fi

  printf "%s %-38s │ %6d │ %6d │ %+4d%%\n" "$icon" "$name" "$raw_tokens" "$rtk_tokens" "$pct"
  printf "| %s | %d | %d | %d%% |\n" "$name" "$raw_tokens" "$rtk_tokens" "$pct" >> "$REPORT"
}

# Main
build_base
build_next

{
  echo "# RTK-next vs base/master benchmark"
  echo ""
  echo "Date: $(date -Iseconds)"
  echo ""
  echo "| Scenario | base/master tokens | rtk-next tokens | Δ |"
  echo "|----------|-------------------:|----------------:|--:|"
} > "$REPORT"

echo ""
echo "RTK-next vs base/master"
echo "────────────────────────────────────────────────────────────────────"
printf "   %-28s │ %6s │ %6s │ %5s\n" "SCENARIO" "BASE" "NEXT" "DELTA"
echo "────────────────────────────────────────────────────────────────────"

# Scenarios that exercise the kept features and common filters.
bench "read main.rs" "read $RTK_ROOT/src/main.rs"
bench "grep fn" "grep -rn 'fn ' $RTK_ROOT/src/"
bench "rg struct" "rg 'struct ' $RTK_ROOT/src/"
bench "json sample" "json $RTK_ROOT/Cargo.toml"
bench "deps" "deps $RTK_ROOT"
bench "ls src" "ls $RTK_ROOT/src/"

# Consolidation scenarios: rtk-next routes `rtk run <cmd>` to specialized filters,
# while base/master treats `rtk run` as a raw shell passthrough.
bench "run git status" "run git status"
bench "run ls src" "run ls $RTK_ROOT/src/"

# Large output that triggers archive/ref tokens.
if command -v cargo >/dev/null 2>&1; then
  bench "cargo test (large)" "test cargo test 2>&1"
fi

TOTAL_PCT=0
if [ "$TOTAL_BASE" -gt 0 ]; then
  TOTAL_PCT=$(((TOTAL_BASE - TOTAL_NEXT) * 100 / TOTAL_BASE))
fi

echo "────────────────────────────────────────────────────────────────────"
printf "   %-28s │ %6d │ %6d │ %+4d%%\n" "TOTAL" "$TOTAL_BASE" "$TOTAL_NEXT" "$TOTAL_PCT"
echo ""
printf "  ✅ rtk-next wins: %d  🔴 base wins: %d  ⚠️ ties: %d\n" "$WINS_NEXT" "$WINS_BASE" "$TIES"
echo ""

echo "Fair feature comparisons (raw equivalent vs rtk-next)"
echo "──────────────────────────────────────────────────────────────────────────────"
printf "   %-38s │ %6s │ %6s │ %5s\n" "SCENARIO" "RAW" "RTK" "DELTA"
echo "──────────────────────────────────────────────────────────────────────────────"

{
  echo ""
  echo "## Fair feature comparisons (raw equivalent vs rtk-next)"
  echo ""
  echo "| Scenario | raw tokens | rtk-next tokens | Δ |"
  echo "|----------|-----------:|----------------:|--:|"
} >> "$REPORT"

# These compare the raw commands an agent would otherwise run against the
# rtk-next optimized equivalent.
bench_pair "read signatures vs full file" \
  "cat \"$RTK_ROOT/src/main.rs\"" \
  "read --mode signatures $RTK_ROOT/src/main.rs"

bench_pair "rtk ls vs raw ls" \
  "ls -la \"$RTK_ROOT/src/\"" \
  "ls $RTK_ROOT/src/"

echo "──────────────────────────────────────────────────────────────────────────────"
echo ""

{
  echo ""
  echo "| **TOTAL** | **$TOTAL_BASE** | **$TOTAL_NEXT** | **$TOTAL_PCT%** |"
  echo ""
  echo "- rtk-next wins: $WINS_NEXT"
  echo "- base/master wins: $WINS_BASE"
  echo "- ties: $TIES"
  echo ""
  echo "## Notes"
  echo ""
  echo "- base's \`rtk run\` is a raw passthrough without filter auto-routing; rtk-next routes recognized commands to their filters. The fair comparison is against the raw commands the kept features replace."
  echo "- \`§ref:<hash>§\` tokens add a small constant overhead (~4 tokens) to archived outputs in exchange for on-demand expansion via \`rtk expand <hash>\`. They pay off when the LLM does not need the full raw output."
  echo "- Secret redaction runs on every emitted output but has no token impact on these benign fixtures."
  echo ""
  echo "Detailed outputs:"
  echo "- base: $BASE_DIR/"
  echo "- next: $NEXT_DIR/"
} >> "$REPORT"

echo "Report written to: $REPORT"
