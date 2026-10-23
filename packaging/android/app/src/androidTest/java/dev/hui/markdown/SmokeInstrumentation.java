package dev.hui.markdown;

import android.app.*;
import android.content.*;
import android.net.Uri;
import android.os.Bundle;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;

/** Runs against the real ContentResolver and isolated test document provider. */
public final class SmokeInstrumentation extends Instrumentation {
    public void onCreate(Bundle args) { super.onCreate(args); start(); }
    private static byte[] bytes(String s) { return s.getBytes(StandardCharsets.UTF_8); }
    private static String sha(byte[] data) throws Exception {
        StringBuilder s = new StringBuilder();
        for (byte b : MessageDigest.getInstance("SHA-256").digest(data)) s.append(String.format(java.util.Locale.ROOT, "%02x", b));
        return s.toString();
    }
    private void check(boolean value, String message) { if (!value) throw new AssertionError(message); }
    private static void write(File file, byte[] bytes) throws Exception { try (OutputStream out = new FileOutputStream(file)) { out.write(bytes); } }
    private byte[] read(Uri uri) throws Exception { try (InputStream in = getTargetContext().getContentResolver().openInputStream(uri)) { return in.readAllBytes(); } }
    private void write(Uri uri, byte[] bytes) throws Exception { try (OutputStream out = getTargetContext().getContentResolver().openOutputStream(uri, "wt")) { out.write(bytes); } }
    public void onStart() {
        Bundle result = new Bundle();
        File local = new File(getTargetContext().getFilesDir(), "bridge-test.md");
        SharedPreferences prefs = getTargetContext().getSharedPreferences("documents", Context.MODE_PRIVATE);
        try {
            HuiActivity activity = (HuiActivity) startActivitySync(new Intent().setClassName(getTargetContext(), "dev.hui.markdown.HuiActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
            java.lang.reflect.Field ready = HuiActivity.class.getDeclaredField("ready");
            ready.setAccessible(true);
            long deadline = android.os.SystemClock.uptimeMillis() + 30000;
            while (!ready.getBoolean(activity) && android.os.SystemClock.uptimeMillis() < deadline) android.os.SystemClock.sleep(100);
            check(ready.getBoolean(activity), "Slint native UI did not initialize");
            android.os.SystemClock.sleep(2000);
            Uri uri = Uri.parse("content://dev.hui.markdown.preview.test.fixture/document");
            byte[] original = bytes("# 中文 日本語 한국어\r\nemoji 👋🏽\r\n"), edited = bytes("# Édition\r\nРусский Español Deutsch\r\n");
            write(uri, original);
            File documents = new File(getTargetContext().getFilesDir(), "documents");
            java.util.Set<String> previous = new java.util.HashSet<>();
            if (documents.list() != null) previous.addAll(java.util.Arrays.asList(documents.list()));
            runOnMainSync(() -> activity.onNewIntent(new Intent(Intent.ACTION_VIEW).setDataAndType(uri, "text/markdown").addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)));
            File imported = null;
            deadline = android.os.SystemClock.uptimeMillis() + 15000;
            while (imported == null && android.os.SystemClock.uptimeMillis() < deadline) {
                File[] dirs = documents.listFiles();
                if (dirs != null) for (File dir : dirs) if (!previous.contains(dir.getName())) {
                    File candidate = new File(dir, "Document.md");
                    File config = new File(getTargetContext().getFilesDir(), "profile/preferences.json");
                    if (candidate.isFile() && config.isFile() && new String(java.nio.file.Files.readAllBytes(config.toPath()), StandardCharsets.UTF_8).contains(candidate.getCanonicalPath())) imported = candidate;
                }
                if (imported == null) android.os.SystemClock.sleep(100);
            }
            if (imported == null) {
                File config = new File(getTargetContext().getFilesDir(), "profile/preferences.json");
                throw new AssertionError("Content URI did not reach the Rust document controller: " + (config.isFile() ? new String(java.nio.file.Files.readAllBytes(config.toPath()), StandardCharsets.UTF_8) : "no preferences"));
            }
            check(java.util.Arrays.equals(java.nio.file.Files.readAllBytes(imported.toPath()), original), "URI import changed document bytes");
            // Use the real import mapping, not a manually seeded test mapping. Rust passes
            // the canonical path while Java's filesDir may use a different Android alias.
            local = imported;
            check(activity.documentName(local.getCanonicalPath()).equals("Document.md"), "Canonical path lost display name");
            write(local, edited);
            check(activity.syncDocument(local.getCanonicalPath()).isEmpty(), "Save through canonical import path failed");
            check(java.util.Arrays.equals(read(uri), edited), "Unicode or CRLF changed");
            check(activity.syncDocument(local.toString()).isEmpty(), "Repeated save failed");
            byte[] external = bytes("external change"), mine = bytes("unsaved local edits");
            write(uri, external); write(local, mine);
            check(activity.syncDocument(local.toString()).contains("External document changed"), "Conflict was not detected");
            check(java.util.Arrays.equals(read(uri), external), "Conflict overwrote external document");
            check(java.util.Arrays.equals(java.nio.file.Files.readAllBytes(local.toPath()), mine), "Conflict lost local edits");
            prefs.edit().putString(local.getCanonicalPath()+".uri", uri.buildUpon().path("/readonly").build().toString()).putString(local.getCanonicalPath()+".sha", sha(external)).commit();
            check(!activity.syncDocument(local.toString()).isEmpty(), "Read-only document silently saved");
            check(java.util.Arrays.equals(read(uri), external), "Read-only failure changed document");
            prefs.edit().putString(local.getCanonicalPath()+".uri", uri.buildUpon().path("/denied").build().toString()).commit();
            check(!activity.syncDocument(local.toString()).isEmpty(), "Revoked permission silently saved");
            result.putString("stream", "PASS: provider save, native Slint initialization, content URI opening and canonical-path write-back, Unicode/CRLF, repeated save, conflict preservation, read-only and revoked permission\n");
            finish(Activity.RESULT_OK, result);
        } catch (Throwable e) {
            result.putString("stream", "FAIL: " + android.util.Log.getStackTraceString(e)); finish(Activity.RESULT_CANCELED, result);
        } finally {
            try {
                String key = local.getCanonicalPath();
                prefs.edit().remove(key+".uri").remove(key+".sha").remove(key+".name").commit();
                local.delete();
            } catch (IOException ignored) { }
        }
    }
}
