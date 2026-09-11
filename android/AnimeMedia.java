package app.rustdl;

import android.media.*;
import java.io.File;
import java.io.IOException;
import java.nio.ByteBuffer;

/** Local media conversion; does not request URLs or open a player. */
final class AnimeMedia {
    interface Cancellation { void check() throws IOException; }
    static void remux(File videoFile, File audioFile, File output, Cancellation cancellation) throws IOException {
        MediaExtractor video = new MediaExtractor(), audio = new MediaExtractor();
        MediaMuxer muxer = null; boolean started = false;
        try {
            video.setDataSource(videoFile.getAbsolutePath()); audio.setDataSource(audioFile.getAbsolutePath());
            muxer = new MediaMuxer(output.getAbsolutePath(), MediaMuxer.OutputFormat.MUXER_OUTPUT_MPEG_4);
            int videoTrack = selectTrack(video, muxer, "video/");
            int audioTrack = selectTrack(audio, muxer, "audio/");
            if (videoTrack < 0) throw new IOException("This source has no downloadable video track");
            long start = Math.max(0, video.getSampleTime());
            if (audioTrack >= 0) start = Math.min(start, Math.max(0, audio.getSampleTime()));
            muxer.start(); started = true;
            copyTrack(video, muxer, videoTrack, start, cancellation);
            if (audioTrack >= 0) copyTrack(audio, muxer, audioTrack, start, cancellation);
        } finally {
            try { if (muxer != null) { if (started) muxer.stop(); } }
            finally { if (muxer != null) muxer.release(); video.release(); audio.release(); }
        }
    }

    private static int selectTrack(MediaExtractor extractor, MediaMuxer muxer, String prefix) {
        for (int i = 0; i < extractor.getTrackCount(); i++) {
            MediaFormat format = extractor.getTrackFormat(i); String mime = format.getString(MediaFormat.KEY_MIME);
            if (mime != null && mime.startsWith(prefix)) { extractor.selectTrack(i); return muxer.addTrack(format); }
        }
        return -1;
    }

    private static void copyTrack(MediaExtractor source, MediaMuxer muxer, int track, long start, Cancellation cancellation) throws IOException {
        ByteBuffer buffer = ByteBuffer.allocateDirect(8 * 1024 * 1024); MediaCodec.BufferInfo info = new MediaCodec.BufferInfo();
        while (true) {
            cancellation.check(); buffer.clear(); int count = source.readSampleData(buffer, 0); if (count < 0) break;
            if (count > buffer.capacity()) throw new IOException("Video sample is too large");
            info.set(0, count, Math.max(0, source.getSampleTime() - start), source.getSampleFlags());
            muxer.writeSampleData(track, buffer, info); source.advance();
        }
    }

}
