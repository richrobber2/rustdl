package app.rustdl;

import android.view.KeyEvent;
import android.view.View;

/** Optional native UI capability; standard APKs do not load GPUI. */
interface NativeHome {
    View view();
    void setActive(boolean active);
    void update(boolean dark, int count, long downloaded, long total);
    boolean key(KeyEvent event);
    boolean ready();
    boolean back();
    int screen();
    boolean privacyReady();
    void updateSettings(String settings);
    void updateQueue(String queue);
    void showQueue();
    void showDiscovery();
    void showPeers();
    void updatePeers(String peers);
    void updateAnime(String anime);
    void updateStorage(String storage);
    void updateActivity(String activity);
    void updateUpdates(String updates);
    void updateStreaming(String data);
    void showStreaming();
    void updatePlayer(String player);
    void showPlayer();
    void playerLayout(int layout);
    void resumePlayer();
    void openPlayer(String filename,String source);
    void reopenPlayer(String filename,String source,double position,boolean playing);
    void playerCommand(String action,double value);
    void playerPrivacy(boolean enabled);
    void playerPip(boolean enabled);
    void playerVisible(boolean enabled);
    void playerSurface(int x,int y,int width,int height);
    void updateDiagnostics(String diagnostics);
    void updateLibrary(String library);
    void updateDiscovery(String discovery);
    void destroy();
}
