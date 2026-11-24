package com.unigate.app.vpn

import org.json.JSONObject
import java.util.UUID

internal class VpnLocalConfig(
    val json: String,
    val clashSecret: String,
    val awgPassword: String?,
) {
    companion object {
        fun prepare(savedConfig: String, usesAwg: Boolean): VpnLocalConfig {
            val config = JSONObject(savedConfig)
            val clashSecret = UUID.randomUUID().toString()
            config.getJSONObject("experimental").getJSONObject("clash_api").put("secret", clashSecret)

            val awgPassword = if (usesAwg) {
                val outbounds = config.getJSONArray("outbounds")
                val proxy = (0 until outbounds.length())
                    .map(outbounds::getJSONObject)
                    .single { it.optString("tag") == "proxy" }
                check(
                    proxy.optString("type") == "socks" &&
                        proxy.optString("server") == "127.0.0.1" &&
                        proxy.optInt("server_port") == 2081,
                ) { "Invalid AmneziaWG SOCKS outbound" }
                UUID.randomUUID().toString().also {
                    proxy.put("username", "unigate")
                    proxy.put("password", it)
                    proxy.put("udp_over_tcp", JSONObject().put("enabled", true).put("version", 1))
                }
            } else {
                null
            }
            return VpnLocalConfig(config.toString(), clashSecret, awgPassword)
        }
    }
}
