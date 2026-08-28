package com.unigate.app.vpn

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager.NameNotFoundException
import android.net.ConnectivityManager
import android.net.IpPrefix
import android.net.LinkProperties
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.VpnService
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.ParcelFileDescriptor
import android.system.OsConstants
import androidx.core.app.NotificationCompat
import com.unigate.app.awgshim.Awgshim
import com.unigate.app.awgshim.Protector
import com.unigate.app.MainActivity
import com.unigate.app.R
import io.nekohasekai.libbox.CommandServer
import io.nekohasekai.libbox.CommandServerHandler
import io.nekohasekai.libbox.ConnectionOwner
import io.nekohasekai.libbox.InterfaceUpdateListener
import io.nekohasekai.libbox.Libbox
import io.nekohasekai.libbox.LocalDNSTransport
import io.nekohasekai.libbox.NetworkInterfaceIterator
import io.nekohasekai.libbox.Notification
import io.nekohasekai.libbox.OverrideOptions
import io.nekohasekai.libbox.PlatformInterface
import io.nekohasekai.libbox.SetupOptions
import io.nekohasekai.libbox.StringIterator
import io.nekohasekai.libbox.SystemProxyStatus
import io.nekohasekai.libbox.TunOptions
import io.nekohasekai.libbox.WIFIState
import java.net.Inet6Address
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.NetworkInterface
import java.util.concurrent.atomic.AtomicBoolean
import io.nekohasekai.libbox.NetworkInterface as LibboxNetworkInterface

class UniGateVpnService : VpnService(), PlatformInterface, CommandServerHandler {
    companion object {
        const val ACTION_START = "com.unigate.app.vpn.START"
        const val ACTION_STOP = "com.unigate.app.vpn.STOP"
        const val EXTRA_CONFIG = "config"
        const val EXTRA_AWG_CONFIG = "awgConfig"
        const val EXTRA_PROFILE_NAME = "profileName"
        const val EXTRA_INCLUDE_PACKAGES = "includePackages"
        const val EXTRA_EXCLUDE_PACKAGES = "excludePackages"
        const val EXTRA_START_TOKEN = "startToken"

        private const val CHANNEL_ID = "unigate_vpn"
        private const val NOTIFICATION_ID = 1
        private val libboxReady = AtomicBoolean(false)
        private val startMonitor = Object()
        private var startSequence = 0L
        private var completedStart = 0L
        private var startFailure: String? = null
        @Volatile
        private var liveState = VpnQuickControl.State.OFF

        fun runtimeState(): VpnQuickControl.State = liveState

        fun beginStart(): Long = synchronized(startMonitor) {
            startSequence += 1
            startFailure = null
            startSequence
        }

        fun awaitStart(token: Long, timeoutMillis: Long): String? = synchronized(startMonitor) {
            val deadline = System.currentTimeMillis() + timeoutMillis
            while (completedStart < token) {
                val remaining = deadline - System.currentTimeMillis()
                if (remaining <= 0) {
                    return@synchronized "VPN не запустился за ${timeoutMillis / 1000} с"
                }
                startMonitor.wait(remaining)
            }
            if (completedStart > token) {
                "Запуск VPN был заменён более новым запросом"
            } else {
                startFailure
            }
        }

        private fun finishStart(token: Long, error: String?) = synchronized(startMonitor) {
            if (token >= completedStart) {
                completedStart = token
                startFailure = error
                startMonitor.notifyAll()
            }
        }
    }

    private val connectivity by lazy {
        getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
    }
    private var commandServer: CommandServer? = null
    private var tunnel: ParcelFileDescriptor? = null
    private var defaultNetworkCallback: ConnectivityManager.NetworkCallback? = null
    private var defaultInterfaceListener: InterfaceUpdateListener? = null
    private var defaultNetwork: Network? = null
    private var profileName = "UniGate"

