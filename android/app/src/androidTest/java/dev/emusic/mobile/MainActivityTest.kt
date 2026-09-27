package dev.emusic.mobile

import android.content.Intent
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Smoke test that launches the real app and drives the server-management flow
 * through the accessibility tree, the same way the `android.ps1` driver does.
 *
 * `testTagsAsResourceId` is enabled on the root, so Compose `testTag`s appear
 * as resource ids and can be located with [By.res].
 */
@RunWith(AndroidJUnit4::class)
class MainActivityTest {
    private lateinit var device: UiDevice

    @Before
    fun launchApp() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        device = UiDevice.getInstance(instrumentation)
        val context = instrumentation.targetContext
        val intent = context.packageManager.getLaunchIntentForPackage(context.packageName)!!
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK)
        context.startActivity(intent)
        // Wait for the top-bar action itself, not just the title: the two render
        // together and waiting on the title alone has raced on the emulator.
        assertTrue(
            "the main screen should be ready",
            device.wait(Until.hasObject(By.res("add_action")), 10_000),
        )
    }

    @Test
    fun opensAndFillsTheAddServerForm() {
        device.findObject(By.res("add_action")).click()
        assertTrue(
            "the server URL field should appear",
            device.wait(Until.hasObject(By.res("server_url")), 5_000),
        )

        device.findObject(By.res("pairing_code")).text = "123456"
        assertTrue(
            "the pairing code field should accept input",
            device.wait(Until.hasObject(By.text("123456")), 5_000),
        )
    }
}
