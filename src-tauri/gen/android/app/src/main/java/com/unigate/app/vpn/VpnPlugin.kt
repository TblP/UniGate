package com.unigate.app.vpn

import android.app.Activity
import android.app.StatusBarManager
import android.appwidget.AppWidgetManager
import android.content.ComponentName
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.drawable.Icon
import android.graphics.drawable.BitmapDrawable
import android.graphics.drawable.Drawable
import android.net.VpnService
import android.os.Build
import android.util.Base64
import androidx.activity.result.ActivityResult
import androidx.core.content.ContextCompat
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import org.json.JSONArray
import java.io.ByteArrayOutputStream
import java.io.File

@InvokeArg
class StartVpnArgs {
    lateinit var config: String
    var profileName: String = "UniGate"
    var includePackages: List<String> = emptyList()
    var excludePackages: List<String> = emptyList()
}

@TauriPlugin
class VpnPlugin(private val activity: Activity) : Plugin(activity) {
    @Volatile
    private var cachedApps: List<Triple<String, String, String>>? = null

    @Command
    fun prepareAssets(invoke: Invoke) {
        try {
            val destination = File(activity.filesDir, "geoip-ru.srs")
            activity.assets.open("geoip-ru.srs").use { source ->
                destination.outputStream().use { target -> source.copyTo(target) }
            }
            invoke.resolve(JSObject().apply { put("geoipPath", destination.absolutePath) })
        } catch (error: Exception) {
            invoke.reject(error.message ?: "Не удалось подготовить geoip-ru.srs", error)
        }
    }

    @Command
    fun listApps(invoke: Invoke) {
        Thread {
            try {
                val apps = cachedApps ?: loadApps().also { cachedApps = it }
                val array = JSONArray()
                apps.forEach { (packageName, name, icon) ->
                    array.put(
                        JSObject().apply {
                            put("packageName", packageName)
                            put("name", name)
                            put("icon", icon)
                        },
                    )
                }
                activity.runOnUiThread {
                    invoke.resolve(JSObject().apply { put("apps", array) })
                }
            } catch (error: Exception) {
                activity.runOnUiThread {
                    invoke.reject(error.message ?: "Не удалось получить список приложений", error)
                }
            }
        }.start()
    }

    @Command
    fun status(invoke: Invoke) {
        val state = UniGateVpnService.runtimeState()
        if (VpnQuickControl.state(activity) != state) {
            VpnQuickControl.setState(activity, state)
        }
        invoke.resolve(
            JSObject().apply {
                put("state", state.name.lowercase())
                put("quickConnectReady", VpnQuickControl.hasSavedConnection(activity))
            },
        )
    }

    @Command
    fun requestQuickTile(invoke: Invoke) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
            invoke.resolve(JSObject().apply { put("added", false) })
            return
        }
        val manager = activity.getSystemService(StatusBarManager::class.java)
        manager.requestAddTileService(
            ComponentName(activity, UniGateTileService::class.java),
            activity.getString(com.unigate.app.R.string.tile_label),
            Icon.createWithResource(activity, com.unigate.app.R.drawable.ic_tile_vpn),
            activity.mainExecutor,
        ) { result ->
            val added =
                result == StatusBarManager.TILE_ADD_REQUEST_RESULT_TILE_ADDED ||
                    result == StatusBarManager.TILE_ADD_REQUEST_RESULT_TILE_ALREADY_ADDED
            invoke.resolve(JSObject().apply {
                put("added", added)
                put("result", result)
            })
        }
    }

    @Command
    fun requestWidget(invoke: Invoke) {
        val manager = AppWidgetManager.getInstance(activity)
        val added =
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
                manager.isRequestPinAppWidgetSupported &&
                manager.requestPinAppWidget(
                    ComponentName(activity, UniGateCompactWidgetProvider::class.java),
                    null,
                    null,
                )
        invoke.resolve(JSObject().apply { put("added", added) })
    }

    @Command
    fun start(invoke: Invoke) {
        val permissionIntent = VpnService.prepare(activity)
        if (permissionIntent == null) {
            startService(invoke)
        } else {
            startActivityForResult(invoke, permissionIntent, "vpnPermissionResult")
        }
    }

    @ActivityCallback
    fun vpnPermissionResult(invoke: Invoke, result: ActivityResult) {
        if (result.resultCode == Activity.RESULT_OK) {
            startService(invoke)
        } else {
            invoke.reject("Android не выдал разрешение на создание VPN")
        }
    }

    private fun startService(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(StartVpnArgs::class.java)
            VpnQuickControl.saveConnection(
                activity,
                args.config,
                args.profileName,
                args.includePackages,
                args.excludePackages,
            )
            VpnQuickControl.setShouldReconnect(activity, true)
            val startToken = UniGateVpnService.beginStart()
            val intent = Intent(activity, UniGateVpnService::class.java).apply {
                action = UniGateVpnService.ACTION_START
                putExtra(UniGateVpnService.EXTRA_CONFIG, args.config)
                putExtra(UniGateVpnService.EXTRA_PROFILE_NAME, args.profileName)
                putExtra(UniGateVpnService.EXTRA_START_TOKEN, startToken)
                putStringArrayListExtra(
                    UniGateVpnService.EXTRA_INCLUDE_PACKAGES,
                    ArrayList(args.includePackages),
                )
                putStringArrayListExtra(
                    UniGateVpnService.EXTRA_EXCLUDE_PACKAGES,
                    ArrayList(args.excludePackages),
                )
            }
            ContextCompat.startForegroundService(activity, intent)
            Thread {
                val failure = UniGateVpnService.awaitStart(startToken, 30_000)
                if (failure == null) {
                    invoke.resolve(JSObject().apply { put("started", true) })
                } else {
                    invoke.reject(failure)
                }
            }.start()
        } catch (error: Exception) {
            VpnQuickControl.setShouldReconnect(activity, false)
            invoke.reject(error.message ?: "Не удалось запустить Android VPN", error)
        }
    }

    @Command
    fun stop(invoke: Invoke) {
        activity.startService(
            Intent(activity, UniGateVpnService::class.java).apply {
                action = UniGateVpnService.ACTION_STOP
            },
        )
        invoke.resolve()
    }

    private fun drawableToDataUri(drawable: Drawable): String {
        val size = 40
        val bitmap = if (drawable is BitmapDrawable && drawable.bitmap != null) {
            Bitmap.createScaledBitmap(drawable.bitmap, size, size, true)
        } else {
            Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888).also { target ->
                val canvas = Canvas(target)
                drawable.setBounds(0, 0, size, size)
                drawable.draw(canvas)
            }
        }
        val bytes = ByteArrayOutputStream().use { output ->
            bitmap.compress(Bitmap.CompressFormat.PNG, 90, output)
            output.toByteArray()
        }
        return "data:image/png;base64," + Base64.encodeToString(bytes, Base64.NO_WRAP)
    }

    private fun loadApps(): List<Triple<String, String, String>> {
        val packageManager = activity.packageManager
        val launcherIntent = Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER)
        return packageManager.queryIntentActivities(launcherIntent, 0)
            .map { info ->
                val packageName = info.activityInfo.packageName
                Triple(
                    packageName,
                    info.loadLabel(packageManager).toString().ifBlank { packageName },
                    drawableToDataUri(info.loadIcon(packageManager)),
                )
            }
            .filter { it.first != activity.packageName }
            .distinctBy { it.first }
            .sortedBy { it.second.lowercase() }
    }
}
