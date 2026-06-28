#!/usr/bin/env bash
# Cargo quality gate for the php-native-symbols crate.
#
# Two modes, both wired in .claude/settings.json:
#   post  PostToolUse on Edit|Write: only when a .rs file or Cargo.toml changed,
#         auto-format then run clippy. Fast feedback while editing.
#   stop  Stop: the full gate (fmt --check, clippy, test) before finishing, so a
#         session never ends with a broken or unformatted tree.
#
# On failure the script prints the offending tool's output to stderr and exits 2.
# For PostToolUse that surfaces the output to Claude; for Stop it also blocks the
# stop so the failure gets fixed.
#
# ponytail: post mode re-runs clippy per .rs edit; if multi-edit turns get slow,
# move the post wiring to a PostToolBatch hook so it runs once per batch.
set -u

mode="${1:-post}"
cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0

# Run a check; on failure print labelled combined output to stderr and exit 2.
run() {
    local label="$1"
    shift
    local output
    if ! output="$("$@" 2>&1)"; then
        printf '%s failed:\n%s\n' "$label" "$output" >&2
        exit 2
    fi
}

if [ "$mode" = "post" ]; then
    # Read the edited path from the hook's stdin JSON; skip non-Rust edits.
    command -v jq >/dev/null 2>&1 || exit 0
    file_path="$(jq -r '.tool_input.file_path // empty')"
    case "$file_path" in
        *.rs | *Cargo.toml) ;;
        *) exit 0 ;;
    esac

    cargo fmt
    run "cargo clippy" cargo clippy --all-targets --quiet -- -D warnings
    exit 0
fi

run "cargo fmt --check" cargo fmt --check
run "cargo clippy" cargo clippy --all-targets --quiet -- -D warnings
run "cargo test" cargo test --quiet
exit 0
