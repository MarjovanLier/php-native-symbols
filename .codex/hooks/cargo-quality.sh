#!/usr/bin/env bash
# Cargo quality gate for the php-native-symbols crate (Codex hooks).
# Parallel to .claude/hooks/cargo-quality.sh, adapted to Codex's hook input.
#
# Two modes, wired in .codex/hooks.json:
#   post  PostToolUse on apply_patch: when the patch touches a .rs file or
#         Cargo.toml, auto-format then run clippy. Fast feedback while editing.
#   stop  Stop: the full gate (fmt --check, clippy, test) before finishing, so a
#         session never ends with a broken or unformatted tree.
#
# Codex reports file edits as tool_name "apply_patch" and carries the patched
# paths inside the command string (tool_input.command), not a dedicated field,
# so post mode greps that text rather than reading a file_path.
#
# On failure: print the offending tool's output to stderr and exit 2. For
# PostToolUse Codex replaces the tool result with that feedback; for Stop it
# feeds the reason back and continues instead of stopping. On success the script
# exits 0 with no stdout (Codex requires JSON, not plain text, on a Stop exit 0).
#
# ponytail: post mode re-runs clippy per patch; if it gets slow, narrow the
# matcher or move to a batch-style trigger.
set -u

mode="${1:-post}"

# Run cargo against the crate regardless of the session's subdirectory.
repo_root="$(git rev-parse --show-toplevel 2>/dev/null)"
cd "${repo_root:-.}" || exit 0

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
    command -v jq >/dev/null 2>&1 || exit 0
    # The patched paths live in the apply_patch command text; skip non-Rust edits.
    command_text="$(jq -r '.tool_input.command // empty')"
    case "$command_text" in
        *.rs* | *Cargo.toml*) ;;
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
