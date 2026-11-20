package com.duochat.app.notifications

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.RemoteInput
import androidx.core.content.ContextCompat
import com.duochat.app.MainActivity
import com.duochat.app.R
import com.duochat.app.data.AppSettings

object DuoNotifications {
    const val CHANNEL_ID = "messages"
    private const val CHANNEL_NAME = "Messages"
    private const val CHANNEL_DESCRIPTION = "New private messages from your PrivCord contacts."

    fun ensureChannel(context: Context) {
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        val channel = NotificationChannel(
            CHANNEL_ID,
            CHANNEL_NAME,
            NotificationManager.IMPORTANCE_HIGH,
        ).apply {
            description = CHANNEL_DESCRIPTION
        }
        manager.createNotificationChannel(channel)
    }

    fun showNotice(context: Context, notice: com.duochat.app.engine.IncomingNotice, settings: AppSettings) {
        // With the app lock on, a message must not be readable from the lock screen either.
        val body = if (notice.isMessage && (!settings.notificationPreviews || settings.appLock)) "Open PrivCord to read it." else preview(notice.body)
        // Quick reply from the notification (only for chats with a friend or a group, and never with the app lock on: a
        // reply must not be sendable from a locked phone).
        val replyTo = notice.conversationId.takeIf { notice.isMessage && !settings.appLock && !it.startsWith("chan:") }
        show(
            context = context,
            notificationId = notice.id.hashCode(),
            title = notice.title,
            body = body,
            settings = settings,
            replyTo = replyTo,
            silent = notice.silent,
        )
    }

    private fun show(
        context: Context,
        notificationId: Int,
        title: String,
        body: String,
        settings: AppSettings,
        replyTo: String? = null,
        silent: Boolean = false,
    ) {
        if (!settings.notificationsEnabled) return
        if (
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            ContextCompat.checkSelfPermission(
                context,
                Manifest.permission.POST_NOTIFICATIONS,
            ) != PackageManager.PERMISSION_GRANTED
        ) {
            return
        }
        ensureChannel(context)
        val intent = Intent(context, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP
        }
        val pendingIntent = PendingIntent.getActivity(
            context,
            notificationId,
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val builder = NotificationCompat.Builder(context, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(title)
            .setContentText(body)
            .setStyle(NotificationCompat.BigTextStyle().bigText(body))
            .setAutoCancel(true)
            .setContentIntent(pendingIntent)
            .setCategory(NotificationCompat.CATEGORY_MESSAGE)
            .setVisibility(NotificationCompat.VISIBILITY_PRIVATE)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setSilent(silent)
        if (replyTo != null) {
            val reply = Intent(context, QuickReply::class.java).apply {
                action = QuickReply.ACTION
                setPackage(context.packageName)
                putExtra(QuickReply.EXTRA_CONVERSATION, replyTo)
                putExtra(QuickReply.EXTRA_NOTIFICATION, notificationId)
            }
            // Mutable: the system fills in the typed text.
            val flags = PendingIntent.FLAG_UPDATE_CURRENT or
                (if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE else 0)
            val action = NotificationCompat.Action.Builder(
                R.drawable.ic_notification,
                "Reply",
                PendingIntent.getBroadcast(context, notificationId, reply, flags),
            )
                .addRemoteInput(RemoteInput.Builder(QuickReply.KEY_TEXT).setLabel("Reply").build())
                .setSemanticAction(NotificationCompat.Action.SEMANTIC_ACTION_REPLY)
                .setShowsUserInterface(false)
                .build()
            builder.addAction(action)
        }
        val notification = builder.build()
        try {
            NotificationManagerCompat.from(context).notify(notificationId, notification)
        } catch (_: SecurityException) {
            // Notifications are best-effort.
        }
    }

    private fun preview(text: String): String {
        val single = text.replace(Regex("\\s+"), " ").trim()
        return if (single.length > 140) "${single.take(137)}…" else single
    }
}