    override fun onBind(intent: Intent): IBinder? = super.onBind(intent)

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            VpnQuickControl.setShouldReconnect(this, false)
            stopVpn()
            return START_NOT_STICKY
        }

        val explicitConfig = intent?.getStringExtra(EXTRA_CONFIG)
        val explicitAwgConfig = intent?.getStringExtra(EXTRA_AWG_CONFIG)
        val alwaysOnRequested = intent?.action == SERVICE_INTERFACE ||
            (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q && isAlwaysOn)
        val restoreRequested = VpnStartPolicy.shouldRestore(
            explicitConfig = explicitConfig,
            reconnectRequested = VpnQuickControl.shouldReconnect(this),
            alwaysOn = alwaysOnRequested,
        )
        val saved = if (restoreRequested) VpnQuickControl.savedConnection(this) else null
        val config = explicitConfig?.takeIf { it.isNotBlank() } ?: saved?.config
        val awgConfig = if (explicitConfig != null) {
            explicitAwgConfig?.takeIf { it.isNotBlank() }
        } else {
            saved?.awgConfig
        }
        if (config.isNullOrBlank()) {
            liveState = VpnQuickControl.State.OFF
            VpnQuickControl.setState(this, VpnQuickControl.State.OFF)
            stopSelf(startId)
            return START_NOT_STICKY
        }

        profileName = intent?.getStringExtra(EXTRA_PROFILE_NAME)
            .orEmpty()
            .ifBlank { saved?.profileName ?: "UniGate" }
        liveState = VpnQuickControl.State.CONNECTING
        VpnQuickControl.setState(this, VpnQuickControl.State.CONNECTING)
        showForeground("Подключение…")
        val startToken = intent?.getLongExtra(EXTRA_START_TOKEN, 0L) ?: 0L
        val includePackages =
            intent?.getStringArrayListExtra(EXTRA_INCLUDE_PACKAGES) ?: saved?.includePackages.orEmpty()
        val excludePackages =
            intent?.getStringArrayListExtra(EXTRA_EXCLUDE_PACKAGES) ?: saved?.excludePackages.orEmpty()
        VpnQuickControl.setShouldReconnect(this, true)

        Thread {
            runCatching {
                startVpn(config, awgConfig, includePackages, excludePackages)
            }.onSuccess {
                finishStart(startToken, null)
                liveState = VpnQuickControl.State.ON
                VpnQuickControl.setState(this, VpnQuickControl.State.ON)
                showForeground("VPN подключён")
            }.onFailure {
                finishStart(startToken, it.message ?: "Неизвестная ошибка VPN")
                liveState = VpnQuickControl.State.OFF
                VpnQuickControl.setShouldReconnect(this, false)
                VpnQuickControl.setState(this, VpnQuickControl.State.OFF)
                showForeground("Ошибка VPN: ${it.message ?: "неизвестная ошибка"}")
                stopVpn()
            }
        }.start()
        return START_STICKY
    }

    @Synchronized
    private fun startVpn(
        config: String,
        awgConfig: String?,
        includePackages: List<String>,
        excludePackages: List<String>,
    ) {
        closeCore()
        if (!awgConfig.isNullOrBlank()) {
            Awgshim.start(
                awgConfig,
                "127.0.0.1:2081",
                object : Protector {
                    override fun protect(fd: Long): Boolean =
                        this@UniGateVpnService.protect(fd.toInt())
                },
            )
        }
        setupLibbox()

        val server = CommandServer(this, this)
        server.start()
        val overrideOptions = OverrideOptions().apply {
            if (includePackages.isNotEmpty()) {
                includePackage = ListStringIterator(includePackages)
            } else if (excludePackages.isNotEmpty()) {
                excludePackage = ListStringIterator(excludePackages)
            }
        }
        server.startOrReloadService(config, overrideOptions)
        commandServer = server
    }

    private fun setupLibbox() {
        if (libboxReady.compareAndSet(false, true)) {
            Libbox.setup(
                SetupOptions().apply {
                    basePath = filesDir.absolutePath
                    workingPath = filesDir.absolutePath
                    tempPath = cacheDir.absolutePath
                    fixAndroidStack = true
                    logMaxLines = 500
                },
            )
        }
    }

    private fun showForeground(text: String) {
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            manager.createNotificationChannel(
                NotificationChannel(
                    CHANNEL_ID,
                    "UniGate VPN",
                    NotificationManager.IMPORTANCE_LOW,
                ),
            )
        }
        val pendingIntent = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val notification = NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.mipmap.ic_launcher)
            .setContentTitle(profileName)
            .setContentText(text)
            .setContentIntent(pendingIntent)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .build()
        startForeground(NOTIFICATION_ID, notification)
    }

    @Synchronized
    private fun stopVpn() {
        closeCore()
        liveState = VpnQuickControl.State.OFF
        VpnQuickControl.setState(this, VpnQuickControl.State.OFF)
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    private fun closeCore() {
        runCatching { commandServer?.closeService() }
        runCatching { commandServer?.close() }
        commandServer = null
        runCatching { Awgshim.stop() }
        tunnel?.close()
        tunnel = null
        defaultNetworkCallback?.let { runCatching { connectivity.unregisterNetworkCallback(it) } }
        defaultNetworkCallback = null
        defaultInterfaceListener = null
        defaultNetwork = null
        runCatching { setUnderlyingNetworks(null) }
    }

    override fun onDestroy() {
        closeCore()
        liveState = VpnQuickControl.State.OFF
        VpnQuickControl.setState(this, VpnQuickControl.State.OFF)
        super.onDestroy()
    }

    override fun onRevoke() {
        VpnQuickControl.setShouldReconnect(this, false)
        stopVpn()
        super.onRevoke()
    }

    override fun openTun(options: TunOptions): Int {
        if (prepare(this) != null) {
            error("Android VPN permission is missing")
        }
        val builder = Builder()
            .setSession(profileName)
            .setMtu(options.mtu)

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            builder.setMetered(false)
        }
        addAddresses(builder, options.inet4Address)
        addAddresses(builder, options.inet6Address)

        if (options.autoRoute) {
            val dns = options.dnsServerAddress?.value
            if (!dns.isNullOrBlank()) {
                builder.addDnsServer(dns)
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                if (!addRoutes(builder, options.inet4RouteAddress) &&
                    options.inet4Address.hasNext()
                ) {
                    builder.addRoute("0.0.0.0", 0)
                }
                if (!addRoutes(builder, options.inet6RouteAddress) &&
                    options.inet6Address.hasNext()
                ) {
                    builder.addRoute("::", 0)
                }
                addExcludedRoutes(builder, options.inet4RouteExcludeAddress)
                addExcludedRoutes(builder, options.inet6RouteExcludeAddress)
            } else {
                // libbox expands include/exclude rules into non-overlapping
                // ranges on Android versions that lack VpnService.excludeRoute.
                addRoutes(builder, options.inet4RouteRange)
                addRoutes(builder, options.inet6RouteRange)
            }
        }

        addIncludedPackages(builder, options.includePackage)
        addExcludedPackages(builder, options.excludePackage)

        val descriptor = builder.establish()
            ?: error("Android refused to establish the VPN interface")
        tunnel = descriptor
        return descriptor.fd
    }

    private fun addAddresses(builder: Builder, iterator: io.nekohasekai.libbox.RoutePrefixIterator) {
        while (iterator.hasNext()) {
            val prefix = iterator.next()
            builder.addAddress(prefix.address(), prefix.prefix())
        }
    }

    private fun addRoutes(
        builder: Builder,
        iterator: io.nekohasekai.libbox.RoutePrefixIterator,
    ): Boolean {
        var added = false
        while (iterator.hasNext()) {
            val prefix = iterator.next()
            builder.addRoute(prefix.address(), prefix.prefix())
            added = true
        }
        return added
    }

    private fun addExcludedRoutes(
        builder: Builder,
        iterator: io.nekohasekai.libbox.RoutePrefixIterator,
    ) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
        while (iterator.hasNext()) {
            val prefix = iterator.next()
            builder.excludeRoute(
                IpPrefix(InetAddress.getByName(prefix.address()), prefix.prefix()),
            )
        }
    }

    private fun addIncludedPackages(builder: Builder, iterator: StringIterator?) {
        if (iterator == null) return
        while (iterator.hasNext()) {
            try {
                builder.addAllowedApplication(iterator.next())
            } catch (_: NameNotFoundException) {
            }
        }
    }

    private fun addExcludedPackages(builder: Builder, iterator: StringIterator?) {
        if (iterator == null) return
        while (iterator.hasNext()) {
            try {
                builder.addDisallowedApplication(iterator.next())
            } catch (_: NameNotFoundException) {
            }
        }
    }

    override fun usePlatformAutoDetectInterfaceControl(): Boolean = true
    override fun autoDetectInterfaceControl(fd: Int) {
        protect(fd)
    }
    override fun useProcFS(): Boolean = Build.VERSION.SDK_INT < Build.VERSION_CODES.Q
    override fun underNetworkExtension(): Boolean = false
    override fun includeAllNetworks(): Boolean = false
    override fun clearDNSCache() = Unit
    override fun localDNSTransport(): LocalDNSTransport? = null
    override fun readWIFIState(): WIFIState? = null
    override fun systemCertificates(): StringIterator? = null
    override fun sendNotification(notification: Notification?) = Unit

    override fun findConnectionOwner(
        ipProtocol: Int,
        sourceAddress: String?,
        sourcePort: Int,
        destinationAddress: String?,
        destinationPort: Int,
    ): ConnectionOwner {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
            error("Connection owner lookup requires Android 10")
        }
        val uid = connectivity.getConnectionOwnerUid(
            ipProtocol,
            InetSocketAddress(sourceAddress, sourcePort),
            InetSocketAddress(destinationAddress, destinationPort),
        )
        return ConnectionOwner().apply {
            userId = uid
            userName = packageManager.getPackagesForUid(uid)?.firstOrNull().orEmpty()
        }
    }

    override fun getInterfaces(): NetworkInterfaceIterator {
        val interfaces = NetworkInterface.getNetworkInterfaces().toList().map { network ->
            LibboxNetworkInterface().apply {
                name = network.name
                index = network.index
                mtu = runCatching { network.mtu }.getOrDefault(1500)
                addresses = ListStringIterator(
                    network.interfaceAddresses.map {
                        val host = if (it.address is Inet6Address) {
                            Inet6Address.getByAddress(it.address.address).hostAddress
                        } else {
                            it.address.hostAddress
                        }
                        "$host/${it.networkPrefixLength}"
                    },
                )
                flags = (if (network.isUp) OsConstants.IFF_UP else 0) or
                    (if (network.isLoopback) OsConstants.IFF_LOOPBACK else 0) or
                    (if (network.isPointToPoint) OsConstants.IFF_POINTOPOINT else 0)
                type = Libbox.InterfaceTypeOther
                metered = false
            }
        }
        return ListNetworkInterfaceIterator(interfaces)
    }

    override fun startDefaultInterfaceMonitor(listener: InterfaceUpdateListener) {
        defaultInterfaceListener = listener
        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                defaultNetwork = network
                runCatching { setUnderlyingNetworks(arrayOf(network)) }
            }

            override fun onCapabilitiesChanged(
                network: Network,
                networkCapabilities: NetworkCapabilities,
            ) {
                if (defaultNetwork == network &&
                    !networkCapabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_VPN)
                ) {
                    clearDefaultInterface(listener)
                }
            }

            override fun onLinkPropertiesChanged(network: Network, linkProperties: LinkProperties) {
                if (defaultNetwork == network) {
                    updateDefaultInterface(network, linkProperties)
                }
            }

            override fun onLost(network: Network) {
                if (defaultNetwork != network) return
                clearDefaultInterface(listener)
            }
        }
        defaultNetworkCallback = callback
        val request = NetworkRequest.Builder()
            .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            .addCapability(NetworkCapabilities.NET_CAPABILITY_NOT_RESTRICTED)
            .addCapability(NetworkCapabilities.NET_CAPABILITY_NOT_VPN)
            .build()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            connectivity.registerBestMatchingNetworkCallback(
                request,
                callback,
                Handler(Looper.getMainLooper()),
            )
        } else {
            connectivity.requestNetwork(request, callback)
        }
        findUnderlyingNetwork()?.let {
            defaultNetwork = it
            runCatching { setUnderlyingNetworks(arrayOf(it)) }
            connectivity.getLinkProperties(it)?.let { properties ->
                updateDefaultInterface(it, properties)
            }
        }
    }

    private fun findUnderlyingNetwork(): Network? =
        connectivity.allNetworks.firstOrNull { network ->
            val capabilities =
                connectivity.getNetworkCapabilities(network) ?: return@firstOrNull false
            capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET) &&
                capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_VPN)
        }

    private fun updateDefaultInterface(network: Network, linkProperties: LinkProperties) {
        val name = linkProperties.interfaceName ?: return
        val index = runCatching { NetworkInterface.getByName(name)?.index ?: -1 }.getOrDefault(-1)
        if (index >= 0) {
            defaultNetwork = network
            runCatching { setUnderlyingNetworks(arrayOf(network)) }
            defaultInterfaceListener?.updateDefaultInterface(name, index, false, false)
        }
    }

    private fun clearDefaultInterface(listener: InterfaceUpdateListener) {
        defaultNetwork = null
        runCatching { setUnderlyingNetworks(emptyArray()) }
        listener.updateDefaultInterface("", -1, false, false)
    }

    override fun closeDefaultInterfaceMonitor(listener: InterfaceUpdateListener?) {
        defaultNetworkCallback?.let { runCatching { connectivity.unregisterNetworkCallback(it) } }
        defaultNetworkCallback = null
        defaultInterfaceListener = null
        defaultNetwork = null
    }

    override fun serviceStop() = stopVpn()
    override fun serviceReload() = Unit
    override fun getSystemProxyStatus(): SystemProxyStatus? = null
    override fun setSystemProxyEnabled(enabled: Boolean) = Unit
    override fun writeDebugMessage(message: String?) = Unit
}

private class ListStringIterator(values: List<String>) : StringIterator {
    private val iterator = values.iterator()
    private val size = values.size
    override fun len(): Int = size
    override fun hasNext(): Boolean = iterator.hasNext()
    override fun next(): String = iterator.next()
}

private class ListNetworkInterfaceIterator(
    values: List<LibboxNetworkInterface>,
) : NetworkInterfaceIterator {
    private val iterator = values.iterator()
    override fun hasNext(): Boolean = iterator.hasNext()
    override fun next(): LibboxNetworkInterface = iterator.next()
}
