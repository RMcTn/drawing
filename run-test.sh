#!/bin/sh

actual=$(mktemp "${TMPDIR:-/tmp}/draw-app-state.XXXXXX") || exit 1
log=$(mktemp "${TMPDIR:-/tmp}/draw-app-replay.XXXXXX") || exit 1
trap 'rm -f "$actual" "$log"' EXIT HUP INT TERM

if ! cargo test; then
    echo "FAIL - Rust tests failed"
    exit 1
fi

for expected in tests/*/expected-state.json; do
    test_dir=$(dirname "$expected")
    test_name=$(basename "$test_dir")
    replay="$test_dir/$test_name.rae"

    if [ ! -f "$replay" ]; then
        echo "FAIL - missing replay file: $replay"
        exit 1
    fi

    if ! cargo run --quiet -- test \
        --replay-path "$replay" \
        --snapshot-path "$actual" \
        --quit-after-replay >"$log" 2>&1; then
        cat "$log"
        echo "FAIL - replay failed: $test_name"
        exit 1
    fi

    if diff -u "$expected" "$actual"; then
        echo "PASS - $test_name"
    else
        echo "FAIL - replay state differs: $test_name"
        exit 1
    fi
done
