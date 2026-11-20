package com.duochat.app.ui

import org.robolectric.annotation.Config

/**
 * Every screen of ScreensRenderTest again on a small phone (360 dp wide) with large text (1.6x), where clipped labels,
 * wrapping buttons and overlapping rows show up first.
 */
@Config(sdk = [35], qualifiers = "w360dp-h640dp-xhdpi")
class StressRenderTest : ScreensRenderTest() {
    override val fontScale = 1.6f
}
