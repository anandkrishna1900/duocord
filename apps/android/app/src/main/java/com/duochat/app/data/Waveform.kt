package com.duochat.app.data

import kotlin.math.abs

/**
 * The loudness of each of [buckets] equal slices of [samples], scaled so the loudest slice is 1. An empty or silent
 * recording gives all zeros.
 */
fun waveformPeaks(samples: ShortArray, buckets: Int): FloatArray {
    if (buckets <= 0) return FloatArray(0)
    val out = FloatArray(buckets)
    if (samples.isEmpty()) return out
    for (i in samples.indices) {
        val b = (i.toLong() * buckets / samples.size).toInt()
        val v = abs(samples[i].toInt()).toFloat()
        if (v > out[b]) out[b] = v
    }
    val top = out.max()
    if (top > 0f) for (i in out.indices) out[i] /= top
    return out
}
