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
    private java.lang.reflect.Method selectionProbe;
    private org.json.JSONObject selection(HuiActivity activity, String action, double x, double y, int offset) throws Exception {
        org.json.JSONObject request = new org.json.JSONObject();
        request.put("action", action); request.put("x",x); request.put("y",y); request.put("byte",offset);
        org.json.JSONObject value = new org.json.JSONObject((String) selectionProbe.invoke(activity,request.toString()));
        check(!value.has("error"),"Selection probe: " + value);
        return value;
    }
    private org.json.JSONObject selection(HuiActivity activity, String action) throws Exception { return selection(activity,action,0,0,0); }
    private void longPress(HuiActivity activity, org.json.JSONObject point) throws Exception {
        double x=point.getDouble("x"), y=point.getDouble("y");
        selection(activity,"move",x,y,0);
        selection(activity,"down",x,y,0); android.os.SystemClock.sleep(650);
        selection(activity,"up",x,y,0); android.os.SystemClock.sleep(400);
    }
    private void snapshot(HuiActivity activity, Bundle result, String name) throws Exception {
        java.lang.reflect.Method probe = HuiActivity.class.getDeclaredMethod("nativeDisplayProbe"); probe.setAccessible(true);
        org.json.JSONObject display = new org.json.JSONObject((String)probe.invoke(activity));
        check(display.getBoolean("snapshot"),"Selection snapshot failed");
        File artifact = new File(getTargetContext().getExternalFilesDir(null),name+".png");
        java.nio.file.Files.copy(new File(getTargetContext().getFilesDir(),"display-probe.png").toPath(),artifact.toPath(),java.nio.file.StandardCopyOption.REPLACE_EXISTING);
        String image=android.util.Base64.encodeToString(java.nio.file.Files.readAllBytes(artifact.toPath()),android.util.Base64.NO_WRAP);
        for(int start=0,index=0;start<image.length();start+=100000,index++) {
            Bundle part=new Bundle(); part.putString("image-"+name+"-"+String.format(java.util.Locale.ROOT,"%04d",index),image.substring(start,Math.min(start+100000,image.length())));
            sendStatus(0,part);
        }
    }
    private void selectionTests(HuiActivity activity, Bundle result) throws Exception {
        selectionProbe = HuiActivity.class.getDeclaredMethod("nativeSelectionProbe",String.class); selectionProbe.setAccessible(true);
        java.lang.reflect.Method event = HuiActivity.class.getDeclaredMethod("nativeEvent",String.class,String.class); event.setAccessible(true);
        File fixture = new File(getTargetContext().getFilesDir(),"touch-selection.md");
        String content = "Alpha 中文 👩‍💻 café\n\nBeta second paragraph\n\n";
        StringBuilder large = new StringBuilder(content);
        for (int i=0;i<240;i++) large.append("日本語 한국어 Русский Español paragraph ").append(i).append("\n\n");
        large.append("LAST_DOCUMENT_MARKER\n");
        write(fixture,bytes(large.toString()));
        event.invoke(activity,"open",fixture.getAbsolutePath()); android.os.SystemClock.sleep(2500);
        longPress(activity,selection(activity,"state"));
        check(selection(activity,"state").getBoolean("active"),"Reading long press did not select a word");
        selection(activity,"copy"); android.os.SystemClock.sleep(300);
        check(activity.pasteText().equals("Alpha"),"Reading word clipboard: "+activity.pasteText());
        org.json.JSONObject handles=selection(activity,"state");
        snapshot(activity,result,"snapshot-reading-word");
        org.json.JSONObject debugBefore = new org.json.JSONObject(handles.toString()); debugBefore.remove("source");
        result.putString("reading-before",debugBefore.toString());
        double bx=handles.getDouble("bx"), by=handles.getDouble("by");
        selection(activity,"move",bx,by+14,0);
        org.json.JSONObject debugDown=selection(activity,"down",bx,by+14,0); debugDown.remove("source"); result.putString("handle-down",debugDown.toString()); android.os.SystemClock.sleep(100);
        selection(activity,"move",bx+100,by+55,0); android.os.SystemClock.sleep(100);
        selection(activity,"up",bx+100,by+55,0); android.os.SystemClock.sleep(400);
        selection(activity,"copy"); android.os.SystemClock.sleep(300);
        snapshot(activity,result,"snapshot-reading-drag");
        org.json.JSONObject debugAfter = selection(activity,"state"); debugAfter.remove("source"); result.putString("reading-after",debugAfter.toString());
        boolean readingDragWorks=activity.pasteText().contains("Beta");
        snapshot(activity,result,"snapshot-reading-selection");
        selection(activity,"all"); android.os.SystemClock.sleep(400);
        selection(activity,"copy"); android.os.SystemClock.sleep(300);
        check(activity.pasteText().startsWith("Alpha") && activity.pasteText().contains("LAST_DOCUMENT_MARKER"),"Reading all missed offscreen text");
        selection(activity,"source"); android.os.SystemClock.sleep(700);
        org.json.JSONObject tap=selection(activity,"state");
        selection(activity,"move",tap.getDouble("x"),tap.getDouble("y"),0);
        selection(activity,"down",tap.getDouble("x"),tap.getDouble("y"),0);
        selection(activity,"up",tap.getDouble("x"),tap.getDouble("y"),0);
        android.os.SystemClock.sleep(400);
        longPress(activity,selection(activity,"state"));
        handles=selection(activity,"state");
        check(handles.getBoolean("active"),"Source long press did not select a word");
        check(handles.getInt("anchor")==0 && handles.getInt("cursor")==5,"Source word offsets: "+handles);
        selection(activity,"copy"); android.os.SystemClock.sleep(300);
        check(activity.pasteText().equals("Alpha"),"Source word clipboard: "+activity.pasteText());
        // Drag the actual handle through the Slint pointer event path to a second-line caret.
        org.json.JSONObject target=selection(activity,"state",0,0,bytes(content).length-2);
        bx=handles.getDouble("bx"); by=handles.getDouble("by");
        selection(activity,"move",bx,by+14,0);
        selection(activity,"down",bx,by+14,0); android.os.SystemClock.sleep(100);
        double tx=target.getDouble("x"), ty=target.getDouble("y")+14;
        selection(activity,"move",tx,ty,0); selection(activity,"up",tx,ty,0); android.os.SystemClock.sleep(350);
        selection(activity,"copy"); android.os.SystemClock.sleep(300);
        check(activity.pasteText().contains("中文 👩‍💻 café") && activity.pasteText().contains("Beta"),"Source handle lost Unicode/cross-line selection: "+activity.pasteText());
        snapshot(activity,result,"snapshot-source-selection");
        selection(activity,"all"); selection(activity,"copy"); android.os.SystemClock.sleep(300);
        check(activity.pasteText().equals(large.toString()),"Source select-all only copied viewport");
        Uri exportUri = Uri.parse("content://dev.hui.markdown.preview.test.fixture/document");
        IntentFilter exportFilter = new IntentFilter(Intent.ACTION_CREATE_DOCUMENT);
        exportFilter.addDataType("text/plain");
        exportFilter.addCategory(Intent.CATEGORY_OPENABLE);
        ActivityMonitor exportMonitor = addMonitor(exportFilter,
            new ActivityResult(Activity.RESULT_OK,new Intent().setData(exportUri)),true);
        selection(activity,"export-txt");
        String exported=""; long exportDeadline=android.os.SystemClock.uptimeMillis()+10000;
        while (android.os.SystemClock.uptimeMillis()<exportDeadline) {
            try { exported=new String(read(exportUri),StandardCharsets.UTF_8); } catch (Exception ignored) { }
            if (exported.contains("LAST_DOCUMENT_MARKER")) break;
            android.os.SystemClock.sleep(100);
        }
        result.putInt("export-picker-interceptions",exportMonitor.getHits());
        java.lang.reflect.Field pending=HuiActivity.class.getDeclaredField("pendingCopy"); pending.setAccessible(true);
        String exportPath=(String)pending.get(activity);
        if (exportPath != null) { result.putString("export-pending",exportPath); result.putLong("export-bytes",new File(exportPath).length()); }
        removeMonitor(exportMonitor);
        check(exported.contains("LAST_DOCUMENT_MARKER"),"TXT export missed offscreen content");
        check(exported.contains("\u4e2d\u6587 \ud83d\udc69\u200d\ud83d\udcbb cafe\u0301"),"TXT lost Unicode");
        selection(activity,"all"); selection(activity,"left"); android.os.SystemClock.sleep(100);
        check(!selection(activity,"state").getBoolean("all"),"Caret navigation retained full-document selection");
        selection(activity,"all"); selection(activity,"replace-all"); android.os.SystemClock.sleep(200);
        selection(activity,"all"); selection(activity,"copy"); android.os.SystemClock.sleep(200);
        check(activity.pasteText().equals("Replacement"),"Typing after select-all did not replace entire document");
        selection(activity,"undo"); android.os.SystemClock.sleep(200);
        selection(activity,"all"); selection(activity,"copy"); android.os.SystemClock.sleep(200);
        check(activity.pasteText().equals(large.toString()),"Undo lost content outside editor viewport");
        check(readingDragWorks,"Reading handle did not cross paragraph");
        result.putString("selection","PASS: long-press word, actual handle drag across paragraphs/lines, Unicode clipboard, reading/source select-all beyond viewport, whole-document replace/undo, UTF-8 TXT export");
        selection(activity,"reading");
    }

    public void onStart() {
        Bundle result = new Bundle();
        File local = new File(getTargetContext().getFilesDir(), "bridge-test.md");
        SharedPreferences prefs = getTargetContext().getSharedPreferences("documents", Context.MODE_PRIVATE);
        try {
            File profile = new File(getTargetContext().getFilesDir(), "profile");
            profile.mkdirs();
            File preferencesFile = new File(profile, "preferences.json");
            org.json.JSONObject preferences = new org.json.JSONObject();
            preferences.put("language", "zh-Hans");
            preferences.put("theme", 0);
            preferences.put("font_size", 15);
            preferences.put("recent", new org.json.JSONArray());
            preferences.put("positions", new org.json.JSONObject());
            write(preferencesFile, bytes(preferences.toString()));
            HuiActivity activity = (HuiActivity) startActivitySync(new Intent().setClassName(getTargetContext(), "dev.hui.markdown.HuiActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
            java.lang.reflect.Field ready = HuiActivity.class.getDeclaredField("ready");
            ready.setAccessible(true);
            long deadline = android.os.SystemClock.uptimeMillis() + 30000;
            while (!ready.getBoolean(activity) && android.os.SystemClock.uptimeMillis() < deadline) android.os.SystemClock.sleep(100);
            check(ready.getBoolean(activity), "Slint native UI did not initialize");
            android.os.SystemClock.sleep(2000);
            java.lang.reflect.Method probe = HuiActivity.class.getDeclaredMethod("nativeDisplayProbe");
            probe.setAccessible(true);
            org.json.JSONObject display = new org.json.JSONObject((String) probe.invoke(activity));
            check(display.getString("font").equals("Noto Sans CJK SC"), "Bundled UI font not selected");
            check(display.getBoolean("snapshot"), "Native snapshot failed: " + display);
            check(display.getDouble("top") > 0, "System status bar inset not reported: " + display);
            check(display.getDouble("height") > display.getDouble("top") + display.getDouble("bottom") + 100, "Unsafe content height: " + display);
            result.putString("display", display.toString());
            result.putString("snapshot", android.util.Base64.encodeToString(
                java.nio.file.Files.readAllBytes(new File(getTargetContext().getFilesDir(), "display-probe.png").toPath()), android.util.Base64.NO_WRAP));
            selectionTests(activity,result);
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
