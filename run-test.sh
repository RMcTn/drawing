#!/bin/sh

export DISPLAY=:0.0
cargo run test --replay-path tests/draw_with_color_change/draw_with_color_change.rae --save-path draw_with_color-TEST.sav --save-after-replay --quit-after-replay
diff_result=$(diff tests/draw_with_color_change/draw_with_color_change.sav draw_with_color-TEST.sav)


if [ -z $diff_result ]; then
	echo "PASS"
	exit 0
else
	echo "FAIL - $diff_result"
	exit 1
fi

