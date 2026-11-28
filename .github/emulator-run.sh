#!/usr/bin/env bash
# Runs inside the emulator job: the ytm-rs look, dark then light; capture each step.
# Pixel 6 (1080x2400, 2.625 px/dp); taps are in pixels.
set -x
mkdir -p out
adb install -r app.apk
# let the launcher settle; its "isn't responding" dialog would take every tap
sleep 40
CLOSE="adb shell am broadcast -a android.intent.action.CLOSE_SYSTEM_DIALOGS"
$CLOSE
PKG=io.github.shiroksh.streamtube
adb logcat -c
adb shell monkey -p $PKG -c android.intent.category.LAUNCHER 1
sleep 35
$CLOSE
adb exec-out screencap -p > out/1-home-dark.png
# open the first video: the blurred backdrop behind the watch page
adb shell input tap 540 720
sleep 20
adb exec-out screencap -p > out/2-watch-dark.png
# back: the floating mini player
adb shell input keyevent 4
sleep 4
adb exec-out screencap -p > out/3-mini-dark.png
# You tab, then Settings (the gear)
adb shell input tap 944 2262
sleep 4
adb exec-out screencap -p > out/4-you.png
adb shell input tap 1006 192
sleep 4
adb exec-out screencap -p > out/5-settings-dark.png
# Light, then the green accent
adb shell input tap 958 780
sleep 3
adb exec-out screencap -p > out/6-settings-light.png
adb shell input tap 528 977
sleep 3
adb exec-out screencap -p > out/7-settings-green.png
# Home in the light theme, then a video opened in it
adb shell input tap 135 2262
sleep 8
adb exec-out screencap -p > out/8-home-light.png
adb shell input tap 540 720
sleep 20
adb exec-out screencap -p > out/9-watch-light.png
adb shell input keyevent 4
sleep 4
adb exec-out screencap -p > out/10-mini-light.png
adb logcat -d > out/logcat.txt
exit 0
