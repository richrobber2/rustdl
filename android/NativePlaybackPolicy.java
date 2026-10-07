package app.rustdl;

import java.net.URI;

/** Pure validation shared by native playback adapters. */
final class NativePlaybackPolicy {
    private static final String MEDIA_NAME = "(?:[0-9]+-[1-9][0-9]*|youtube-[A-Za-z0-9_-]{11}|snapchat-[A-Za-z0-9_-]{20,160}|anime-[a-f0-9]{24})\\.(?:mp4|m4a)";
    static boolean validSource(String filename,String source,String origin) {
        if (filename==null||!filename.matches(MEDIA_NAME)||source==null||origin==null) return false;
        try {
            URI expected=new URI(origin),actual=new URI(source);
            return "http".equals(expected.getScheme())&&"127.0.0.1".equals(expected.getHost())
                    &&expected.getPort()>0&&expected.getPort()<=65535
                    &&actual.getScheme().equals(expected.getScheme())&&expected.getHost().equals(actual.getHost())
                    &&actual.getPort()==expected.getPort()&&actual.getRawUserInfo()==null
                    &&actual.getRawQuery()==null&&actual.getRawFragment()==null
                    &&(actual.getPath().equals("/media/"+filename)||actual.getPath().equals("/stream/"+filename));
        } catch (Exception invalid) {return false;}
    }
    static double seekSeconds(double requested,double duration) {
        if (!Double.isFinite(requested)||!Double.isFinite(duration)||duration<=0d) return 0d;
        return Math.max(0d,Math.min(requested,duration));
    }
    static double growingSeekSeconds(double requested,double current,double duration,double bufferedPercent,boolean growing) {
        double target=seekSeconds(requested,duration);
        if(!growing) return target;
        double position=seekSeconds(current,duration);
        if(target<=position) return target;
        double percent=Double.isFinite(bufferedPercent)?Math.max(0d,Math.min(100d,bufferedPercent)):0d;
        double boundary=Double.isFinite(duration)&&duration>0d?Math.max(0d,duration*percent/100d-.35d):0d;
        return Math.min(target,Math.max(position,boundary));
    }
    static boolean previewAvailable(double seconds,double duration,double percent,boolean growing) {
        if(!Double.isFinite(seconds)||!Double.isFinite(duration)||seconds<0d||duration<=0d||seconds>duration||seconds>Long.MAX_VALUE/1000000d) return false;
        return !growing||(Double.isFinite(percent)&&percent>0d&&seconds<=duration*Math.min(100d,percent)/100d);
    }
    static double doubleTapSeek(double x,double width) {
        if(!Double.isFinite(x)||!Double.isFinite(width)||width<=0d) return 0d;
        return x<width/2d?-10d:10d;
    }
    static boolean pipCommandAllowed(String command,boolean privacy) {return "pause".equals(command)||("play".equals(command)&&!privacy);}
    static boolean allowPip(boolean privacy,boolean visible,boolean destroyed,boolean supported,boolean audioOnly,boolean playing) {
        return supported&&!audioOnly&&playing&&showFrames(privacy,visible,destroyed);
    }
    static boolean growingRetryAllowed(boolean error,long downloaded,long previous,int attempts,long elapsed) {
        return error&&downloaded>0&&downloaded>previous&&attempts>=0&&attempts<3&&elapsed>=5000L;
    }
    static boolean playerScreen(int screen) {return screen==12||screen==15;}
    static boolean autoplayAllowed(boolean requested,boolean active,boolean destroyed) {return requested&&active&&!destroyed;}
    static boolean transitionCurrent(int expectedGeneration,int generation,long expectedRevision,long revision) {
        return expectedGeneration==generation&&expectedRevision==revision;
    }
    static boolean pauseForFocusChange(int change) {return change>=-3&&change<=-1;}
    static boolean validSleepMinutes(double value) {
        return value==0d||value==15d||value==30d||value==60d;
    }
    static boolean completed(boolean expectedPlaying,boolean decoderPlaying,double position,double duration) {
        return expectedPlaying&&!decoderPlaying&&Double.isFinite(position)&&Double.isFinite(duration)
                &&duration>0d&&position>=Math.max(0d,duration-.5d)&&position<=duration+1d;
    }
    static boolean completed(boolean expectedPlaying,boolean decoderPlaying,double position,double duration,boolean growing) {
        return !growing&&completed(expectedPlaying,decoderPlaying,position,duration);
    }
    static boolean allowFullscreen(boolean privacy,boolean visible,boolean destroyed,boolean playerScreen) {
        return playerScreen&&showFrames(privacy,visible,destroyed);
    }
    static boolean showFrames(boolean privacy,boolean visible,boolean destroyed) {
        return !privacy&&visible&&!destroyed;
    }
    static boolean artworkResultAllowed(long request,long current,boolean privacy,boolean active,boolean destroyed) {
        return request==current&&!privacy&&active&&!destroyed;
    }
    static int artworkSample(int width,int height) {
        if(width<=0||height<=0||width>16384||height>16384||(long)width*height>64L*1024*1024) return 0;
        int sample=1;while(Math.max(width,height)/sample>512) sample*=2;return sample;
    }
}
