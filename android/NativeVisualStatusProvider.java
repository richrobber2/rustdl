package app.rustdl;
import android.content.ContentProvider;
import android.content.ContentValues;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
/** Shell-only, synthetic readiness state; exposes no files, preferences or media data. */
public final class NativeVisualStatusProvider extends ContentProvider {
    static volatile String readyScreen="";
    @Override public boolean onCreate(){return true;}
    @Override public Bundle call(String method,String arg,Bundle extras){
        Bundle result=new Bundle();
        result.putString("readyScreen","ready".equals(method)?readyScreen:"");
        return result;
    }
    @Override public Cursor query(Uri uri,String[] projection,String selection,String[] args,String order){return null;}
    @Override public String getType(Uri uri){return null;}
    @Override public Uri insert(Uri uri,ContentValues values){throw new UnsupportedOperationException();}
    @Override public int update(Uri uri,ContentValues values,String selection,String[] args){throw new UnsupportedOperationException();}
    @Override public int delete(Uri uri,String selection,String[] args){throw new UnsupportedOperationException();}
}
