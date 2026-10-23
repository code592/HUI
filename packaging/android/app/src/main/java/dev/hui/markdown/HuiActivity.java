package dev.hui.markdown;

import android.app.NativeActivity;
import android.content.*;
import android.net.Uri;
import android.os.Bundle;
import android.provider.OpenableColumns;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.UUID;
import java.util.concurrent.Executors;
import java.util.concurrent.ExecutorService;

/** SAF owns external files; private mirrors support the existing Rope/storage engine. */
public final class HuiActivity extends NativeActivity {
    // NativeActivity uses dlopen; register the library with this Java ClassLoader too.
    static { System.loadLibrary("hui_app"); }
    private static final int OPEN = 10, SAVE = 11;
    private static final long LIMIT = 50L * 1024 * 1024;
    private final ExecutorService io = Executors.newSingleThreadExecutor();
    private volatile boolean ready;
    private String pendingCopy;
    private boolean pendingDocument;
    private native void nativeEvent(String kind, String value);
    private SharedPreferences mappings() { return getSharedPreferences("documents", MODE_PRIVATE); }
    @Override public void onCreate(Bundle state) {
        // Resources are versioned and installed before the Rust native entry point starts.
        try {
            File marker = new File(getFilesDir(), "assets-v3");
            if (!marker.exists()) { copyAssets("katex", new File(getFilesDir(), "katex")); marker.createNewFile(); }
        } catch (IOException e) { throw new IllegalStateException("Cannot install document assets", e); }
        if (state != null) { pendingCopy = state.getString("pendingCopy"); pendingDocument = state.getBoolean("pendingDocument"); }
        super.onCreate(state);
    }
    private void copyAssets(String source, File target) throws IOException {
        String[] children = getAssets().list(source);
        if (children != null && children.length > 0) {
            if (!target.isDirectory() && !target.mkdirs()) throw new IOException("Cannot create asset directory");
            for (String child : children) copyAssets(source + "/" + child, new File(target, child));
        } else {
            try (InputStream in = getAssets().open(source); OutputStream out = new FileOutputStream(target)) { transfer(in, out, Long.MAX_VALUE); }
        }
    }
    public String documentName(String path) { return mappings().getString(path+".name", ""); }
    public String systemLanguage() { return getResources().getConfiguration().getLocales().get(0).toLanguageTag(); }
    public String dataDirectory() { return getFilesDir().getAbsolutePath(); }
    public String nativeReady() { runOnUiThread(() -> { ready = true; handleIntent(getIntent()); }); return ""; }
    private void event(String kind, String value) { if (ready) nativeEvent(kind, value); }
    private void error(Exception e) { event("notice", e.getMessage() == null ? e.toString() : e.getMessage()); }
    @Override protected void onNewIntent(Intent intent) { super.onNewIntent(intent); setIntent(intent); if (ready) handleIntent(intent); }
    @Override protected void onPause() { if (ready) event("pause", ""); super.onPause(); }
    @Override protected void onSaveInstanceState(Bundle state) { state.putString("pendingCopy", pendingCopy); state.putBoolean("pendingDocument", pendingDocument); super.onSaveInstanceState(state); }
    private void handleIntent(Intent intent) {
        if (intent == null) return;
        Uri uri = intent.getData();
        if (Intent.ACTION_SEND.equals(intent.getAction())) uri = intent.getParcelableExtra(Intent.EXTRA_STREAM);
        if (uri != null && "content".equals(uri.getScheme())) { importUri(uri, intent.getFlags()); return; }
        if (Intent.ACTION_SEND.equals(intent.getAction()) && intent.getCharSequenceExtra(Intent.EXTRA_TEXT) != null) {
            String text = intent.getCharSequenceExtra(Intent.EXTRA_TEXT).toString();
            io.execute(() -> { try { File file = privateDocument("Shared.md"); try (OutputStream out = new FileOutputStream(file)) { out.write(text.getBytes(StandardCharsets.UTF_8)); } event("open", file.getAbsolutePath()); } catch (Exception e) { error(e); } });
        }
    }
    public String openDocument() {
        runOnUiThread(() -> { Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType("*/*");
            intent.putExtra(Intent.EXTRA_MIME_TYPES, new String[]{"text/*", "application/octet-stream"});
            intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
            startActivityForResult(intent, OPEN);
        }); return "";
    }
    public String saveCopy(String path, String name) { return saveFile(path, name, false); }
    public String saveDocument(String path, String name) { return saveFile(path, name, true); }
    private String saveFile(String path, String name, boolean document) {
        runOnUiThread(() -> {
            if (pendingCopy != null) { if (document) event("cancel-save", path); event("notice", "Finish the current Save As dialog first."); return; }
            pendingCopy = path; pendingDocument = document;
            Intent intent = new Intent(Intent.ACTION_CREATE_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE)
                .setType(name.endsWith(".pdf") ? "application/pdf" : name.endsWith(".html") ? "text/html" : "text/markdown")
                .putExtra(Intent.EXTRA_TITLE, name);
            startActivityForResult(intent, SAVE);
        }); return "";
    }
    @Override protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request == OPEN && result == RESULT_OK && data != null && data.getData() != null) importUri(data.getData(), data.getFlags());
        if (request == SAVE) {
            String path = pendingCopy; boolean document = pendingDocument; pendingCopy = null; pendingDocument = false;
            if (result != RESULT_OK || data == null || data.getData() == null || path == null) { if (document && path != null) event("cancel-save", path); return; }
            Uri uri = data.getData();
            io.execute(() -> { try {
                try (InputStream in = new FileInputStream(path); OutputStream out = getContentResolver().openOutputStream(uri, "wt")) {
                    if (out == null) throw new IOException("Document provider refused writing"); transfer(in, out, Long.MAX_VALUE);
                }
                if (document) {
                    int grant = data.getFlags() & (Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
                    try { getContentResolver().takePersistableUriPermission(uri, grant); } catch (SecurityException ignored) { }
                    String sha; try (InputStream in = new FileInputStream(path)) { sha = digest(in); }
                    mappings().edit().putString(path+".uri", uri.toString()).putString(path+".sha", sha).putString(path+".name", displayName(uri)).commit();
                    event("saved", path);
                }
                event("notice", "Saved: " + displayName(uri));
            } catch (Exception e) { if (document) event("cancel-save", path); error(e); } });
        }
    }
    private String displayName(Uri uri) {
        try (android.database.Cursor cursor = getContentResolver().query(uri, new String[]{OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (cursor != null && cursor.moveToFirst()) return cursor.getString(0);
        } catch (Exception ignored) { }
        return "Document.md";
    }
    private File privateDocument(String name) throws IOException {
        File dir = new File(getFilesDir(), "documents/" + UUID.randomUUID());
        if (!dir.mkdirs()) throw new IOException("Cannot create document directory");
        name = name.replaceAll("[\\\\/\\p{Cntrl}]", "_");
        if (!(name.endsWith(".md") || name.endsWith(".markdown"))) name += ".md";
        return new File(dir, name);
    }
    private void importUri(Uri uri, int flags) {
        io.execute(() -> { try {
            int grant = flags & (Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
            try { getContentResolver().takePersistableUriPermission(uri, grant); } catch (SecurityException ignored) { /* Temporary share grants remain usable for this session. */ }
            File file = privateDocument(displayName(uri));
            try (InputStream in = getContentResolver().openInputStream(uri); OutputStream out = new FileOutputStream(file)) {
                if (in == null) throw new IOException("Cannot read document"); transfer(in, out, LIMIT);
            } catch (Exception e) { file.delete(); throw e; }
            String digest; try (InputStream in = new FileInputStream(file)) { digest = digest(in); }
            mappings().edit().putString(file.getAbsolutePath()+".uri", uri.toString()).putString(file.getAbsolutePath()+".sha", digest).commit();
            event("open", file.getAbsolutePath());
        } catch (Exception e) { error(e); } });
    }
    /** Called on Rust's save worker, never on the Android UI thread. */
    public synchronized String syncDocument(String path) {
        String value = mappings().getString(path+".uri", null);
        if (value == null) return ""; // Recovery and export files are private.
        try {
            Uri uri = Uri.parse(value);
            String expected = mappings().getString(path+".sha", "");
            try (InputStream in = getContentResolver().openInputStream(uri)) {
                if (in == null || !digest(in).equals(expected)) return "External document changed. Your edits are kept locally; use Save As.";
            }
            String updated; try (InputStream in = new FileInputStream(path)) { updated = digest(in); }
            try (InputStream in = new FileInputStream(path); OutputStream out = getContentResolver().openOutputStream(uri, "wt")) {
                if (out == null) throw new IOException("Document is read-only"); transfer(in, out, LIMIT);
            }
            mappings().edit().putString(path+".sha", updated).commit(); return "";
        } catch (Exception e) { return "Original file could not be saved; local recovery retained. " + e; }
    }
    public String copyText(String value) { runOnUiThread(() -> ((android.content.ClipboardManager)getSystemService(CLIPBOARD_SERVICE)).setPrimaryClip(ClipData.newPlainText("HUI", value))); return ""; }
    public String pasteText() {
        android.content.ClipboardManager clipboard = (android.content.ClipboardManager)getSystemService(CLIPBOARD_SERVICE);
        ClipData clip = clipboard.getPrimaryClip();
        return clip != null && clip.getItemCount() > 0 ? clip.getItemAt(0).coerceToText(this).toString() : "";
    }
    @Override protected void onDestroy() { ready = false; io.shutdown(); super.onDestroy(); }
    private static void transfer(InputStream in, OutputStream out, long limit) throws IOException {
        byte[] buffer = new byte[65536]; long total=0; int n;
        while ((n=in.read(buffer)) != -1) { total += n; if (total > limit) throw new IOException("Document exceeds the 50 MiB import limit"); out.write(buffer,0,n); }
    }
    private static String digest(InputStream in) throws Exception {
        MessageDigest md = MessageDigest.getInstance("SHA-256"); byte[] buffer = new byte[65536]; int n;
        while ((n=in.read(buffer)) != -1) md.update(buffer,0,n);
        StringBuilder result = new StringBuilder(); for (byte b:md.digest()) result.append(String.format(java.util.Locale.ROOT,"%02x",b)); return result.toString();
    }
}
