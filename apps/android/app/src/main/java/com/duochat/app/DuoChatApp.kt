package com.duochat.app

import android.app.Application
import com.duochat.app.notifications.DuoNotifications
import com.duochat.app.p2p.PeerConnectionFactoryHolder

class DuoChatApp : Application() {
    override fun onCreate() {
        super.onCreate()
        CrashLog.captureLastExit(this)
        CrashLog.install(this)
        com.duochat.app.engine.EngineHost.app = this
        DuoNotifications.ensureChannel(this)
        // A safety net for phones that stop the background service: a short check about every 15 minutes.
        com.duochat.app.notifications.BackgroundCheck.schedule(this)
        PeerConnectionFactoryHolder.initialize(this)
        android.util.Log.i("DuoChat", "Rust core: ${RustCore.status}")
    }
}
