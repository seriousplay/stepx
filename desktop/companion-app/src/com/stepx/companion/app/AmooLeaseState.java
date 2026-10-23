package com.stepx.companion.app;

import java.util.HashMap;
import java.util.Map;

/** Monotonic, per-connection leases. No screen, account, or model state is invented here. */
final class AmooLeaseState {
    static final long TTL_MS=90_000, OFF_SETTLE_MS=15_000, WAKE_INTERVAL_MS=60_000;
    private final Map<String,Long> leases=new HashMap<>();
    private long offSince=-1, lastRequest=-1, epochWake=-1;
    void acquire(String id,long now){expire(now);if(!leases.containsKey(id)&&leases.size()>=64)throw new IllegalStateException("后台连接数已达上限");leases.put(id,now+TTL_MS);}
    void renew(String id,long now){expire(now);if(!leases.containsKey(id))throw new IllegalStateException("任务租约已失效，请重新连接");leases.put(id,now+TTL_MS);}
    void release(String id){leases.remove(id);}
    void clear(){leases.clear();}
    void expire(long now){leases.entrySet().removeIf(e->e.getValue()<=now);}
    boolean has(String id,long now){expire(now);return leases.containsKey(id);}
    boolean active(long now){expire(now);return !leases.isEmpty();}
    long expires(long now){expire(now);long max=0;for(long expiry:leases.values())max=Math.max(max,expiry);return max;}
    int count(long now){expire(now);return leases.size();}
    void screen(boolean interactive,long now){if(interactive){offSince=-1;epochWake=-1;}else if(offSince<0){offSince=now;epochWake=-1;}}
    long wakeDelay(boolean interactive,long now){screen(interactive,now);if(interactive||!active(now))return -1;long settled=offSince+OFF_SETTLE_MS;long interval=lastRequest<0?0:lastRequest+WAKE_INTERVAL_MS;return Math.max(0,Math.max(settled,interval)-now);}
    void woke(long now){lastRequest=now;epochWake=now;}
    void restoreWake(long previous,long now){if(previous>=0&&previous<=now)lastRequest=previous;}
    long lastWake(){return epochWake;}
    long lastRequest(){return lastRequest;}
    long nativeWindowRemaining(long now){return epochWake<0?0:Math.max(0,epochWake+180_000-now);}
    long idleGraceRemaining(boolean enabled,boolean interactive,long now){return enabled&&!interactive&&!active(now)?nativeWindowRemaining(now):0;}
}
