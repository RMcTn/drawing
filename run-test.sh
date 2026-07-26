#!/bin/sh

actual=$(mktemp "${TMPDIR:-/tmp}/draw-app-state.XXXXXX") || exit 1
log=$(mktemp "${TMPDIR:-/tmp}/draw-app-replay.XXXXXX") || exit 1
summary=$(mktemp "${TMPDIR:-/tmp}/draw-app-summary.XXXXXX") || exit 1
trap 'rm -f "$actual" "$log" "$summary"' EXIT HUP INT TERM

failure_count=0
record_failure() {
    failure_count=$((failure_count + 1))
    printf '  - %s\n' "$1" >>"$summary"
}

if ! cargo test; then
    echo "FAIL - Rust tests failed"
    record_failure "Rust tests"
fi

for expected in tests/*/expected-state.json; do
    test_dir=$(dirname "$expected")
    test_name=$(basename "$test_dir")
    replay="$test_dir/$test_name.rae"

    if [ ! -f "$replay" ]; then
        echo "FAIL - missing replay file: $replay"
        record_failure "$test_name: missing replay file"
        continue
    fi

    # Do not let output from a previous replay hide a failure to produce this replay's files.
    : >"$actual"
    : >"$log"

    if ! cargo run --quiet -- test \
        --replay-path "$replay" \
        --snapshot-path "$actual" \
        --quit-after-replay >"$log" 2>&1; then
        cat "$log"
        echo "FAIL - replay failed: $test_name"
        record_failure "$test_name: replay failed"
        continue
    fi

    # diff writes the complete unified diff as the test runs.
    if diff -u "$expected" "$actual"; then
        echo "PASS - $test_name"
    else
        echo "FAIL - replay state differs: $test_name"
        record_failure "$test_name: replay state differs"
    fi
done

if [ "$failure_count" -ne 0 ]; then
    echo
    echo "$failure_count test failure(s):"
    cat "$summary"
    exit 1
fi

echo
echo "All tests passed"
