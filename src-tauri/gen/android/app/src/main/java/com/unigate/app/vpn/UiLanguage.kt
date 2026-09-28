package com.unigate.app.vpn

import android.content.Context
import android.content.res.Configuration
import androidx.annotation.StringRes
import java.util.Locale

/** Language selected in UniGate settings, including surfaces outside the WebView. */
object UiLanguage {
    private const val PREFS = "unigate_ui_language"
    private const val KEY = "language"

    fun set(context: Context, value: String) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit()
            .putString(KEY, if (value == "en") "en" else "ru").apply()
    }

    fun text(context: Context, @StringRes id: Int, vararg args: Any): String {
        val language = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .getString(KEY, "ru") ?: "ru"
        val config = Configuration(context.resources.configuration)
        config.setLocale(Locale.forLanguageTag(language))
        val localized = context.createConfigurationContext(config)
        return localized.getString(id, *args)
    }
}
