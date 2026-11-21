package com.duochat.app.ui

import android.media.MediaCodec
import android.media.MediaExtractor
import android.media.MediaFormat
import com.duochat.app.data.waveformPeaks
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.nio.ByteOrder

/** Decodes a voice note's file to PCM and reduces it to bar heights. Returns null when the phone cannot decode it. */
object WaveformDecoder {
    // About 3 minutes at 44.1 kHz mono; a longer note is cut here and the bars cover only that part.
    private const val MAX_SAMPLES = 8_000_000

    suspend fun peaks(file: File, buckets: Int): FloatArray? = withContext(Dispatchers.Default) {
        runCatching { decode({ it.setDataSource(file.path) }, buckets) }.getOrNull()
    }

    /** The same from a voice note held in memory (nothing is written to storage). */
    suspend fun peaks(bytes: ByteArray, buckets: Int): FloatArray? = withContext(Dispatchers.Default) {
        runCatching { decode({ it.setDataSource(BytesMediaDataSource(bytes)) }, buckets) }.getOrNull()
    }

    private fun decode(open: (MediaExtractor) -> Unit, buckets: Int): FloatArray? {
        val extractor = MediaExtractor()
        var codec: MediaCodec? = null
        try {
            open(extractor)
            val track = (0 until extractor.trackCount).firstOrNull {
                extractor.getTrackFormat(it).getString(MediaFormat.KEY_MIME)?.startsWith("audio/") == true
            } ?: return null
            extractor.selectTrack(track)
            val format = extractor.getTrackFormat(track)
            var channels = format.getInteger(MediaFormat.KEY_CHANNEL_COUNT).coerceAtLeast(1)
            val c = MediaCodec.createDecoderByType(format.getString(MediaFormat.KEY_MIME)!!)
            codec = c
            c.configure(format, null, null, 0)
            c.start()
            val info = MediaCodec.BufferInfo()
            var mono = ShortArray(1 shl 16)
            var count = 0
            var inputDone = false
            var outputDone = false
            while (!outputDone && count < MAX_SAMPLES) {
                if (!inputDone) {
                    val i = c.dequeueInputBuffer(10_000)
                    if (i >= 0) {
                        val buf = c.getInputBuffer(i)!!
                        val n = extractor.readSampleData(buf, 0)
                        if (n < 0) {
                            c.queueInputBuffer(i, 0, 0, 0, MediaCodec.BUFFER_FLAG_END_OF_STREAM)
                            inputDone = true
                        } else {
                            c.queueInputBuffer(i, 0, n, extractor.sampleTime, 0)
                            extractor.advance()
                        }
                    }
                }
                val o = c.dequeueOutputBuffer(info, 10_000)
                if (o >= 0) {
                    val out = c.getOutputBuffer(o)!!.order(ByteOrder.LITTLE_ENDIAN).asShortBuffer()
                    out.limit(info.size / 2)
                    while (out.remaining() >= channels) {
                        var sum = 0
                        for (ch in 0 until channels) sum += out.get().toInt()
                        if (count == mono.size) mono = mono.copyOf(mono.size * 2)
                        mono[count++] = (sum / channels).toShort()
                    }
                    c.releaseOutputBuffer(o, false)
                    if (info.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0) outputDone = true
                } else if (o == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED) {
                    channels = c.outputFormat.getInteger(MediaFormat.KEY_CHANNEL_COUNT).coerceAtLeast(1)
                }
            }
            if (count == 0) return null
            return waveformPeaks(mono.copyOf(count), buckets)
        } finally {
            runCatching { codec?.stop() }
            runCatching { codec?.release() }
            extractor.release()
        }
    }
}
