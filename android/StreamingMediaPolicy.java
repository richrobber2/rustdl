package app.rustdl;
import java.net.URI;
import java.util.Locale;
/** Pure resolver policy; never opens a connection. */
final class StreamingMediaPolicy {
    static boolean playable(String value) {
        try {if(value==null||value.length()>16000)return false;URI uri=new URI(value);String host=uri.getHost();
            if(!"https".equals(uri.getScheme())||host==null||host.equalsIgnoreCase("localhost")||host.contains(":")||host.matches("[0-9.]+")||uri.getUserInfo()!=null)return false;
            String path=uri.getPath()==null?"":uri.getPath().toLowerCase(Locale.ROOT);
            return !path.endsWith("/init.mp4")&&(path.endsWith(".m3u8")||path.endsWith(".mp4"));
        }catch(Exception invalid){return false;}
    }
}
