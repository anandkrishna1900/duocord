package com.duochat.app.ui

import android.media.MediaDataSource
import androidx.media3.common.MediaItem
import androidx.media3.datasource.ByteArrayDataSource
import androidx.media3.datasource.DataSource
import androidx.media3.exoplayer.source.MediaSource
import androidx.media3.exoplayer.source.ProgressiveMediaSource

/**
 * Media held in memory. A received video or voice note is read from the encrypted database into memory and played from
 * there, so nothing decrypted is ever written to the phone's storage (the players below read straight from this).
 */
class BytesMediaDataSource(private val bytes: ByteArray) : MediaDataSource() {
    override fun readAt(position: Long, buffer: ByteArray, offset: Int, size: Int): Int {
        if (position >= bytes.size) return -1
        val n = minOf(size.toLong(), bytes.size - position).toInt()
        System.arraycopy(bytes, position.toInt(), buffer, offset, n)
        return n
    }

    override fun getSize(): Long = bytes.size.toLong()

    override fun close() {}
}

/** An ExoPlayer source that plays [bytes] without a file or a network address. */
fun memoryMediaSource(bytes: ByteArray): MediaSource =
    ProgressiveMediaSource.Factory(DataSource.Factory { ByteArrayDataSource(bytes) })
        .createMediaSource(MediaItem.fromUri(android.net.Uri.EMPTY))
