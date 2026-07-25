package com.unigate.app

import android.app.Activity
import android.content.Intent
import android.net.VpnService
import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import com.unigate.app.vpn.VpnQuickControl

class MainActivity : TauriActivity() {
  companion object {
    private const val QUICK_VPN_PERMISSION = 614
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    handleQuickAction(intent)
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    setIntent(intent)
    handleQuickAction(intent)
  }

  override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
    super.onActivityResult(requestCode, resultCode, data)
    if (requestCode == QUICK_VPN_PERMISSION && resultCode == Activity.RESULT_OK) {
      VpnQuickControl.connect(this)
    }
  }

  private fun handleQuickAction(source: Intent?) {
    when (source?.action) {
      VpnQuickControl.ACTION_CONNECT -> {
        source.action = null
        if (!VpnQuickControl.hasSavedConnection(this)) return
        val permission = VpnService.prepare(this)
        if (permission == null) {
          VpnQuickControl.connect(this)
        } else {
          startActivityForResult(permission, QUICK_VPN_PERMISSION)
        }
      }
      VpnQuickControl.ACTION_DISCONNECT -> {
        source.action = null
        VpnQuickControl.disconnect(this)
      }
    }
  }
}
