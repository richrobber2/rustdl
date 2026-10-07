package app.rustdl;

import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import org.json.JSONArray;
import org.json.JSONObject;
import java.io.*;
import java.net.*;
import java.security.MessageDigest;
import java.util.concurrent.*;
import java.util.function.Consumer;

/** App-runtime poster loading; decoder/media identities never enter this cache. */
final class NativeAnimePosters {
    private static final ThreadPoolExecutor WORK = new ThreadPoolExecutor(2,2,30,TimeUnit.SECONDS,
            new ArrayBlockingQueue<>(32),r->{Thread t=new Thread(r,"anime-posters");t.setDaemon(true);return t;},
            new ThreadPoolExecutor.AbortPolicy());
    static boolean allowed(String address) {
        try {URI uri=new URI(address);String host=uri.getHost();return "https".equals(uri.getScheme())&&host!=null
            &&!host.equalsIgnoreCase("localhost")&&!host.contains(":")&&!host.matches("[0-9.]+")
            &&uri.getUserInfo()==null&&uri.getFragment()==null&&address.length()<=4096;}catch(Exception invalid){return false;}
    }
    static String load(Context context, JSONObject original, boolean privacy, Consumer<String> update) {
        try {
            JSONObject page=new JSONObject(original.toString());JSONArray items=page.optJSONArray("items");
            if(items==null) return page.toString();
            synchronized(page) {
            for(int i=0;i<Math.min(25,items.length());i++) {
                JSONObject item=items.optJSONObject(i);if(item==null)continue;
                String url=item.optString("poster");item.put("poster","");item.put("posterWidth",0);item.put("posterHeight",0);
                if(privacy||!allowed(url))continue;
                final int index=i;
                try {WORK.execute(()->{
                    String path=fetch(context.getCacheDir(),url);if(path.isEmpty())return;
                    synchronized(page) {try {BitmapFactory.Options bounds=new BitmapFactory.Options();bounds.inJustDecodeBounds=true;BitmapFactory.decodeFile(path,bounds);JSONObject loaded=items.getJSONObject(index);loaded.put("poster",path);loaded.put("posterWidth",Math.max(0,bounds.outWidth));loaded.put("posterHeight",Math.max(0,bounds.outHeight));update.accept(page.toString());}catch(Exception ignored){}}
                });}catch(RejectedExecutionException busy){}
            }
            return page.toString();
            }
        }catch(Exception invalid){return original.toString();}
    }
    private static String fetch(File cache,String address) {
        HttpURLConnection connection=null;Bitmap bitmap=null;
        try {
            StringBuilder hash=new StringBuilder();for(byte b:MessageDigest.getInstance("SHA-256").digest(address.getBytes("UTF-8")))hash.append(String.format("%02x",b&255));
            File directory=new File(cache,"anime-posters");if(!directory.isDirectory()&&!directory.mkdirs())return "";
            File result=new File(directory,hash+".png");if(result.isFile()&&result.length()>0)return result.getAbsolutePath();
            connection=(HttpURLConnection)new URL(address).openConnection();connection.setInstanceFollowRedirects(false);
            connection.setConnectTimeout(5000);connection.setReadTimeout(5000);
            if(connection.getResponseCode()!=200||connection.getContentLengthLong()>2*1024*1024)return "";
            ByteArrayOutputStream bytes=new ByteArrayOutputStream();
            try(InputStream input=connection.getInputStream()){byte[] block=new byte[8192];int count;while((count=input.read(block))!=-1){if(bytes.size()+count>2*1024*1024)return "";bytes.write(block,0,count);}}
            byte[] data=bytes.toByteArray();BitmapFactory.Options options=new BitmapFactory.Options();options.inJustDecodeBounds=true;
            BitmapFactory.decodeByteArray(data,0,data.length,options);if(options.outWidth<=0||options.outHeight<=0||options.outWidth>16384||options.outHeight>16384)return "";
            options.inJustDecodeBounds=false;options.inSampleSize=1;while(Math.max(options.outWidth,options.outHeight)/options.inSampleSize>512)options.inSampleSize*=2;
            bitmap=BitmapFactory.decodeByteArray(data,0,data.length,options);if(bitmap==null)return "";
            File[] previous=directory.listFiles();if(previous!=null&&previous.length>=128){java.util.Arrays.sort(previous,(a,b)->Long.compare(a.lastModified(),b.lastModified()));for(int i=0;i<previous.length-127;i++)previous[i].delete();}
            File temporary=File.createTempFile("poster-",".tmp",directory);
            try(FileOutputStream output=new FileOutputStream(temporary)){if(!bitmap.compress(Bitmap.CompressFormat.PNG,100,output)){temporary.delete();return "";}}
            if(!temporary.renameTo(result)){temporary.delete();return "";}return result.getAbsolutePath();
        }catch(Exception failed){return "";}finally{if(connection!=null)connection.disconnect();if(bitmap!=null)bitmap.recycle();}
    }
}
