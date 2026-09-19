import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

android {
    compileSdk = 36
    namespace = "com.unigate.app"
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        applicationId = "com.unigate.app"
        minSdk = 24
        targetSdk = 36
        versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
        versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")
    }
    signingConfigs {
        create("release") {
            // Ключ подписи не хранится в репозитории: путь и пароль приходят
            // снаружи. Основной канал — свойства ORG_GRADLE_PROJECT_*: их
            // передаёт клиент Gradle, поэтому они долетают и при уже запущенном
            // демоне, в отличие от System.getenv() (тот читает окружение
            // демона). Прямые переменные оставлены как запасной вариант.
            fun setting(property: String, variable: String): String? =
                (project.findProperty(property) as String?)?.takeIf { it.isNotBlank() }
                    ?: System.getenv(variable)?.takeIf { it.isNotBlank() }

            val storePath = setting("unigateKeystore", "UNIGATE_ANDROID_KEYSTORE")
            if (storePath != null) {
                val store = setting("unigateKeystorePassword", "UNIGATE_ANDROID_KEYSTORE_PASSWORD")
                storeFile = file(storePath)
                storePassword = store
                keyAlias = setting("unigateKeyAlias", "UNIGATE_ANDROID_KEY_ALIAS") ?: "unigate"
                keyPassword = setting("unigateKeyPassword", "UNIGATE_ANDROID_KEY_PASSWORD") ?: store
            }
        }
    }
    buildTypes {
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
                jniLibs.keepDebugSymbols.add("*/armeabi-v7a/*.so")
                jniLibs.keepDebugSymbols.add("*/x86/*.so")
                jniLibs.keepDebugSymbols.add("*/x86_64/*.so")
            }
        }
        getByName("release") {
            signingConfig = signingConfigs.getByName("release")
            // Kotlin-части (VpnPlugin, сервисы, виджеты) вызываются из Tauri и
            // системы рефлексией/через манифест — R8 экономит здесь единицы
            // килобайт, но легко ломает эти вызовы. Выключено осознанно.
            isMinifyEnabled = false
            proguardFiles(
                *fileTree(".") { include("**/*.pro") }
                    .plus(getDefaultProguardFile("proguard-android-optimize.txt"))
                    .toList().toTypedArray()
            )
        }
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
    buildFeatures {
        buildConfig = true
    }
}

rust {
    rootDirRel = "../../../"
}

dependencies {
    implementation(files("libs/libbox.aar"))
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.lifecycle:lifecycle-process:2.10.0")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = "tauri.build.gradle.kts")
