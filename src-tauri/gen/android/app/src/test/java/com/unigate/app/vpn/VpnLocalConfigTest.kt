package com.unigate.app.vpn

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Test

class VpnLocalConfigTest {
    @Test
    fun preparesSavedAmneziaConfigWithFreshCredentials() {
        val saved = """
            {
              "experimental": {"clash_api": {"external_controller": "127.0.0.1:9090"}},
              "outbounds": [
                {"type": "direct", "tag": "direct"},
                {"type": "socks", "tag": "proxy", "server": "127.0.0.1", "server_port": 2081}
              ]
            }
        """.trimIndent()
        val first = VpnLocalConfig.prepare(saved, usesAwg = true)
        val next = VpnLocalConfig.prepare(saved, usesAwg = true)
        val config = JSONObject(next.json)
        val proxy = config.getJSONArray("outbounds").getJSONObject(1)

        assertNotEquals(first.clashSecret, next.clashSecret)
        assertNotEquals(first.awgPassword, next.awgPassword)
        assertNotEquals(next.awgPassword, next.clashSecret)
        assertEquals(next.clashSecret, config.getJSONObject("experimental").getJSONObject("clash_api").getString("secret"))
        assertEquals("unigate", proxy.getString("username"))
        assertEquals(next.awgPassword, proxy.getString("password"))
        val udpOverTcp = proxy.getJSONObject("udp_over_tcp")
        assertEquals(true, udpOverTcp.getBoolean("enabled"))
        assertEquals(1, udpOverTcp.getInt("version"))
    }

    @Test
    fun preservesOrdinaryProxyCredentials() {
        val saved = """
            {
              "experimental": {"clash_api": {"external_controller": "127.0.0.1:9090"}},
              "outbounds": [{"type": "socks", "tag": "proxy", "password": "profile-password"}]
            }
        """.trimIndent()
        val local = VpnLocalConfig.prepare(saved, usesAwg = false)
        assertNull(local.awgPassword)
        assertEquals(
            "profile-password",
            JSONObject(local.json).getJSONArray("outbounds").getJSONObject(0).getString("password"),
        )
    }
}
