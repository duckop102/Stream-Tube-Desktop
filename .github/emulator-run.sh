#!/usr/bin/env bash
# Runs inside the emulator job: background playback of a watch page; capture each step.
set -x
mkdir -p out
adb install -r app.apk
# let the launcher settle; its "isn't responding" dialog would take every tap
sleep 40
CLOSE="adb shell am broadcast -a android.intent.action.CLOSE_SYSTEM_DIALOGS"
$CLOSE
PKG=io.github.shiroksh.streamtube
adb shell dumpsys package $PKG | grep -iE "permission|PlaybackService" > out/package.txt
adb logcat -c
adb shell monkey -p $PKG -c android.intent.category.LAUNCHER 1
sleep 35
adb exec-out screencap -p > out/1-home.png
SIZE=$(adb shell wm size | grep -o '[0-9]*x[0-9]*' | tail -1); W=${SIZE%x*}; H=${SIZE#*x}
$CLOSE
# a fling down the feed and back
adb shell input swipe $((W/2)) $((H*80/100)) $((W/2)) $((H*20/100)) 120
sleep 2
adb exec-out screencap -p > out/2-scrolled.png
adb shell input swipe $((W/2)) $((H*20/100)) $((W/2)) $((H*80/100)) 120
sleep 2
# open the first video
adb shell input tap $((W/2)) $((H*30/100))
sleep 20
adb exec-out screencap -p > out/3-watch.png
adb shell dumpsys media_session > out/media-session-open.txt
adb shell dumpsys activity services $PKG > out/services-open.txt
# leave the app: the video should go on in the background
adb shell input keyevent 3
sleep 10
adb shell dumpsys activity services $PKG > out/services-background.txt
adb shell dumpsys media_session > out/media-session-background.txt
adb shell cmd statusbar expand-notifications
sleep 3
adb exec-out screencap -p > out/4-notifications.png
adb shell cmd statusbar collapse
adb shell dumpsys notification --noredact | grep -A30 "pkg=$PKG" > out/notification.txt
# back to the app
adb shell monkey -p $PKG -c android.intent.category.LAUNCHER 1
sleep 6
adb exec-out screencap -p > out/5-back.png
# swipe the app away: the service should end
adb shell am force-stop $PKG
sleep 3
adb shell dumpsys activity services $PKG > out/services-after-stop.txt
adb logcat -d > out/logcat.txt
exit 0
