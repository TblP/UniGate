package com.unigate.app.vpn

import org.junit.Assert.assertTrue
import org.junit.Test

class VpnRoutingDiagnosticsTest {
    @Test
    fun reportsWhenAndroidAppliesNoPackagesFromAnAllowList() {
        val report = VpnRoutingDiagnostics(
            source = "tile",
            requestedMode = "only",
            requestedPackages = listOf("org.example.browser"),
        ).withResult(emptyList(), emptyList(), listOf("org.example.browser"), true)
            .render(36, "16", "on")

        assertTrue(report.contains("Source: tile"))
        assertTrue(report.contains("Requested mode: only"))
        assertTrue(report.contains("Applied mode: all"))
        assertTrue(report.contains("Missing packages: org.example.browser"))
    }

    @Test
    fun reportsOwnerChecksWithoutExposingOtherPackageNames() {
        val checks = VpnOwnerChecks(listOf("org.example.chat", "org.example.radio"))
        checks.record(listOf("org.example.chat"))
        checks.record(listOf("org.example.shared", "org.example.radio"))
        checks.record(listOf("org.example.excluded"))
        checks.record(emptyList())

        val report = VpnRoutingDiagnostics("app", "only", listOf("org.example.chat", "org.example.radio"), checks)
            .render(36, "16", "on")
        assertTrue(report.contains("Owner checks: selected=2, other=1, unknown=1"))
        assertTrue(!report.contains("org.example.excluded"))
    }

    @Test
    fun showsAppliedExclusionsAndAStoppedVpn() {
        val report = VpnRoutingDiagnostics("app", "except", listOf("org.example.checker"))
            .withResult(emptyList(), listOf("org.example.checker"), emptyList(), true)
            .render(35, "15", "off")

        assertTrue(report.contains("Current VPN state: off"))
        assertTrue(report.contains("Applied mode: except"))
        assertTrue(report.contains("Applied packages: org.example.checker"))
    }
}
