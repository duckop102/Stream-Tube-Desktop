package io.github.shiroksh.streamtube;

import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.os.Handler;
import android.os.Looper;
import android.util.LruCache;
import android.widget.ImageView;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** Thumbnails and avatars for the watch page, from YouTube's image hosts only. */
final class RemoteImages {
    private static final int MAX_BYTES = 4 * 1024 * 1024;
    private static final ExecutorService POOL = Executors.newFixedThreadPool(3, task -> {
        Thread thread = new Thread(task, "stream-tube-images");
        thread.setDaemon(true);
        return thread;
    });
    private static final Handler MAIN = new Handler(Looper.getMainLooper());
    private static final LruCache<String, Bitmap> CACHE = new LruCache<String, Bitmap>(24 * 1024 * 1024) {
        @Override protected int sizeOf(String key, Bitmap value) {
            return value.getByteCount();
        }
    };

    private RemoteImages() {}

    static String thumbnail(String videoId) {
        return "https://i.ytimg.com/vi/" + videoId + "/mqdefault.jpg";
    }

    static void load(ImageView view, String url, int width) {
        view.setTag(url);
        if (url == null || url.isEmpty()) {
            view.setImageDrawable(null);
            return;
        }
        Bitmap cached = CACHE.get(url);
        if (cached != null) {
            view.setImageBitmap(cached);
            return;
        }
        view.setImageDrawable(null);
        POOL.execute(() -> {
            Bitmap bitmap = fetch(url, width);
            MAIN.post(() -> {
                if (bitmap == null) {
                    return;
                }
                CACHE.put(url, bitmap);
                if (url.equals(view.getTag())) {
                    view.setImageBitmap(bitmap);
                }
            });
        });
    }

    private static boolean trusted(URL url) {
        String host = url.getHost();
        return "https".equals(url.getProtocol()) && host != null && (host.equals("i.ytimg.com")
            || host.endsWith(".ytimg.com") || host.endsWith(".ggpht.com")
            || host.endsWith(".googleusercontent.com"));
    }

    private static Bitmap fetch(String address, int width) {
        HttpURLConnection connection = null;
        try {
            URL url = new URL(address);
            if (!trusted(url)) {
                return null;
            }
            connection = (HttpURLConnection) url.openConnection();
            connection.setConnectTimeout(10000);
            connection.setReadTimeout(10000);
            connection.setInstanceFollowRedirects(false);
            if (connection.getResponseCode() != 200) {
                return null;
            }
            byte[] bytes;
            try (InputStream in = connection.getInputStream()) {
                ByteArrayOutputStream out = new ByteArrayOutputStream();
                byte[] chunk = new byte[16384];
                int read;
                while ((read = in.read(chunk)) > 0) {
                    out.write(chunk, 0, read);
                    if (out.size() > MAX_BYTES) {
                        return null;
                    }
                }
                bytes = out.toByteArray();
            }
            BitmapFactory.Options bounds = new BitmapFactory.Options();
            bounds.inJustDecodeBounds = true;
            BitmapFactory.decodeByteArray(bytes, 0, bytes.length, bounds);
            BitmapFactory.Options options = new BitmapFactory.Options();
            options.inSampleSize = 1;
            while (width > 0 && bounds.outWidth / (options.inSampleSize * 2) >= width) {
                options.inSampleSize *= 2;
            }
            return BitmapFactory.decodeByteArray(bytes, 0, bytes.length, options);
        } catch (IOException | RuntimeException failure) {
            return null;
        } finally {
            if (connection != null) {
                connection.disconnect();
            }
        }
    }
}
