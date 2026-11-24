package com.unigate.app.vpn

internal data class VpnRoutingDiagnostics(
    val source: String,
    val requestedMode: String,
    val requestedPackages: List<String>,
    val appliedMode: String? = null,
    val appliedPackages: List<String>? = null,
    val missingPackages: List<String> = emptyList(),
    val tunEstablished: Boolean = false,
) {
    fun withResult(
        allowed: List<String>,
        excluded: List<String>,
        missing: List<String>,
        established: Boolean,
    ) = copy(
        appliedMode = when {
            !established -> null
            allowed.isNotEmpty() -> "only"
            excluded.isNotEmpty() -> "except"
            else -> "all"
        },
        appliedPackages = allowed + excluded,
        missingPackages = missing,
        tunEstablished = established,
    )

    fun render(sdk: Int, androidVersion: String, currentState: String): String {
        return listOf(
            "UniGate 1.2.4 Android routing diagnostics",
            "Android: $androidVersion (API $sdk)",
            "Current VPN state: $currentState",
            "Source: $source",
            "Requested mode: $requestedMode",
            "Requested packages: ${requestedPackages.display()}",
            "Applied mode: ${appliedMode ?: "unconfirmed"}",
            "Applied packages: ${appliedPackages?.display() ?: "(not received)"}",
            "Missing packages: ${missingPackages.display()}",
            "TUN established: $tunEstablished",
        ).joinToString("\n")
    }

    private fun List<String>.display() = sorted().joinToString(", ").ifEmpty { "(none)" }
}
