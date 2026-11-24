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

    @Test
    fun blocksUnapprovedTunTrafficAndKeepsExistingRoutesForSelectedPackages() {
        val selected = listOf("org.example.chat", "org.example.radio")
        val config = JSONObject(
            VpnLocalConfig.prepare(routingConfig, usesAwg = false, checkedPackages = selected).json,
        )
        val routes = config.getJSONObject("route").getJSONArray("rules")
        assertEquals("sniff", routes.getJSONObject(0).getString("action"))
        assertEquals("hijack-dns", routes.getJSONObject(1).getString("action"))
        val guard = routes.getJSONObject(2)
        assertEquals("reject", guard.getString("action"))
        assertEquals(true, guard.getBoolean("invert"))
        assertEquals(org.json.JSONArray(selected).toString(), guard.getJSONArray("package_name").toString())
        assertEquals("direct", routes.getJSONObject(3).getString("outbound"))

        val dns = config.getJSONObject("dns").getJSONArray("rules").getJSONObject(0)
        assertEquals("local", dns.getString("server"))
        val conditions = dns.getJSONArray("rules")
        assertEquals("tun-in", conditions.getJSONObject(0).getString("inbound"))
        assertEquals(true, conditions.getJSONObject(1).getBoolean("invert"))
        assertEquals(
            guard.getJSONArray("package_name").toString(),
            conditions.getJSONObject(1).getJSONArray("package_name").toString(),
        )
    }

    @Test
    fun leavesRoutingUnchangedWithoutAnAllowList() {
        val before = JSONObject(routingConfig)
        val after = JSONObject(VpnLocalConfig.prepare(routingConfig, usesAwg = false).json)
        assertEquals(before.getJSONObject("route").toString(), after.getJSONObject("route").toString())
        assertEquals(before.getJSONObject("dns").toString(), after.getJSONObject("dns").toString())
    }

    private val routingConfig = """
        {
          "experimental": {"clash_api": {"external_controller": "127.0.0.1:9090"}},
          "route": {"final": "proxy", "rules": [
             {"action": "sniff"},
             {"protocol": "dns", "action": "hijack-dns"},
             {"ip_is_private": true, "outbound": "direct"}
          ]},
          "dns": {"final": "remote", "rules": [
             {"domain_suffix": [".ru"], "action": "route", "server": "local"}
          ]}
        }
    """.trimIndent()
}
