package app.rustdl;
import android.media.*;
import java.io.*;
import java.nio.ByteBuffer;
import java.security.MessageDigest;
import java.util.Arrays;

/** Generates synthetic encoded data and compares track hashes; never opens a player. */
public class AnimeMediaTest {
    static void encode(File file, boolean audio) throws Exception {
        String mime = audio ? "audio/mp4a-latm" : "video/avc";
        MediaFormat format = audio ? MediaFormat.createAudioFormat(mime,44100,1) : MediaFormat.createVideoFormat(mime,320,240);
        format.setInteger(MediaFormat.KEY_BIT_RATE,audio?64000:150000);
        if (audio) format.setInteger(MediaFormat.KEY_AAC_PROFILE,MediaCodecInfo.CodecProfileLevel.AACObjectLC);
        else { format.setInteger(MediaFormat.KEY_FRAME_RATE,10);format.setInteger(MediaFormat.KEY_I_FRAME_INTERVAL,1);format.setInteger(MediaFormat.KEY_COLOR_FORMAT,MediaCodecInfo.CodecCapabilities.COLOR_FormatYUV420Flexible); }
        MediaCodec codec = MediaCodec.createEncoderByType(mime);
        MediaMuxer mux = new MediaMuxer(file.getAbsolutePath(),MediaMuxer.OutputFormat.MUXER_OUTPUT_MPEG_4);
        boolean started=false;int track=-1;int frame=0;boolean inputDone=false;boolean done=false;
        byte[] data = new byte[audio?2048:320*240*3/2];Arrays.fill(data,(byte)(audio?0:128));
        try {
            codec.configure(format,null,null,MediaCodec.CONFIGURE_FLAG_ENCODE);codec.start();
            long deadline=System.currentTimeMillis()+20000;
            MediaCodec.BufferInfo info=new MediaCodec.BufferInfo();
            while(!done){
                if(System.currentTimeMillis()>deadline)throw new IOException("Synthetic encoder timeout");
                if(!inputDone){int index=codec.dequeueInputBuffer(10000);if(index>=0){
                    ByteBuffer input=codec.getInputBuffer(index);input.clear();
                    if(frame<8){input.put(data);codec.queueInputBuffer(index,0,data.length,frame*(audio?23219L:100000L),0);frame++;}
                    else{codec.queueInputBuffer(index,0,0,frame*(audio?23219L:100000L),MediaCodec.BUFFER_FLAG_END_OF_STREAM);inputDone=true;}
                }}
                int index=codec.dequeueOutputBuffer(info,10000);
                if(index==MediaCodec.INFO_OUTPUT_FORMAT_CHANGED){track=mux.addTrack(codec.getOutputFormat());mux.start();started=true;}
                else if(index>=0){
                    ByteBuffer output=codec.getOutputBuffer(index);
                    if((info.flags&MediaCodec.BUFFER_FLAG_CODEC_CONFIG)==0&&info.size>0)mux.writeSampleData(track,output,info);
                    done=(info.flags&MediaCodec.BUFFER_FLAG_END_OF_STREAM)!=0;codec.releaseOutputBuffer(index,false);
                }
            }
        } finally {codec.stop();codec.release();if(started)mux.stop();mux.release();}
    }
    static byte[] trackHash(File file,String prefix)throws Exception{
        MediaExtractor extractor=new MediaExtractor();MessageDigest hash=MessageDigest.getInstance("SHA-256");int samples=0;
        try{
            extractor.setDataSource(file.getAbsolutePath());int found=-1;
            for(int i=0;i<extractor.getTrackCount();i++)if(extractor.getTrackFormat(i).getString(MediaFormat.KEY_MIME).startsWith(prefix)){found=i;break;}
            if(found<0)throw new AssertionError("Missing expected track");extractor.selectTrack(found);ByteBuffer bytes=ByteBuffer.allocate(1024*1024);
            while(true){bytes.clear();int count=extractor.readSampleData(bytes,0);if(count<0)break;bytes.position(0);bytes.limit(count);hash.update(bytes);samples++;extractor.advance();}
            if(samples==0)throw new AssertionError("No media samples");return hash.digest();
        }finally{extractor.release();}
    }
    public static void main(String[] args)throws Exception{
        File root=new File(args[0]);root.mkdirs();File video=new File(root,"synthetic-video.mp4"),audio=new File(root,"synthetic-audio.m4a"),combined=new File(root,"combined.mp4"),silent=new File(root,"silent.mp4");
        encode(video,false);encode(audio,true);
        AnimeMedia.remux(video,audio,combined,()->{});
        if(!Arrays.equals(trackHash(video,"video/"),trackHash(combined,"video/"))||!Arrays.equals(trackHash(audio,"audio/"),trackHash(combined,"audio/")))throw new AssertionError("Remux changed sample hashes");
        AnimeMedia.remux(video,video,silent,()->{});
        if(!Arrays.equals(trackHash(video,"video/"),trackHash(silent,"video/")))throw new AssertionError("Video-only conversion changed samples");
        try{AnimeMedia.remux(video,audio,new File(root,"cancelled.mp4"),()->{throw new IOException("cancelled");});throw new AssertionError("Cancellation ignored");}catch(IOException expected){}
        System.out.println("Android media tests passed: video/audio sample hashes, video-only conversion, cancellation; no playback");
    }
}
