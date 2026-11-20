package com.duochat.app.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.duochat.app.data.AppSettings
import com.duochat.app.data.ChatMessage
import com.duochat.app.data.ChatState
import com.duochat.app.data.ChatSummary
import com.duochat.app.data.ContactTrust
import com.duochat.app.data.ContactView
import com.duochat.app.data.DeliveryState
import com.duochat.app.data.EnginePhase
import com.duochat.app.data.LinkStatus
import com.duochat.app.data.ReplyRef
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Renders the main screens (home, a chat in both themes, a group chat, an empty chat, Add friend) on the JVM with
 * Robolectric and the Compose test rule, so a screen that crashes while composing, measuring or laying out fails the build
 * (pixels are not drawn: that needs Robolectric's native graphics mode, which CI runners do not have).
 */
@RunWith(AndroidJUnit4::class)
@GraphicsMode(GraphicsMode.Mode.LEGACY)
@Config(sdk = [35], qualifiers = "w393dp-h851dp-xxhdpi", application = android.app.Application::class)
open class ScreensRenderTest {
    @get:Rule
    val compose = createComposeRule()

    /** System font scale (Settings > Display > Font size) for this run. */
    open val fontScale = 1.0f

    /** Shows [content] and lets a few frames run (without waiting for idle: some screens animate forever). */
    private fun snapshot(scale: Float = fontScale, content: @Composable () -> Unit) {
        compose.mainClock.autoAdvance = false
        compose.setContent {
            val density = LocalDensity.current
            CompositionLocalProvider(LocalDensity provides Density(density.density, scale)) { content() }
        }
        compose.mainClock.advanceTimeBy(400)
    }

    private val me = "me-uid"
    private val ann = "ann-uid"
    private val now = System.currentTimeMillis()

    private fun msg(id: String, from: String, text: String, minsAgo: Int, state: DeliveryState = DeliveryState.READ, reply: ReplyRef? = null, reactions: Map<String, String> = emptyMap()) =
        ChatMessage(
            messageId = id, clientMessageId = id, senderId = from, text = text,
            clientCreatedAt = now - minsAgo * 60_000L, deliveryState = state, conversationId = ann,
            replyTo = reply, reactions = reactions,
        )

    private val state = ChatState(
        phase = EnginePhase.READY,
        uid = me,
        contacts = listOf(
            ContactView(ann, "Ann Lee", ContactTrust.VERIFIED, null, true, LinkStatus.CONNECTED, null, "1"),
        ),
        chats = listOf(
            ChatSummary(ann, false, "Ann Lee", "See you at 7 then 👋", now - 2 * 60_000L, 3, LinkStatus.CONNECTED, "Connected"),
            ChatSummary("g1", true, "Weekend trip", "Sam: I booked the cabin", now - 26 * 3_600_000L, 0, LinkStatus.CONNECTED, "4 members"),
            ChatSummary("b", false, "Ben Carter", "Thanks!", now - 4 * 86_400_000L, 0, LinkStatus.OFFLINE, "Waiting"),
            ChatSummary("c", false, "Cat with a really quite long name that should be cut", null, 0, 0, LinkStatus.CONNECTING, "Connecting…"),
        ),
        messages = mapOf(
            ann to listOf(
                msg("1", ann, "Hey! Are we still on for dinner tonight?", 130),
                msg("2", ann, "I can book the place by the river", 129),
                msg("3", me, "Yes! The river place sounds great", 120),
                msg("4", me, "7 o'clock?", 119, reactions = mapOf(ann to "👍")),
                msg("5", ann, "Perfect, booking now. It is usually busy on Fridays so this might take a minute, but I will let you know as soon as it's confirmed.", 20),
                msg("6", me, "Great, see you there", 3, DeliveryState.DELIVERED, reply = ReplyRef("5", ann, "Perfect, booking now")),
                msg("7", ann, "See you at 7 then 👋", 2),
                msg("8", me, "On my way", 1, DeliveryState.FAILED),
            ),
        ),
    )

    private val group = com.duochat.app.data.GroupView(
        groupId = "g1", name = "Weekend trip",
        members = listOf(
            com.duochat.app.data.MemberView(me, "You", true, true, null, null, null),
            com.duochat.app.data.MemberView(ann, "Ann Lee", false, false, null, ContactTrust.VERIFIED, LinkStatus.CONNECTED, true),
            com.duochat.app.data.MemberView("sam", "Sam Ortiz", false, false, "Ann Lee", ContactTrust.INTRODUCED, LinkStatus.CONNECTED),
        ),
        iAmMember = true, iAmAdmin = true, exitNote = null, connectedCount = 2,
    )

    private val groupState = state.copy(
        groups = listOf(group),
        messages = mapOf(
            "g1" to listOf(
                msg("g1", ann, "Who is driving on Saturday?", 60).copy(conversationId = "g1"),
                msg("g2", "sam", "I can take four people", 58).copy(conversationId = "g1"),
                msg("g3", "sam", "Leaving at 9 sharp", 57).copy(conversationId = "g1"),
                msg("g4", me, "Count me in 🙌", 50).copy(conversationId = "g1"),
                msg("g5", ann, "Same here", 49).copy(conversationId = "g1"),
            ),
        ),
    )

    @Test
    fun groupChat() = snapshot {
        androidx.compose.runtime.CompositionLocalProvider(
            androidx.activity.compose.LocalActivityResultRegistryOwner provides registryOwner,
        ) {
            DuoChatTheme(AppSettings(theme = "dark")) {
                ChatScreen(
                    state = groupState, conversationId = "g1", muteUntil = 0L, archived = false, onBack = {},
                    onSend = { _, _, _ -> }, onSendFile = { _, _, _, _, _ -> }, loadFile = { null },
                    onRetry = ::noop1, onClearNotice = {}, onOpenDetails = {}, onVerify = {}, onMuteUntil = {}, onSetArchived = {},
                    onClearChat = {}, onConnectBluetooth = {}, onLeaveChannel = {}, onMuteSender = ::noop1,
                )
            }
        }
    }

    @Test
    fun addFriend() = snapshot {
        androidx.compose.runtime.CompositionLocalProvider(
            androidx.activity.compose.LocalActivityResultRegistryOwner provides registryOwner,
        ) {
            DuoChatTheme(AppSettings(theme = "dark")) {
                AddFriendScreen(
                    state = state.copy(invite = com.duochat.app.data.ActiveInvite("water because surface fat worth sudden", now + 900_000)),
                    ownCard = { "DUOID1:eyJ2IjoxfQ" }, onAdd = { _, _ -> }, onCreateInvite = {}, onRetryAnswer = {},
                    onClearAnswer = {}, onChecked = {}, onBack = {}, onOpenChat = {},
                )
            }
        }
    }

    @Test
    fun emptyChat() = snapshot {
        androidx.compose.runtime.CompositionLocalProvider(
            androidx.activity.compose.LocalActivityResultRegistryOwner provides registryOwner,
        ) {
            DuoChatTheme(AppSettings(theme = "dark")) {
                ChatScreen(
                    state = state.copy(messages = emptyMap()), conversationId = ann, muteUntil = 0L, archived = false, onBack = {},
                    onSend = { _, _, _ -> }, onSendFile = { _, _, _, _, _ -> }, loadFile = { null },
                    onRetry = ::noop1, onClearNotice = {}, onOpenDetails = {}, onVerify = {}, onMuteUntil = {}, onSetArchived = {},
                    onClearChat = {}, onConnectBluetooth = {}, onLeaveChannel = {}, onMuteSender = ::noop1,
                )
            }
        }
    }

    private fun noop1(@Suppress("UNUSED_PARAMETER") a: String) = Unit

    @Test
    fun home() = snapshot {
        DuoChatTheme(AppSettings(theme = "dark")) {
            HomeScreen(state = state, onOpenChat = {}, onAddFriend = {}, onNewGroup = {}, onChannels = {}, onOpenSettings = {}, onClearNotice = {})
        }
    }

    @Test
    fun homeLight() = snapshot {
        DuoChatTheme(AppSettings(theme = "light")) {
            HomeScreen(state = state, onOpenChat = {}, onAddFriend = {}, onNewGroup = {}, onChannels = {}, onOpenSettings = {}, onClearNotice = {})
        }
    }

    @Test
    fun homeEmpty() = snapshot {
        DuoChatTheme(AppSettings(theme = "light")) {
            HomeScreen(state = state.copy(chats = emptyList()), onOpenChat = {}, onAddFriend = {}, onNewGroup = {}, onChannels = {}, onOpenSettings = {}, onClearNotice = {})
        }
    }

    @Test
    fun onboarding() = snapshot {
        DuoChatTheme(AppSettings(theme = "light")) { OnboardingScreen(onCreate = { _, _, _ -> }) }
    }

    @Test
    fun onboardingDark() = snapshot {
        DuoChatTheme(AppSettings(theme = "dark")) { OnboardingScreen(onCreate = { _, _, _ -> }) }
    }

    @Test
    fun lock() = snapshot {
        DuoChatTheme(AppSettings(theme = "dark")) { LockScreen(onUnlocked = {}) }
    }

    @Test
    fun loading() = snapshot {
        DuoChatTheme(AppSettings(theme = "light")) { LoadingScreen("Opening your chats") }
    }

    @Test
    fun newGroup() = snapshot {
        DuoChatTheme(AppSettings(theme = "light")) {
            NewGroupScreen(state = groupState, onCreate = { _, _, _ -> }, onBack = {}, onCreated = {})
        }
    }

    @Test
    fun groupDetails() = snapshot {
        DuoChatTheme(AppSettings(theme = "dark")) {
            GroupDetailsScreen(
                state = groupState, groupId = "g1", onBack = {}, onAddMembers = { _, _ -> }, onRemove = { _, _ -> },
                onRename = { _, _ -> }, onLeave = {}, onVerify = {}, onMessage = {}, onLeft = {},
            )
        }
    }

    @Test
    fun chatDark() = snapshot { chat(AppSettings(theme = "dark")) }

    @Test
    fun chatLight() = snapshot { chat(AppSettings(theme = "light")) }

    /** Larger system font (Settings > Display > Font size): nothing may be cut off or overlap. */
    @Test
    fun chatLargeText() {
        snapshot(1.3f) { chat(AppSettings(theme = "dark")) }
    }

    @Test
    fun homeLargeText() {
        snapshot(1.3f) {
            DuoChatTheme(AppSettings(theme = "dark")) {
                HomeScreen(state = state, onOpenChat = {}, onAddFriend = {}, onNewGroup = {}, onChannels = {}, onOpenSettings = {}, onClearNotice = {})
            }
        }
    }

    private val registryOwner = object : androidx.activity.result.ActivityResultRegistryOwner {
        override val activityResultRegistry = object : androidx.activity.result.ActivityResultRegistry() {
            override fun <I, O> onLaunch(
                requestCode: Int,
                contract: androidx.activity.result.contract.ActivityResultContract<I, O>,
                input: I,
                options: androidx.core.app.ActivityOptionsCompat?,
            ) = Unit
        }
    }

    @Composable
    private fun chat(settings: AppSettings) {
        androidx.compose.runtime.CompositionLocalProvider(
            androidx.activity.compose.LocalActivityResultRegistryOwner provides registryOwner,
        ) { chatInner(settings) }
    }

    @Composable
    private fun chatInner(settings: AppSettings) {
        DuoChatTheme(settings) {
            ChatScreen(
                state = state, conversationId = ann, muteUntil = 0L, archived = false, onBack = {},
                onSend = { _, _, _ -> }, onSendFile = { _, _, _, _, _ -> }, loadFile = { null },
                onRetry = ::noop1, onClearNotice = {}, onOpenDetails = {}, onVerify = {}, onMuteUntil = {}, onSetArchived = {},
                onClearChat = {}, onConnectBluetooth = {}, onLeaveChannel = {}, onMuteSender = ::noop1,
            )
        }
    }
}
