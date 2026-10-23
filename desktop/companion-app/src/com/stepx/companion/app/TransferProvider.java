package com.stepx.companion.app;

import android.content.*;
import android.database.*;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;
import java.io.*;
import java.util.List;

/** Not exported. A user-selected file is shared read-only via a temporary URI grant. */
public final class TransferProvider extends ContentProvider {
    private TransferStore store;
    @Override public boolean onCreate(){store=new TransferStore(getContext());return true;}
    private TransferStore.Entry entry(Uri uri)throws Exception {List<String> p=uri.getPathSegments();if(!TransferStore.AUTHORITY.equals(uri.getAuthority())||p.size()!=2)throw new FileNotFoundException("无效文件");return store.get(p.get(0),p.get(1));}
    @Override public String getType(Uri uri){try{return TransferStore.mime(entry(uri).name);}catch(Exception e){return "application/octet-stream";}}
    @Override public Cursor query(Uri uri,String[] projection,String selection,String[] args,String order){try{TransferStore.Entry e=entry(uri);String[] columns=projection==null?new String[]{OpenableColumns.DISPLAY_NAME,OpenableColumns.SIZE}:projection;MatrixCursor c=new MatrixCursor(columns,1);Object[] values=new Object[columns.length];for(int i=0;i<columns.length;i++){if(OpenableColumns.DISPLAY_NAME.equals(columns[i]))values[i]=e.name;else if(OpenableColumns.SIZE.equals(columns[i]))values[i]=e.size;}c.addRow(values);return c;}catch(Exception e){return null;}}
    @Override public ParcelFileDescriptor openFile(Uri uri,String mode)throws FileNotFoundException {if(!"r".equals(mode))throw new FileNotFoundException("仅允许读取");try{TransferStore.Entry e=entry(uri);return ParcelFileDescriptor.open(store.verifiedFile(e.direction,e.id),ParcelFileDescriptor.MODE_READ_ONLY);}catch(Exception e){throw new FileNotFoundException(e.getMessage());}}
    @Override public Uri insert(Uri uri,ContentValues values){throw new UnsupportedOperationException();}
    @Override public int update(Uri uri,ContentValues values,String selection,String[] args){throw new UnsupportedOperationException();}
    @Override public int delete(Uri uri,String selection,String[] args){throw new UnsupportedOperationException();}
}
