package com.duochat.app.notifications

import android.app.Application
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.RemoteInput
import com.duochat.app.engine.EngineHost
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

/**
 * Reply to a message from its notification (build step 10). The text goes through the running engine like any other
 * message; when the app is not running, the engine is started for it (the same one the background service uses).
 * The notification is dismissed once the reply is written, and kept (with a note) when it could not be.
 */
class QuickReply : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != ACTION) return
        val conversation = intent.getStringExtra(EXTRA_CONVERSATION) ?: return
        val notificationId = intent.getIntExtra(EXTRA_NOTIFICATION, 0)
        val text = RemoteInput.getResultsFromIntent(intent)?.getCharSequence(KEY_TEXT)?.toString()?.trim()
        if (text.isNullOrEmpty()) return
        val app = context.applicationContext as? Application ?: return
        val pending = goAsync()
        CoroutineScope(SupervisorJob() + Dispatchers.Default).launch {
            try {
                val engine = EngineHost.ensureStarted(app)
                EngineHost.startJob?.join()
                val result = engine.send(conversation, text)
                if (result.isSuccess) {
                    NotificationManagerCompat.from(app).cancel(notificationId)
                }
            } catch (_: Exception) {
                // Best effort: the notification stays, so the person can open the app and write it there.
            } finally {
                pending.finish()
            }
        }
    }

    companion object {
        const val ACTION = "com.duochat.app.QUICK_REPLY"
        const val EXTRA_CONVERSATION = "conversation"
        const val EXTRA_NOTIFICATION = "notification"
        const val KEY_TEXT = "reply_text"
    }
}
