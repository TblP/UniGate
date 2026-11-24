package com.unigate.app.vpn

import org.json.JSONArray
import org.json.JSONObject
import java.util.UUID

internal class VpnLocalConfig(
    val json: String,
    val clashSecret: String,
    val awgPassword: String?,
) {
    companion object {
        fun prepare(
            savedConfig: String,
            usesAwg: Boolean,
            checkedPackages: List<String> = emptyList(),
        ): VpnLocalConfig {
            val config = JSONObject(savedConfig)
            if (checkedPackages.isNotEmpty()) {
                addPackageGuard(config, checkedPackages)
            }
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

        private fun addPackageGuard(config: JSONObject, packages: List<String>) {
            val names = JSONArray(packages.distinct())
            val route = config.getJSONObject("route")
            val existing = route.getJSONArray("rules")
            val rules = JSONArray()
            // Keep sniffing and the DNS handler first. Every other route must
            // see the package check, including the fallback proxy route.
            var index = 0
            while (index < existing.length()) {
                val rule = existing.getJSONObject(index)
                if (rule.optString("action") !in listOf("sniff", "hijack-dns")) break
                rules.put(rule)
                index++
            }
            rules.put(JSONObject().put("package_name", names).put("invert", true).put("action", "reject"))
            while (index < existing.length()) {
                rules.put(existing.getJSONObject(index++))
            }
            route.put("rules", rules)

            // Android's DNS resolver may own the socket on behalf of the app.
            // Such queries must work without using the VPN DNS upstream.
            val dns = config.getJSONObject("dns")
            val existingDns = dns.optJSONArray("rules") ?: JSONArray()
            val dnsRules = JSONArray().put(
                JSONObject().put("type", "logical").put("mode", "and").put(
                    "rules",
                    JSONArray()
                        .put(JSONObject().put("inbound", "tun-in"))
                        .put(JSONObject().put("package_name", names).put("invert", true)),
                ).put("action", "route").put("server", "local"),
            )
            for (i in 0 until existingDns.length()) {
                dnsRules.put(existingDns.getJSONObject(i))
            }
            dns.put("rules", dnsRules)
        }
    }
}
