package net.daboross.scrs;
public final class ScreepsConnection {
    private long mNativeObj;

    public ScreepsConnection()  {
        mNativeObj = init();
    }
    private static native long init() ;

    public void login(String a_0, String a_1)  {
          do_login(mNativeObj, a_0, a_1);
    }
    private static native void do_login(long me, String a_0, String a_1) ;

    public synchronized void delete() {
        if (mNativeObj != 0) {
            do_delete(mNativeObj);
            mNativeObj = 0;
       }
    }
    @Override
    protected void finalize() { delete(); }
    private static native void do_delete(long me);
}