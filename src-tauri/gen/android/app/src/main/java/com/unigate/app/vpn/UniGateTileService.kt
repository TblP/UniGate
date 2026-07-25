package com.unigate.app.vpn

import android.graphics.drawable.Icon
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import com.unigate.app.R

class UniGateTileService : TileService() {
    override fun onStartListening() {
        super.onStartListening()
        updateTile()
    }

    override fun onClick() {
        super.onClick()
        if (VpnQuickControl.isRunning(this)) {
            VpnQuickControl.disconnect(this)
        } else if (!VpnQuickControl.connect(this)) {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                startActivityAndCollapse(VpnQuickControl.connectPendingIntent(this))
            } else {
                @Suppress("DEPRECATION")
                startActivityAndCollapse(
                    android.content.Intent(this, com.unigate.app.MainActivity::class.java).apply {
                        action = VpnQuickControl.ACTION_CONNECT
                        flags = android.content.Intent.FLAG_ACTIVITY_NEW_TASK
                    },
                )
            }
        }
        updateTile()
    }

    private fun updateTile() {
        val tile = qsTile ?: return
        val state = VpnQuickControl.state(this)
        tile.state = when (state) {
            VpnQuickControl.State.OFF -> Tile.STATE_INACTIVE
            VpnQuickControl.State.CONNECTING -> Tile.STATE_UNAVAILABLE
            VpnQuickControl.State.ON -> Tile.STATE_ACTIVE
        }
        tile.label = getString(R.string.tile_label)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            tile.subtitle = getString(
                when (state) {
                    VpnQuickControl.State.OFF -> R.string.widget_off
                    VpnQuickControl.State.CONNECTING -> R.string.widget_connecting
                    VpnQuickControl.State.ON -> R.string.widget_on
                },
            )
        }
        tile.icon = Icon.createWithResource(this, R.drawable.ic_tile_vpn)
        tile.updateTile()
    }
}
