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
        // Decrypted previews and recordings from older versions (or left by a crash) do not stay in the cache folder.
        runCatching {
            cacheDir.listFiles()
                ?.filter { f -> listOf("voice-", "preview-", "audio-").any { f.name.startsWith(it) } }
                ?.forEach { it.delete() }
        }
        // A safety net for phones that stop the background service: a short check about every 15 minutes.
        com.duochat.app.notifications.BackgroundCheck.schedule(this)
        PeerConnectionFactoryHolder.initialize(this)
        android.util.Log.i("DuoChat", "Rust core: ${RustCore.status}")
    }
}
