package app.rustdl;

import android.graphics.Rect;
import android.os.Bundle;
import android.view.MotionEvent;
import android.view.View;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityManager;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityNodeProvider;
import org.json.JSONArray;
import org.json.JSONObject;
import java.util.LinkedHashMap;
import java.util.Map;

/** Runtime semantic adapter. Never a debug tree or a source of media identity. */
final class NativeAccessibility extends AccessibilityNodeProvider {
    private final View host;
    private final AccessibilityManager manager;
    private final Map<Integer,JSONObject> nodes=new LinkedHashMap<>();
    private final Map<String,Integer> ids=new LinkedHashMap<>();
    private int nextId=1,root=-1,focused=-1,keyboardFocused=-1,hovered=-1,screen=-1;
    private long revision=-1;
    private boolean visible,enabled,attached;
    private String previous="";
    private final AccessibilityManager.AccessibilityStateChangeListener listener=value->sync();
    private final Runnable poll=new Runnable() {public void run() {
        if(!enabled) return;
        refresh();host.postDelayed(this,100);
    }};
    NativeAccessibility(View host) {
        this.host=host;
        manager=(AccessibilityManager)host.getContext().getSystemService(android.content.Context.ACCESSIBILITY_SERVICE);
        host.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_YES);
    }
    void attached() {attached=true;if(manager!=null) manager.addAccessibilityStateChangeListener(listener);sync();}
    void detached() {attached=false;if(manager!=null)manager.removeAccessibilityStateChangeListener(listener);sync();}
    void visible(boolean value) {visible=value;sync();}
    void invalidate() {
        nodes.clear();ids.clear();root=focused=keyboardFocused=hovered=-1;previous="";
        host.sendAccessibilityEvent(AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED);
    }
    private void sync() {
        boolean next=visible&&attached&&manager!=null&&manager.isEnabled();
        if(enabled==next)return;
        enabled=next;host.removeCallbacks(poll);invalidate();
        NativeHomeHost.nativeAccessibilityActive(next);
        if(next)host.post(poll);
    }
    private int id(String semantic) {
        Integer existing=ids.get(semantic);if(existing!=null)return existing;
        if(nextId==Integer.MAX_VALUE)throw new IllegalStateException("Virtual identifier exhausted");
        int id=nextId++;ids.put(semantic,id);return id;
    }
    private void refresh() {
        try {
            String data=NativeHomeHost.nativeAccessibilitySnapshot();
            if(data==null||data.length()>4194304) {invalidate();return;}
            if(data.equals(previous))return;
            JSONObject page=new JSONObject(data);
            long nextRevision=Long.parseLong(page.getString("revision"));int nextScreen=page.getInt("screen");
            JSONArray rows=page.getJSONArray("nodes");if(rows.length()>4096) {invalidate();return;}
            if(revision!=nextRevision||screen!=nextScreen)invalidate();
            revision=nextRevision;screen=nextScreen;
            Map<Integer,JSONObject> replacement=new LinkedHashMap<>();
            for(int i=0;i<rows.length();i++) {JSONObject row=rows.getJSONObject(i);replacement.put(id(row.getString("id")),row);}
            nodes.clear();nodes.putAll(replacement);
            ids.entrySet().removeIf(entry->!nodes.containsKey(entry.getValue()));
            root=page.isNull("root")?-1:id(page.getString("root"));
            keyboardFocused=page.isNull("focus")?-1:id(page.getString("focus"));
            if(!nodes.containsKey(focused))focused=-1;
            previous=data;host.sendAccessibilityEvent(AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED);
        }catch(RuntimeException|org.json.JSONException invalid) {invalidate();}
    }
    private Rect bounds(JSONObject row) {
        JSONArray values=row.optJSONArray("bounds");if(values==null||values.length()!=4)return new Rect();
        // GPUI element.rs already emits AccessKit bounds in physical pixels.
        double[] coordinates=new double[4];for(int i=0;i<4;i++) {coordinates[i]=values.optDouble(i,Double.NaN);if(!Double.isFinite(coordinates[i])||Math.abs(coordinates[i])>Integer.MAX_VALUE/2)return new Rect();}
        return new Rect((int)Math.floor(coordinates[0]),(int)Math.floor(coordinates[1]),(int)Math.ceil(coordinates[2]),(int)Math.ceil(coordinates[3]));
    }
    private boolean supports(JSONObject row,int code) {
        JSONArray actions=row.optJSONArray("actions");if(actions==null)return false;
        for(int i=0;i<actions.length();i++)if(actions.optInt(i)==code)return true;return false;
    }
    private static CharSequence text(JSONObject row,String field) {return row.isNull(field)?null:row.optString(field,"");}
    @Override public AccessibilityNodeInfo createAccessibilityNodeInfo(int virtualId) {
        if(virtualId==View.NO_ID) {
            AccessibilityNodeInfo info=AccessibilityNodeInfo.obtain(host);host.onInitializeAccessibilityNodeInfo(info);
            if(enabled&&root!=-1)info.addChild(host,root);return info;
        }
        JSONObject row=nodes.get(virtualId);if(!enabled||row==null)return null;
        AccessibilityNodeInfo info=AccessibilityNodeInfo.obtain();info.setSource(host,virtualId);
        info.setPackageName(host.getContext().getPackageName());
        String role=row.optString("role");info.setClassName(role.equals("Button")?"android.widget.Button":role.equals("Slider")?"android.widget.SeekBar":role.equals("ScrollView")?"android.widget.ScrollView":role.equals("CheckBox")?"android.widget.CheckBox":role.equals("Switch")?"android.widget.Switch":role.equals("RadioButton")?"android.widget.RadioButton":"android.view.View");
        Integer parent=ids.get(row.optString("parent"));if(parent==null)info.setParent(host);else info.setParent(host,parent);
        String semantic=row.optString("id");for(Map.Entry<Integer,JSONObject> child:nodes.entrySet())if(semantic.equals(child.getValue().optString("parent")))info.addChild(host,child.getKey());
        Rect rect=bounds(row);Rect parentRect=parent==null?new Rect():bounds(nodes.get(parent));
        Rect relative=new Rect(rect);relative.offset(-parentRect.left,-parentRect.top);info.setBoundsInParent(relative);
        int[] location=new int[2];host.getLocationOnScreen(location);rect.offset(location[0],location[1]);info.setBoundsInScreen(rect);
        info.setVisibleToUser(host.isShown()&&!rect.isEmpty());info.setEnabled(!row.optBoolean("disabled"));
        info.setContentDescription(text(row,"label"));info.setText(text(row,"value"));if(android.os.Build.VERSION.SDK_INT>=30)info.setStateDescription(text(row,"description"));
        boolean meaningful=!row.isNull("label")||!row.isNull("value")||(row.optJSONArray("actions")!=null&&row.optJSONArray("actions").length()>0);
        info.setFocusable(meaningful);info.setFocused(keyboardFocused==virtualId);info.setAccessibilityFocused(focused==virtualId);
        if(!row.isNull("selected"))info.setSelected(row.optBoolean("selected"));
        if(!row.isNull("checked")) {info.setCheckable(true);info.setChecked(row.optBoolean("checked"));}
        if(row.optBoolean("mixed")&&android.os.Build.VERSION.SDK_INT>=30)info.setStateDescription("Partially selected");
        info.addAction(focused==virtualId?AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS:AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS);
        if(supports(row,1)) {info.setClickable(true);info.addAction(AccessibilityNodeInfo.ACTION_CLICK);}
        if(supports(row,2))info.addAction(AccessibilityNodeInfo.ACTION_FOCUS);
        if(supports(row,3))info.addAction(AccessibilityNodeInfo.ACTION_CLEAR_FOCUS);
        info.setScrollable(supports(row,10)||supports(row,11));
        if(supports(row,4)||supports(row,10))info.addAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);
        if(supports(row,5)||supports(row,11))info.addAction(AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD);
        if(supports(row,6))info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_PROGRESS);
        if(supports(row,7))info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN);
        if(supports(row,8))info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_EXPAND);
        if(supports(row,9))info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_COLLAPSE);
        if(!row.isNull("minimum")&&!row.isNull("maximum")&&!row.isNull("numeric_value")) {
            float min=(float)row.optDouble("minimum"),max=(float)row.optDouble("maximum"),value=(float)row.optDouble("numeric_value");
            if(Float.isFinite(min)&&Float.isFinite(max)&&Float.isFinite(value)&&min<=max)info.setRangeInfo(AccessibilityNodeInfo.RangeInfo.obtain(AccessibilityNodeInfo.RangeInfo.RANGE_TYPE_FLOAT,min,max,value));
        }
        return info;
    }
    @Override public AccessibilityNodeInfo findFocus(int focusType) {
        int target=focusType==AccessibilityNodeInfo.FOCUS_ACCESSIBILITY?focused:focusType==AccessibilityNodeInfo.FOCUS_INPUT?keyboardFocused:-1;
        return target==-1?null:createAccessibilityNodeInfo(target);
    }
    private void event(int id,int type) {
        AccessibilityEvent event=AccessibilityEvent.obtain(type);event.setSource(host,id);event.setPackageName(host.getContext().getPackageName());
        if(host.getParent()!=null)host.getParent().requestSendAccessibilityEvent(host,event);
    }
    @Override public boolean performAction(int virtualId,int action,Bundle arguments) {
        JSONObject row=nodes.get(virtualId);if(!enabled||row==null)return false;
        if(action==AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS) {if(focused!=-1)event(focused,AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED);focused=virtualId;event(virtualId,AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED);return true;}
        if(action==AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS) {if(focused!=virtualId)return false;focused=-1;event(virtualId,AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED);return true;}
        int code=action==AccessibilityNodeInfo.ACTION_CLICK?1:action==AccessibilityNodeInfo.ACTION_FOCUS?2:action==AccessibilityNodeInfo.ACTION_CLEAR_FOCUS?3:action==AccessibilityNodeInfo.ACTION_SCROLL_FORWARD?(supports(row,10)?10:4):action==AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD?(supports(row,11)?11:5):action==AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_PROGRESS.getId()?6:action==AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId()?7:action==AccessibilityNodeInfo.AccessibilityAction.ACTION_EXPAND.getId()?8:action==AccessibilityNodeInfo.AccessibilityAction.ACTION_COLLAPSE.getId()?9:0;
        if(!supports(row,code)||row.optBoolean("disabled"))return false;
        double value=code==6&&arguments!=null?arguments.getFloat(AccessibilityNodeInfo.ACTION_ARGUMENT_PROGRESS_VALUE,Float.NaN):0;
        return NativeHomeHost.nativeAccessibilityAction(row.optString("id"),code,value,revision,screen);
    }
    boolean hover(MotionEvent motion) {
        if(!enabled||manager==null||!manager.isTouchExplorationEnabled())return false;
        int hit=-1;if(motion.getActionMasked()!=MotionEvent.ACTION_HOVER_EXIT)for(Map.Entry<Integer,JSONObject> entry:nodes.entrySet())if((!entry.getValue().isNull("label")||!entry.getValue().isNull("value")||(entry.getValue().optJSONArray("actions")!=null&&entry.getValue().optJSONArray("actions").length()>0))&&bounds(entry.getValue()).contains((int)motion.getX(),(int)motion.getY()))hit=entry.getKey();
        if(hit!=hovered) {if(hovered!=-1)event(hovered,AccessibilityEvent.TYPE_VIEW_HOVER_EXIT);hovered=hit;if(hit!=-1)event(hit,AccessibilityEvent.TYPE_VIEW_HOVER_ENTER);}
        return hit!=-1||motion.getActionMasked()==MotionEvent.ACTION_HOVER_EXIT;
    }
}
