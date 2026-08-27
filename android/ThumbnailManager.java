package app.rustdl;

import android.graphics.Bitmap;
import android.media.MediaMetadataRetriever;

import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.util.concurrent.Semaphore;
import java.util.regex.Pattern;

final class ThumbnailManager {
    private static final Pattern VIDEO_NAME = Pattern.compile(
            "(?:[0-9]+-[1-9][0-9]*|youtube-[A-Za-z0-9_-]{11}|snapchat-[A-Za-z0-9_-]{20,160})\\.mp4");
    private static final int MAX_WIDTH = 640;
    private static final Semaphore GENERATION_SLOTS = new Semaphore(2);

    private ThumbnailManager() {
    }

    static boolean generate(File video, String displayName) {
        boolean acquired = false;
        try {
            GENERATION_SLOTS.acquire();
            acquired = true;
            return generateInSlot(video, displayName);
        } catch (InterruptedException error) {
            Thread.currentThread().interrupt();
            return false;
        } finally {
            if (acquired) GENERATION_SLOTS.release();
        }
    }

    private static boolean generateInSlot(File video, String displayName) {
        File videoDirectory = video.getParentFile();
        if (videoDirectory == null || !video.isFile()) {
            return false;
        }
        File thumbnailDirectory = new File(videoDirectory, ".thumbnails");
        if (!thumbnailDirectory.isDirectory() && !thumbnailDirectory.mkdirs()) {
            return false;
        }
        File destination = new File(thumbnailDirectory, displayName + ".jpg");
        if (destination.isFile() && destination.length() > 0L
                && destination.lastModified() >= video.lastModified()) {
            return false;
        }

        MediaMetadataRetriever retriever = new MediaMetadataRetriever();
        Bitmap frame = null;
        Bitmap scaled = null;
        File pending = new File(thumbnailDirectory, displayName + ".jpg.part");
        try {
            retriever.setDataSource(video.getAbsolutePath());
            frame = scaledFrame(retriever);
            if (frame == null) {
                return false;
            }
            Bitmap outputBitmap = frame;
            if (frame.getWidth() > MAX_WIDTH) {
                int height = Math.max(1, Math.round(
                        frame.getHeight() * (MAX_WIDTH / (float) frame.getWidth())));
                scaled = Bitmap.createScaledBitmap(frame, MAX_WIDTH, height, true);
                outputBitmap = scaled;
            }
            if (pending.exists() && !pending.delete()) {
                return false;
            }
            try (FileOutputStream output = new FileOutputStream(pending)) {
                if (!outputBitmap.compress(Bitmap.CompressFormat.JPEG, 84, output)) {
                    throw new IOException("Could not encode thumbnail");
                }
                output.flush();
                output.getFD().sync();
            }
            if (destination.exists() && !destination.delete()) {
                return false;
            }
            if (!pending.renameTo(destination)) {
                return false;
            }
            destination.setLastModified(video.lastModified());
            return true;
        } catch (RuntimeException | IOException error) {
            return false;
        } finally {
            pending.delete();
            if (scaled != null) {
                scaled.recycle();
            }
            if (frame != null) {
                frame.recycle();
            }
            try {
                retriever.release();
            } catch (RuntimeException | IOException ignored) {
            }
        }
    }

    private static Bitmap scaledFrame(MediaMetadataRetriever retriever) {
        try {
            int width = Integer.parseInt(retriever.extractMetadata(
                    MediaMetadataRetriever.METADATA_KEY_VIDEO_WIDTH));
            int height = Integer.parseInt(retriever.extractMetadata(
                    MediaMetadataRetriever.METADATA_KEY_VIDEO_HEIGHT));
            if (width > MAX_WIDTH && height > 0) {
                int scaledHeight = Math.max(1, Math.round(
                        height * (MAX_WIDTH / (float) width)));
                return retriever.getScaledFrameAtTime(
                        -1L,
                        MediaMetadataRetriever.OPTION_CLOSEST_SYNC,
                        MAX_WIDTH,
                        scaledHeight);
            }
        } catch (RuntimeException ignored) {
        }
        return retriever.getFrameAtTime(-1L, MediaMetadataRetriever.OPTION_CLOSEST_SYNC);
    }
}
