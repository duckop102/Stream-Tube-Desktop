#!/usr/bin/env bash
# Runs inside the emulator job: install, search once (so Home becomes "For you"), capture.
set -x
mkdir -p out
adb install -r app.apk
PKG=io.github.shiroksh.streamtube
adb logcat -c
adb shell monkey -p $PKG -c android.intent.category.LAUNCHER 1
sleep 40
adb exec-out screencap -p > out/1-home.png
SIZE=$(adb shell wm size | grep -o '[0-9]*x[0-9]*' | tail -1); W=${SIZE%x*}; H=${SIZE#*x}
adb shell input tap $((W*932/1000)) $((H*80/1000))
sleep 3
adb shell input text "league%sof%slegends"
adb shell input keyevent 66
sleep 15
adb exec-out screencap -p > out/2-search.png
adb shell input tap $((W*117/1000)) $((H*945/1000))
sleep 45
adb exec-out screencap -p > out/3-foryou.png
sleep 30
adb exec-out screencap -p > out/4-foryou-later.png
adb shell am force-stop $PKG
sleep 2
adb shell monkey -p $PKG -c android.intent.category.LAUNCHER 1
sleep 45
adb exec-out screencap -p > out/5-foryou-restart.png
adb logcat -d > out/logcat.txt
exit 0
