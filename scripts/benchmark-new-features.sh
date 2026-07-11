#!/usr/bin/env bash
set -euo pipefail

# Supplemental benchmark that exercises features present in rtk-next but not
# in base/master, so the comparison is meaningful.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_ROOT"

RTK_BASE="${1:-$PROJECT_ROOT/target/release/rtk-base}"
RTK_NEXT="${2:-$PROJECT_ROOT/target/release/rtk-next}"

if [[ ! -x "$RTK_BASE" ]]; then
  echo "Error: base binary not found at $RTK_BASE" >&2
  exit 1
fi
if [[ ! -x "$RTK_NEXT" ]]; then
  echo "Error: rtk-next binary not found at $RTK_NEXT" >&2
  exit 1
fi

count_tokens() {
  local input="$1"
  local len=${#input}
  echo $(( (len + 3) / 4 ))
}

bench() {
  local name="$1"
  local unix_cmd="$2"
  local base_cmd="$3"
  local next_cmd="$4"

  local unix_out base_out next_out
  unix_out=$(eval "$unix_cmd" 2>/dev/null || true)
  base_out=$(eval "$base_cmd" 2>/dev/null || true)
  next_out=$(eval "$next_cmd" 2>/dev/null || true)

  local unix_tokens base_tokens next_tokens
  unix_tokens=$(count_tokens "$unix_out")
  base_tokens=$(count_tokens "$base_out")
  next_tokens=$(count_tokens "$next_out")

  local base_saved=0
  local next_saved=0
  if [[ "$unix_tokens" -gt 0 ]]; then
    base_saved=$(( (unix_tokens - base_tokens) * 100 / unix_tokens ))
    next_saved=$(( (unix_tokens - next_tokens) * 100 / unix_tokens ))
  fi

  printf "%-30s │ %8d │ %8d (%+4d%%) │ %8d (%+4d%%)\n" \
    "$name" "$unix_tokens" "$base_tokens" "$base_saved" "$next_tokens" "$next_saved"
}

# Fixture: temp directory with a Makefile that prints repeated lines.
MAKE_FIXTURE=$(mktemp -d)
cat > "$MAKE_FIXTURE/Makefile" << 'EOF'
all:
	@echo "INFO: starting build"
	@for i in $$(seq 1 20); do echo "INFO: compiling module..."; done
	@echo "INFO: build complete"
EOF

# Fixture: Rust source file for read --mode signatures/map.
SIGNATURES_FILE=$(mktemp)
cat > "$SIGNATURES_FILE" << 'EOF'
use std::collections::HashMap;

pub struct UserService {
    db: Database,
}

impl UserService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn find_user(&self, id: u64) -> Result<User, Error> {
        let user = self.db.query("SELECT * FROM users WHERE id = ?", id).await?;
        Ok(user)
    }

    pub async fn create_user(&self, name: &str, email: &str) -> Result<User, Error> {
        let user = self.db.insert("users", name, email).await?;
        Ok(user)
    }
}
EOF

echo "New-feature benchmark: base/master vs rtk-next"
echo "═══════════════════════════════════════════════════════════════════════════════"
printf "%-30s │ %8s │ %18s │ %18s\n" "TEST" "RAW" "BASE" "RTK-NEXT"
echo "───────────────────────────────────────────────────────────────────────────────"

bench "make-repeated-lines" \
  "cd $MAKE_FIXTURE && make" \
  "cd $MAKE_FIXTURE && $RTK_BASE make" \
  "cd $MAKE_FIXTURE && $RTK_NEXT make"

bench "read-signatures" \
  "cat $SIGNATURES_FILE" \
  "$RTK_BASE read $SIGNATURES_FILE" \
  "$RTK_NEXT read $SIGNATURES_FILE --mode signatures"

bench "read-map" \
  "cat $SIGNATURES_FILE" \
  "$RTK_BASE read $SIGNATURES_FILE" \
  "$RTK_NEXT read $SIGNATURES_FILE --mode map"

rm -rf "$MAKE_FIXTURE" "$SIGNATURES_FILE"
