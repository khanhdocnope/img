#!/usr/bin/env python3
"""Chạy SAU `cargo tauri android init`: thêm Foreground Service + wake lock
để bot tiếp tục chạy khi người dùng thoát app."""
import pathlib, re, sys

root = pathlib.Path("src-tauri/gen/android/app/src/main")
manifest = root / "AndroidManifest.xml"
main_act = next(root.rglob("MainActivity.kt"))
pkg = re.search(r"^package\s+([\w.]+)", main_act.read_text(), re.M).group(1)
kt_dir = main_act.parent

# ---------- 1. Manifest ----------
m = manifest.read_text(encoding="utf-8")
perms = [
    "android.permission.INTERNET",
    "android.permission.FOREGROUND_SERVICE",
    "android.permission.FOREGROUND_SERVICE_SPECIAL_USE",
    "android.permission.WAKE_LOCK",
    "android.permission.POST_NOTIFICATIONS",
    "android.permission.REQUEST_IGNORE_BATTERY_OPTIMIZATIONS",
]
add = "".join(
    f'    <uses-permission android:name="{p}" />\n' for p in perms if p not in m
)
m = m.replace("<application", add + "    <application", 1)
if "ReactorService" not in m:
    service = f'''
        <service
            android:name=".ReactorService"
            android:exported="false"
            android:foregroundServiceType="specialUse">
            <property
                android:name="android.app.PROPERTY_SPECIAL_USE_FGS_SUBTYPE"
                android:value="Tự động thả reaction Discord liên tục ở nền" />
        </service>
'''
    m = m.replace("</application>", service + "    </application>", 1)
manifest.write_text(m, encoding="utf-8")

# ---------- 2. ReactorService.kt ----------
(kt_dir / "ReactorService.kt").write_text(f'''package {pkg}

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.os.PowerManager

class ReactorService : Service() {{
    private var wakeLock: PowerManager.WakeLock? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {{
        val channelId = "reactor_bg"
        val nm = getSystemService(NOTIFICATION_SERVICE) as NotificationManager
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {{
            nm.createNotificationChannel(
                NotificationChannel(channelId, "Discord Reactor", NotificationManager.IMPORTANCE_LOW)
            )
        }}

        val open = packageManager.getLaunchIntentForPackage(packageName)
        val pi = PendingIntent.getActivity(
            this, 0, open, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )
        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O)
            Notification.Builder(this, channelId) else Notification.Builder(this)
        val notification = builder
            .setContentTitle("Discord Reactor đang chạy")
            .setContentText("Bot đang thả reaction ở nền. Chạm để mở app.")
            .setSmallIcon(android.R.drawable.stat_notify_sync)
            .setContentIntent(pi)
            .setOngoing(true)
            .build()

        if (Build.VERSION.SDK_INT >= 34) {{
            startForeground(1, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        }} else {{
            startForeground(1, notification)
        }}

        if (wakeLock == null) {{
            val pm = getSystemService(POWER_SERVICE) as PowerManager
            wakeLock = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "DiscordReactor::bg").apply {{
                setReferenceCounted(false)
                acquire()
            }}
        }}
        return START_STICKY
    }}

    override fun onDestroy() {{
        wakeLock?.let {{ if (it.isHeld) it.release() }}
        wakeLock = null
        super.onDestroy()
    }}
}}
''', encoding="utf-8")

# ---------- 3. AndroidBridge.kt ----------
(kt_dir / "AndroidBridge.kt").write_text(f'''package {pkg}

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.provider.Settings
import android.webkit.JavascriptInterface

class AndroidBridge(private val activity: Activity) {{
    @JavascriptInterface
    fun startService() {{
        activity.runOnUiThread {{
            if (Build.VERSION.SDK_INT >= 33 &&
                activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)
                != PackageManager.PERMISSION_GRANTED
            ) {{
                activity.requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1001)
            }}
            val intent = Intent(activity, ReactorService::class.java)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O)
                activity.startForegroundService(intent) else activity.startService(intent)

            // Xin miễn tối ưu pin (hỏi 1 lần) để Android không cắt mạng khi tắt màn hình
            val pm = activity.getSystemService(Activity.POWER_SERVICE) as PowerManager
            if (!pm.isIgnoringBatteryOptimizations(activity.packageName)) {{
                try {{
                    activity.startActivity(
                        Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS)
                            .setData(Uri.parse("package:" + activity.packageName))
                    )
                }} catch (_: Exception) {{}}
            }}
        }}
    }}

    @JavascriptInterface
    fun stopService() {{
        activity.runOnUiThread {{
            activity.stopService(Intent(activity, ReactorService::class.java))
        }}
    }}
}}
''', encoding="utf-8")

# ---------- 4. MainActivity.kt ----------
t = main_act.read_text(encoding="utf-8")
if "AndroidBridge" not in t:
    t = re.sub(r"(^package\s+[\w.]+\s*\n)", r"\1\nimport android.webkit.WebView\n", t, count=1, flags=re.M)
    hook = '''
    override fun onWebViewCreate(webView: WebView) {
        super.onWebViewCreate(webView)
        webView.addJavascriptInterface(AndroidBridge(this), "AndroidBridge")
    }
'''
    if re.search(r"TauriActivity\(\)\s*\{", t):
        t = re.sub(r"(TauriActivity\(\)\s*\{)", r"\1" + hook.replace("\\", "\\\\"), t, count=1)
    else:
        t = re.sub(r"TauriActivity\(\)", "TauriActivity() {" + hook + "}", t, count=1)
    main_act.write_text(t, encoding="utf-8")

print("Đã patch Android project, package:", pkg)
