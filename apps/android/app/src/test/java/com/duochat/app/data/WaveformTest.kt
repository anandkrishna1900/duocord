package com.duochat.app.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class WaveformTest {
    @Test fun peaksAreScaledToTheLoudestSlice() {
        val p = waveformPeaks(shortArrayOf(100, -100, 400, -200, 50, 0, 0, 0), 4)
        assertEquals(4, p.size)
        assertEquals(0.25f, p[0], 1e-6f)
        assertEquals(1f, p[1], 1e-6f)
        assertEquals(0.125f, p[2], 1e-6f)
        assertEquals(0f, p[3], 1e-6f)
    }

    @Test fun silenceAndEmptyGiveZeros() {
        assertTrue(waveformPeaks(ShortArray(10), 5).all { it == 0f })
        assertTrue(waveformPeaks(ShortArray(0), 5).all { it == 0f })
        assertEquals(0, waveformPeaks(ShortArray(3), 0).size)
    }

    @Test fun fewerSamplesThanBucketsStillWorks() {
        val p = waveformPeaks(shortArrayOf(10, 20), 6)
        assertEquals(6, p.size)
        assertEquals(1f, p.max(), 1e-6f)
    }

    @Test fun receiptsAndHops() {
        val r = parseReceipts("""[{"device_id":"abcd1234","device_name":"Laptop","state":"read","at":1000},{"device_id":"ef567890","device_name":null,"state":"waiting","at":0}]""")
        assertEquals(2, r.size)
        assertEquals("Laptop: read 12:03", receiptLine(r[0]) { "12:03" })
        assertEquals("Device ef56: waiting", receiptLine(r[1]) { "x" })
        assertTrue(parseReceipts("nope").isEmpty())
        assertEquals(mapOf("a" to 2, "b" to 0), parseMeshHops("""{"a":2,"b":0}"""))
        assertEquals("directly", hopsWords(null))
        assertEquals("through 1 nearby phone", hopsWords(1))
        assertEquals("through 3 nearby phones", hopsWords(3))
    }
}
