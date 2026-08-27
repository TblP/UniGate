package com.unigate.app.vpn

import android.app.PendingIntent
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.VpnService
import android.service.quicksettings.TileService
import androidx.core.content.ContextCompat
import com.unigate.app.MainActivity

object VpnQuickControl {
    data class SavedConnection(
        val config: String,
        val profileName: String,
        val includePackages: List<String>,
        val excludePackages: List<String>,
    )

    enum class State {
        OFF,
        CONNECTING,
        ON,
    }

    const val ACTION_CONNECT = "com.unigate.app.action.CONNECT"
    const val ACTION_DISCONNECT = "com.unigate.app.action.DISCONNECT"
    const val ACTION_TOGGLE = "com.unigate.app.action.TOGGLE"

    private const val PREFS = "unigate_quick_control"
    private const val KEY_CONFIG = "config"
    private const val KEY_PROFILE_NAME = "profile_name"
    private const val KEY_INCLUDE_PACKAGES = "include_packages"
    private const val KEY_EXCLUDE_PACKAGES = "exclude_packages"
    private const val KEY_STATE = "state"
    private const val KEY_SHOULD_RECONNECT = "should_reconnect"

    fun saveConnection(
        context: Context,
        config: String,
        profileName: String,
        includePackages: List<String>,
        excludePackages: List<String>,
    ) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putString(KEY_CONFIG, config)
            .putString(KEY_PROFILE_NAME, profileName)
            .putStringSet(KEY_INCLUDE_PACKAGES, includePackages.toSet())
            .putStringSet(KEY_EXCLUDE_PACKAGES, excludePackages.toSet())
            .apply()
    }

    fun hasSavedConnection(context: Context): Boolean =
        !context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .getString(KEY_CONFIG, null)
            .isNullOrBlank()

    fun savedConnection(context: Context): SavedConnection? {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val config = prefs.getString(KEY_CONFIG, null)?.takeIf { it.isNotBlank() } ?: return null
        return SavedConnection(
            config = config,
            profileName = prefs.getString(KEY_PROFILE_NAME, "UniGate").orEmpty().ifBlank { "UniGate" },
            includePackages = prefs.getStringSet(KEY_INCLUDE_PACKAGES, emptySet()).orEmpty().toList(),
            excludePackages = prefs.getStringSet(KEY_EXCLUDE_PACKAGES, emptySet()).orEmpty().toList(),
        )
    }

    fun shouldReconnect(context: Context): Boolean =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .getBoolean(KEY_SHOULD_RECONNECT, false)

    fun setShouldReconnect(context: Context, enabled: Boolean) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putBoolean(KEY_SHOULD_RECONNECT, enabled)
            .commit()
    }

    fun state(context: Context): State {
        val stored = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .getString(KEY_STATE, State.OFF.name)
        return runCatching { State.valueOf(stored ?: State.OFF.name) }.getOrDefault(State.OFF)
    }

    fun isRunning(context: Context): Boolean = state(context) != State.OFF

    fun setState(context: Context, state: State) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putString(KEY_STATE, state.name)
            .apply()
        UniGateWidgetProvider.updateAll(context)
        UniGateCompactWidgetProvider.updateAll(context)
        TileService.requestListeningState(
            context,
            ComponentName(context, UniGateTileService::class.java),
        )
    }

    fun connect(context: Context): Boolean {
        val connection = savedConnection(context)
        if (connection == null || VpnService.prepare(context) != null) return false

        val intent = Intent(context, UniGateVpnService::class.java).apply {
            action = UniGateVpnService.ACTION_START
            putExtra(UniGateVpnService.EXTRA_CONFIG, connection.config)
            putExtra(UniGateVpnService.EXTRA_PROFILE_NAME, connection.profileName)
            putExtra(UniGateVpnService.EXTRA_START_TOKEN, UniGateVpnService.beginStart())
            putStringArrayListExtra(
                UniGateVpnService.EXTRA_INCLUDE_PACKAGES,
                ArrayList(connection.includePackages),
            )
            putStringArrayListExtra(
                UniGateVpnService.EXTRA_EXCLUDE_PACKAGES,
                ArrayList(connection.excludePackages),
            )
        }
        return runCatching {
            setShouldReconnect(context, true)
            setState(context, State.CONNECTING)
            ContextCompat.startForegroundService(context, intent)
            true
        }.getOrElse {
            setShouldReconnect(context, false)
            setState(context, State.OFF)
            false
        }
    }

    fun disconnect(context: Context) {
        setShouldReconnect(context, false)
        context.startService(
            Intent(context, UniGateVpnService::class.java).apply {
                action = UniGateVpnService.ACTION_STOP
            },
        )
        setState(context, State.OFF)
    }

    fun toggle(context: Context) {
        if (isRunning(context)) {
            disconnect(context)
        } else if (!connect(context)) {
            requestConnect(context)
        }
    }

    fun connectPendingIntent(context: Context): PendingIntent {
        val intent = Intent(context, MainActivity::class.java).apply {
            action = ACTION_CONNECT
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
        }
        val pendingIntent = PendingIntent.getActivity(
            context,
            41,
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        return pendingIntent
    }

    fun requestConnect(context: Context) {
        runCatching { connectPendingIntent(context).send() }
    }
}
