package io.github.shiroksh.streamtube;

import android.app.Activity;
import android.app.PendingIntent;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.pm.PackageInfo;
import android.content.pm.PackageInstaller;
import android.os.Build;

import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.security.MessageDigest;

/**
 * Installs a newer Stream Tube APK through PackageInstaller.
 *
 * The APK streams straight into an install session while its SHA-256 is computed. The
 * session is committed only when the checksum matches the one GitHub reports for the
 * release file; otherwise it is abandoned. Android then asks the user to confirm,
 * unless this app installed the current version itself (Android 12 and newer).
 */
public final class AppUpdater {
    private static final String ACTION = "io.github.shiroksh.streamtube.UPDATE_STATUS";
    private static final int PROGRESS = 0;
    private static final int CONFIRM = 1;
    private static final int INSTALLED = 2;
    private static final int FAILED = 3;
    private static volatile boolean busy;
    private static BroadcastReceiver receiver;

    private AppUpdater() {}

    static native void deliver(int status, int progress, String message);

    public static long versionCode(Activity activity) {
        try {
            PackageInfo info = activity.getPackageManager().getPackageInfo(activity.getPackageName(), 0);
            return Build.VERSION.SDK_INT >= 28 ? info.getLongVersionCode() : info.versionCode;
        } catch (Exception error) {
            return 0;
        }
    }

    public static String versionName(Activity activity) {
        try {
            PackageInfo info = activity.getPackageManager().getPackageInfo(activity.getPackageName(), 0);
            return info.versionName == null ? "" : info.versionName;
        } catch (Exception error) {
            return "";
        }
    }

    public static void install(final Activity activity, final String url, final String sha256, final long size) {
        if (busy) return;
        busy = true;
        Thread worker = new Thread(() -> {
            try {
                run(activity, url, sha256, size);
            } catch (Throwable error) {
                busy = false;
                deliver(FAILED, 0, String.valueOf(error.getMessage()));
            }
        }, "stream-tube-update");
        worker.start();
    }

    private static void run(Activity activity, String url, String sha256, long size) throws Exception {
        Context context = activity.getApplicationContext();
        PackageInstaller installer = context.getPackageManager().getPackageInstaller();
        PackageInstaller.SessionParams params =
                new PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL);
        params.setAppPackageName(context.getPackageName());
        if (size > 0) params.setSize(size);
        if (Build.VERSION.SDK_INT >= 31) {
            params.setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED);
        }
        int id = installer.createSession(params);
        PackageInstaller.Session session = installer.openSession(id);
        boolean committed = false;
        try {
            MessageDigest digest = MessageDigest.getInstance("SHA-256");
            HttpURLConnection connection = (HttpURLConnection) new URL(url).openConnection();
            connection.setInstanceFollowRedirects(true);
            connection.setConnectTimeout(15000);
            connection.setReadTimeout(30000);
            connection.setRequestProperty("User-Agent", "StreamTube-Android-Updater");
            int code = connection.getResponseCode();
            if (code != 200) throw new IllegalStateException("HTTP " + code);
            long total = size > 0 ? size : connection.getContentLengthLong();
            long done = 0;
            int reported = -1;
            try (InputStream in = connection.getInputStream();
                 OutputStream out = session.openWrite("base.apk", 0, total > 0 ? total : -1)) {
                byte[] buffer = new byte[256 * 1024];
                int read;
                while ((read = in.read(buffer)) != -1) {
                    out.write(buffer, 0, read);
                    digest.update(buffer, 0, read);
                    done += read;
                    int percent = total > 0 ? (int) Math.min(100, done * 100 / total) : 0;
                    if (percent != reported) {
                        reported = percent;
                        deliver(PROGRESS, percent, "");
                    }
                }
                session.fsync(out);
            } finally {
                connection.disconnect();
            }
            if (size > 0 && done != size) throw new IllegalStateException("The download is incomplete");
            if (!hex(digest.digest()).equalsIgnoreCase(sha256)) {
                throw new IllegalStateException("The download doesn't match the release's checksum");
            }

            listen(activity);
            Intent intent = new Intent(ACTION).setPackage(context.getPackageName());
            int flags = PendingIntent.FLAG_UPDATE_CURRENT
                    | (Build.VERSION.SDK_INT >= 31 ? PendingIntent.FLAG_MUTABLE : 0);
            PendingIntent pending = PendingIntent.getBroadcast(context, id, intent, flags);
            session.commit(pending.getIntentSender());
            committed = true;
        } finally {
            if (!committed) session.abandon();
            session.close();
        }
    }

    private static synchronized void listen(final Activity activity) {
        if (receiver != null) return;
        final Context context = activity.getApplicationContext();
        receiver = new BroadcastReceiver() {
            @Override
            public void onReceive(Context ignored, Intent intent) {
                int status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE);
                if (status == PackageInstaller.STATUS_PENDING_USER_ACTION) {
                    Intent confirm = intent.getParcelableExtra(Intent.EXTRA_INTENT);
                    if (confirm != null) {
                        confirm.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
                        if (activity.isFinishing()) context.startActivity(confirm);
                        else activity.startActivity(confirm);
                    }
                    deliver(CONFIRM, 100, "");
                    return;
                }
                busy = false;
                if (status == PackageInstaller.STATUS_SUCCESS) {
                    deliver(INSTALLED, 100, "");
                } else {
                    String message = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE);
                    deliver(FAILED, 0, message == null ? "Installation was cancelled" : message);
                }
            }
        };
        IntentFilter filter = new IntentFilter(ACTION);
        if (Build.VERSION.SDK_INT >= 33) {
            context.registerReceiver(receiver, filter, Context.RECEIVER_NOT_EXPORTED);
        } else {
            context.registerReceiver(receiver, filter);
        }
    }

    private static String hex(byte[] bytes) {
        StringBuilder out = new StringBuilder(bytes.length * 2);
        for (byte value : bytes) out.append(String.format("%02x", value));
        return out.toString();
    }
}
