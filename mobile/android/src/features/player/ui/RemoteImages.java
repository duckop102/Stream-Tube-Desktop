package io.github.shiroksh.streamtube;

import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.drawable.BitmapDrawable;
import android.graphics.drawable.Drawable;
import android.graphics.drawable.TransitionDrawable;
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

    /**
     * A soft, blurred copy of a picture that fades in over the previous one: the backdrop
     * of the watch page, as ytm-rs draws the album art behind everything.
     */
    static void loadBlurred(ImageView view, String url) {
        String key = "blur:" + url;
        view.setTag(key);
        Bitmap cached = CACHE.get(key);
        if (cached != null) {
            crossFade(view, cached);
            return;
        }
        POOL.execute(() -> {
            Bitmap source = fetch(url, 160);
            Bitmap blurred = source == null ? null : blur(source);
            MAIN.post(() -> {
                if (blurred == null) {
                    return;
                }
                CACHE.put(key, blurred);
                if (key.equals(view.getTag())) {
                    crossFade(view, blurred);
                }
            });
        });
    }

    private static void crossFade(ImageView view, Bitmap bitmap) {
        Drawable next = new BitmapDrawable(view.getResources(), bitmap);
        Drawable previous = view.getDrawable();
        if (previous instanceof TransitionDrawable) {
            previous = ((TransitionDrawable) previous).getDrawable(1);
        }
        if (previous == null) {
            view.setImageDrawable(next);
            return;
        }
        TransitionDrawable fade = new TransitionDrawable(new Drawable[]{previous, next});
        fade.setCrossFadeEnabled(true);
        view.setImageDrawable(fade);
        fade.startTransition(450);
    }

    /** Shrinks the picture to a few dozen pixels and box-blurs it; scaling it up smooths the rest. */
    private static Bitmap blur(Bitmap source) {
        int width = 40;
        int height = Math.max(1, source.getHeight() * width / Math.max(1, source.getWidth()));
        Bitmap small = Bitmap.createScaledBitmap(source, width, height, true);
        int[] pixels = new int[width * height];
        small.getPixels(pixels, 0, width, 0, 0, width, height);
        int[] spare = new int[pixels.length];
        for (int pass = 0; pass < 3; pass++) {
            boxBlur(pixels, spare, width, height, 1, width, 3);
            boxBlur(spare, pixels, height, width, width, 1, 3);
        }
        Bitmap out = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888);
        out.setPixels(pixels, 0, width, 0, 0, width, height);
        return out;
    }

    /** One blur pass along rows (step 1) or columns (step = width) of an ARGB image. */
    private static void boxBlur(int[] in, int[] out, int length, int lines, int step, int lineStep, int radius) {
        for (int line = 0; line < lines; line++) {
            int base = line * lineStep;
            for (int i = 0; i < length; i++) {
                int r = 0;
                int g = 0;
                int b = 0;
                int count = 0;
                for (int k = Math.max(0, i - radius); k <= Math.min(length - 1, i + radius); k++) {
                    int pixel = in[base + k * step];
                    r += (pixel >> 16) & 0xFF;
                    g += (pixel >> 8) & 0xFF;
                    b += pixel & 0xFF;
                    count++;
                }
                out[base + i * step] = 0xFF000000 | ((r / count) << 16) | ((g / count) << 8) | (b / count);
            }
        }
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
