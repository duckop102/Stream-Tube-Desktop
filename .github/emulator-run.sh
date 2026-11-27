#!/usr/bin/env bash
# Runs inside the emulator job: install, open Home, open Shorts, capture.
set -x
mkdir -p out
adb install -r app.apk
PKG=io.github.shiroksh.streamtube
adb logcat -c
adb shell monkey -p $PKG -c android.intent.category.LAUNCHER 1
sleep 45
adb exec-out screencap -p > out/1-home.png
SIZE=$(adb shell wm size | grep -o '[0-9]*x[0-9]*' | tail -1); W=${SIZE%x*}; H=${SIZE#*x}
echo "screen $W x $H"
adb shell input swipe $((W/2)) $((H*7/10)) $((W/2)) $((H*3/10)) 300
sleep 8
adb exec-out screencap -p > out/2-home-scrolled.png
adb shell input tap $((W*354/1000)) $((H*945/1000))
sleep 20
adb exec-out screencap -p > out/3-shorts.png
sleep 20
adb exec-out screencap -p > out/4-shorts-later.png
adb shell input swipe $((W/2)) $((H*7/10)) $((W/2)) $((H*2/10)) 250
sleep 15
adb exec-out screencap -p > out/5-shorts-next.png
adb shell dumpsys activity processes | grep -i -A3 "not responding\|anr" > out/anr.txt || true
adb shell pidof $PKG > out/pid.txt || true
adb logcat -d > out/logcat.txt
adb shell ls /data/anr > out/anr-files.txt 2>&1 || true
exit 0
