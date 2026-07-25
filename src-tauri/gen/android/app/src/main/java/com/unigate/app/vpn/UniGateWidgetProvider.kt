package com.unigate.app.vpn

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.widget.RemoteViews
import com.unigate.app.R

class UniGateWidgetProvider : AppWidgetProvider() {
    override fun onUpdate(
        context: Context,
        appWidgetManager: AppWidgetManager,
        appWidgetIds: IntArray,
    ) {
        appWidgetIds.forEach { appWidgetManager.updateAppWidget(it, views(context)) }
    }

    override fun onReceive(context: Context, intent: Intent) {
        super.onReceive(context, intent)
        if (intent.action == VpnQuickControl.ACTION_TOGGLE) {
            VpnQuickControl.toggle(context)
            updateAll(context)
        }
    }

    companion object {
        fun updateAll(context: Context) {
            val manager = AppWidgetManager.getInstance(context)
            val component = ComponentName(context, UniGateWidgetProvider::class.java)
            manager.updateAppWidget(component, views(context))
        }

        private fun views(context: Context): RemoteViews {
            val state = VpnQuickControl.state(context)
            val intent = Intent(context, UniGateWidgetProvider::class.java).apply {
                action = VpnQuickControl.ACTION_TOGGLE
            }
            val pendingIntent = PendingIntent.getBroadcast(
                context,
                42,
                intent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
            return RemoteViews(context.packageName, R.layout.widget_unigate).apply {
                setInt(
                    R.id.widget_root,
                    "setBackgroundResource",
                    when (state) {
                        VpnQuickControl.State.OFF -> R.drawable.widget_background_off
                        VpnQuickControl.State.CONNECTING -> R.drawable.widget_background_connecting
                        VpnQuickControl.State.ON -> R.drawable.widget_background_on
                    },
                )
                setImageViewResource(
                    R.id.widget_power,
                    when (state) {
                        VpnQuickControl.State.OFF -> R.drawable.ic_widget_power_off
                        VpnQuickControl.State.CONNECTING -> R.drawable.ic_widget_connecting
                        VpnQuickControl.State.ON -> R.drawable.ic_widget_power_on
                    },
                )
                setTextViewText(
                    R.id.widget_status,
                    context.getString(
                        when (state) {
                            VpnQuickControl.State.OFF -> R.string.widget_off
                            VpnQuickControl.State.CONNECTING -> R.string.widget_connecting
                            VpnQuickControl.State.ON -> R.string.widget_on
                        },
                    ),
                )
                setTextColor(
                    R.id.widget_status,
                    context.getColor(
                        if (state == VpnQuickControl.State.ON) R.color.widget_text_on
                        else R.color.widget_text_off,
                    ),
                )
                setOnClickPendingIntent(R.id.widget_root, pendingIntent)
            }
        }
    }
}
