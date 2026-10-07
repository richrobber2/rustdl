package app.rustdl;
import java.util.ArrayList;
import java.util.List;
/** Pure selection rules shared by the Android fallback and GPUI decoder controls. */
final class StreamingControlPolicy {
    static boolean matches(String language,boolean available,String filter) {
        if("sub".equals(filter)||"dub".equals(filter)) return filter.equalsIgnoreCase(language);
        if("ready".equals(filter)) return available;
        if("issues".equals(filter)) return !available;
        return "all".equals(filter);
    }
    static List<Integer> matchingIndices(List<String> languages,List<Boolean> available,String filter) {
        List<Integer> result=new ArrayList<>();
        for(int index=0;index<Math.min(languages.size(),available.size());index++)
            if(matches(languages.get(index),available.get(index),filter)) result.add(index);
        return result;
    }
    static boolean currentGeneration(int expected,int current) { return expected>=0&&expected==current; }
    private StreamingControlPolicy() {}
}
