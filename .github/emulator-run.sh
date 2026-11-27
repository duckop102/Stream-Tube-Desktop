#!/usr/bin/env bash
# Runs inside the emulator job: Home, the Shorts player, a watch page; capture each.
set -x
mkdir -p out
adb install -r app.apk
PKG=io.github.shiroksh.streamtube
adb logcat -c
adb shell monkey -p $PKG -c android.intent.category.LAUNCHER 1
sleep 35
adb exec-out screencap -p > out/1-home.png
SIZE=$(adb shell wm size | grep -o '[0-9]*x[0-9]*' | tail -1); W=${SIZE%x*}; H=${SIZE#*x}
# Shorts tab opens the Shorts player
adb shell input tap $((W*375/1000)) $((H*955/1000))
sleep 25
adb exec-out screencap -p > out/2-shorts.png
sleep 10
adb exec-out screencap -p > out/3-shorts-later.png
adb shell input swipe $((W/2)) $((H*70/100)) $((W/2)) $((H*25/100)) 250
sleep 12
adb exec-out screencap -p > out/4-shorts-next.png
adb shell input keyevent 4
sleep 3
adb shell input tap $((W*125/1000)) $((H*955/1000))
sleep 10
adb exec-out screencap -p > out/5-home-again.png
adb shell input tap $((W/2)) $((H*30/100))
sleep 20
adb exec-out screencap -p > out/6-watch.png
sleep 15
adb exec-out screencap -p > out/7-watch-later.png
adb shell dumpsys meminfo $PKG | head -40 > out/meminfo.txt
adb logcat -d > out/logcat.txt
exit 0
