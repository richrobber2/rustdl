// Synthetic code, never compiled or executed.
class Unsafe {
    public void onReceivedSslError(WebView view, SslErrorHandler handler, SslError error) {
        // ruleid: rustdl.android.tls-error-bypass
        handler.proceed();
    }
    void fileAccess(WebSettings settings) {
        // ruleid: rustdl.android.universal-file-access
        settings.setAllowUniversalAccessFromFileURLs(true);
        // ok: rustdl.android.universal-file-access
        settings.setAllowUniversalAccessFromFileURLs(false);
    }
    // ruleid: rustdl.android.hostname-verifier-bypass
    public boolean verify(String hostname, SSLSession session) { return true; }
}
