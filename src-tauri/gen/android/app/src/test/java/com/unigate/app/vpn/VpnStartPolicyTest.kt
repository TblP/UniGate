package com.unigate.app.vpn

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class VpnStartPolicyTest {
    @Test
    fun restoresStickyServiceWhenReconnectWasRequested() {
        assertTrue(VpnStartPolicy.shouldRestore(null, reconnectRequested = true, alwaysOn = false))
    }

    @Test
    fun restoresAlwaysOnServiceWithoutExplicitIntent() {
        assertTrue(VpnStartPolicy.shouldRestore(null, reconnectRequested = false, alwaysOn = true))
    }

    @Test
    fun doesNotRestoreAfterManualDisconnect() {
        assertFalse(VpnStartPolicy.shouldRestore(null, reconnectRequested = false, alwaysOn = false))
    }

    @Test
    fun explicitStartDoesNotUseRestorePath() {
        assertFalse(VpnStartPolicy.shouldRestore("{}", reconnectRequested = true, alwaysOn = true))
    }
}
