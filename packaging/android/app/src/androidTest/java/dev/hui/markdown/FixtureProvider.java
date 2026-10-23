package dev.hui.markdown;

import android.content.*;
import android.database.Cursor;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import java.io.*;

/** Test APK only. Never packaged in HUI. Exposes one disposable fixture, no user files. */
public final class FixtureProvider extends ContentProvider {
    public boolean onCreate() { return true; }
    public String getType(Uri uri) { return "text/markdown"; }
    public Cursor query(Uri u, String[] p, String s, String[] a, String o) { return null; }
    public Uri insert(Uri u, ContentValues v) { throw new UnsupportedOperationException(); }
    public int update(Uri u, ContentValues v, String s, String[] a) { throw new UnsupportedOperationException(); }
    public int delete(Uri u, String s, String[] a) { throw new UnsupportedOperationException(); }
    public ParcelFileDescriptor openFile(Uri uri, String mode) throws FileNotFoundException {
        if (uri.getPath().equals("/denied")) throw new FileNotFoundException("Permission revoked (test)");
        if (uri.getPath().equals("/readonly") && !mode.equals("r")) throw new FileNotFoundException("Read only (test)");
        return ParcelFileDescriptor.open(new File(getContext().getFilesDir(), "fixture.md"), ParcelFileDescriptor.parseMode(mode));
    }
}
