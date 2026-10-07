package app.rustdl;

import java.util.ArrayList;
import java.util.Collection;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/** App-owned download selections; presentation uses indexes and format labels only. */
final class StreamingDownloadChoices {
    static final class Choice {
        final String source;
        final boolean hls;
        Choice(String source, boolean hls) { this.source=source; this.hls=hls; }
        String label(int index) { return (hls ? "HLS" : "MP4")+" · "+(index+1); }
    }
    static List<Choice> select(Collection<String> sources, Set<String> hlsSources) {
        List<Choice> all=new ArrayList<>(), hls=new ArrayList<>();
        Set<String> seen=new HashSet<>();
        for(String source:sources) {
            if(source==null||source.isEmpty()||!seen.add(source)) continue;
            Choice choice=new Choice(source,hlsSources.contains(source));
            all.add(choice); if(choice.hls) hls.add(choice);
        }
        return hls.isEmpty() ? all : hls;
    }
    private StreamingDownloadChoices() {}
}
