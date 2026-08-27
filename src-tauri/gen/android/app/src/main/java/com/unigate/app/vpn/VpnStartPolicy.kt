package com.unigate.app.vpn

internal object VpnStartPolicy {
    fun shouldRestore(
        explicitConfig: String?,
        reconnectRequested: Boolean,
        alwaysOn: Boolean,
    ): Boolean = explicitConfig.isNullOrBlank() && (reconnectRequested || alwaysOn)
}
