#!/bin/sh
# Run the Step 1/2 hardening quality gate, and fail if any check fails.
set -u
failed=0
for cmd in \
    'cargo fmt --check' \
    'cargo check --all-targets --all-features' \
    'cargo clippy --all-targets --all-features -- -D warnings' \
    'cargo test' \
    'cargo test --release' \
    'cargo test --locked' \
    'cargo test --locked --release' \
    'cargo doc --no-deps'; do
    printf '\n$ %s\n' "$cmd"
    if sh -c "$cmd"; then
        printf 'EXIT_STATUS=0\n'
    else
        status=$?
        printf 'EXIT_STATUS=%s\n' "$status"
        failed=1
    fi
done
if [ "$failed" -ne 0 ]; then
    printf '\nVerification FAILED: one or more checks did not pass.\n' >&2
    exit 1
fi
printf '\nVerification PASSED: all checks succeeded.\n'
