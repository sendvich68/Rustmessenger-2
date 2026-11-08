#!/usr/bin/env bash
# ============================================
#  build-apk.sh — сборка APK для Rust Messenger
#  Требуется: Rust, cargo-ndk, Android SDK+NDK,
#             Java JDK 17+, Gradle
# ============================================
set -e

APP_NAME="RustMessenger"
PKG_ID="com.example.rustmessenger"
VERSION="1.0.0"
NDK_VERSION="25.2.9519653"
MIN_SDK=24
TARGET_SDK=33
TARGET_ABI="arm64-v8a"   # можно: armeabi-v7a x86_64

# ---------- Проверки ----------
echo "🔍 Проверка окружения..."
command -v cargo >/dev/null || { echo "❌ cargo не найден"; exit 1; }
command -v cargo-ndk >/dev/null || { echo "📦 Устанавливаю cargo-ndk..."; cargo install cargo-ndk; }

if [ -z "$ANDROID_HOME" ]; then
    echo "❌ ANDROID_HOME не задан. Укажи путь к Android SDK."
    echo "   export ANDROID_HOME=\$HOME/Android/Sdk"
    exit 1
fi
if [ -z "$ANDROID_NDK_HOME" ]; then
    export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/$NDK_VERSION"
fi

echo "✅ ANDROID_HOME=$ANDROID_HOME"
echo "✅ NDK=$ANDROID_NDK_HOME"

# ---------- Добавляем Android-таргет ----------
echo "🎯 Добавляю rust target aarch64-linux-android..."
rustup target add aarch64-linux-android

# ---------- Компилируем нативный бинарник ----------
echo "🦀 Собираю Rust-бинарник для Android ($TARGET_ABI)..."
cargo ndk -t "$TARGET_ABI" -o ./android/app/src/main/jniLibs build --release

# ---------- Генерируем Android-проект ----------
echo "📱 Генерирую Android-проект..."
mkdir -p android/app/src/main/java/$(echo "$PKG_ID" | tr '.' '/')
mkdir -p android/app/src/main/jniLibs
mkdir -p android/gradle/wrapper

# --- build.gradle (корень) ---
cat > android/settings.gradle <<EOF
rootProject.name = "$APP_NAME"
include ':app'
EOF

cat > android/build.gradle <<EOF
buildscript {
    repositories { google(); mavenCentral() }
    dependencies {
        classpath 'com.android.tools.build:gradle:8.1.4'
    }
}
allprojects {
    repositories { google(); mavenCentral() }
}
EOF

cat > android/gradle.properties <<EOF
org.gradle.jvmargs=-Xmx2048m
android.useAndroidX=true
EOF

# --- app/build.gradle ---
cat > android/app/build.gradle <<EOF
plugins { id 'com.android.application' }

android {
    namespace '$PKG_ID'
    compileSdk $TARGET_SDK

    defaultConfig {
        applicationId "$PKG_ID"
        minSdk $MIN_SDK
        targetSdk $TARGET_SDK
        versionCode 1
        versionName "$VERSION"
        ndk {
            abiFilters '$TARGET_ABI'
        }
    }
    buildTypes {
        release {
            minifyEnabled false
        }
    }
    sourceSets {
        main {
            jniLibs.srcDirs = ['src/main/jniLibs']
        }
    }
}
EOF

# --- AndroidManifest.xml ---
cat > android/app/src/main/AndroidManifest.xml <<EOF
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    <uses-permission android:name="android.permission.INTERNET"/>
    <uses-permission android:name="android.permission.ACCESS_NETWORK_STATE"/>
    <application
        android:label="$APP_NAME"
        android:theme="@android:style/Theme.Material.Light.NoActionBar">
        <activity android:name=".MainActivity"
            android:exported="true"
            android:configChanges="orientation|keyboardHidden|screenSize">
            <intent-filter>
                <action android:name="android.intent.action.MAIN"/>
                <category android:name="android.intent.category.LAUNCHER"/>
            </intent-filter>
        </activity>
    </application>
</manifest>
EOF

# --- MainActivity.java (запуск Rust-бинарника в процессе) ---
cat > android/app/src/main/java/$(echo "$PKG_ID" | tr '.' '/')/MainActivity.java <<'JAVA'
package com.example.rustmessenger;

import android.app.Activity;
import android.os.Bundle;
import android.widget.TextView;
import android.widget.ScrollView;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.view.Gravity;
import java.io.*;
import java.util.concurrent.atomic.AtomicBoolean;

public class MainActivity extends Activity {
    static {
        // подгружаем нашу .so (имя = lib + cargo-ndk output)
        System.loadLibrary("rust_messenger");
    }

    private EditText input;
    private TextView output;
    private ScrollView scroll;
    private Process proc;
    private BufferedWriter procIn;
    private final AtomicBoolean alive = new AtomicBoolean(true);

    @Override
    protected void onCreate(Bundle b) {
        super.onCreate(b);

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(24, 24, 24, 24);

        output = new TextView(this);
        output.setTextSize(14);
        output.setText("Запуск Rust Messenger...\n");

        scroll = new ScrollView(this);
        scroll.addView(output);
        scroll.setLayoutParams(new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f));

        input = new EditText(this);
        input.setHint("Введи сообщение и нажми Enter");
        input.setSingleLine(true);

        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT);
        input.setLayoutParams(lp);

        root.addView(scroll);
        root.addView(input);
        setContentView(root);

        startRustBinary();

        input.setOnEditorActionListener((v, actionId, ev) -> {
            String text = input.getText().toString();
            if (!text.isEmpty() && procIn != null) {
                try {
                    procIn.write(text + "\n");
                    procIn.flush();
                    input.setText("");
                } catch (IOException e) {
                    append("Ошибка отправки: " + e.getMessage());
                }
            }
            return true;
        });
    }

    private void startRustBinary() {
        try {
            // Библиотека лежит в nativeLibraryDir; cargo-ndk даёт lib*.so,
            // но для запуска как отдельного процесса лучше использовать
            // сам бинарник, положенный в assets (см. ниже build.gradle).
            File bin = new File(getFilesDir(), "rust_messenger");
            if (!bin.exists()) {
                InputStream is = getAssets().open("rust_messenger");
                FileOutputStream os = new FileOutputStream(bin);
                byte[] buf = new byte[8192];
                int n;
                while ((n = is.read(buf)) > 0) os.write(buf, 0, n);
                os.close(); is.close();
                bin.setExecutable(true);
            }
            ProcessBuilder pb = new ProcessBuilder(bin.getAbsolutePath());
            pb.redirectErrorStream(true);
            proc = pb.start();
            procIn = new BufferedWriter(new OutputStreamWriter(proc.getOutputStream()));

            // Чтение stdout в отдельном потоке
            new Thread(() -> {
                try (BufferedReader r = new BufferedReader(
                        new InputStreamReader(proc.getInputStream()))) {
                    String line;
                    while ((line = r.readLine()) != null) {
                        final String l = line;
                        runOnUiThread(() -> append(l));
                    }
                } catch (IOException ignored) {}
                runOnUiThread(() -> append("--- процесс завершён ---"));
            }).start();

        } catch (Exception e) {
            append("Ошибка запуска: " + e.getMessage());
        }
    }

    private void append(String s) {
        output.append(s + "\n");
        scroll.post(() -> scroll.fullScroll(ScrollView.FOCUS_DOWN));
    }

    @Override
    protected void onDestroy() {
        super.onDestroy();
        alive.set(false);
        if (proc != null) proc.destroy();
    }
}
JAVA

# ---------- Копируем бинарник в assets ----------
echo "📦 Копирую бинарник в assets..."
mkdir -p android/app/src/main/assets
cp "target/aarch64-linux-android/release/rust_messenger" \
   android/app/src/main/assets/rust_messenger
chmod +x android/app/src/main/assets/rust_messenger

# ---------- Gradle wrapper ----------
echo "⚙️  Настраиваю gradle wrapper..."
cat > android/gradle/wrapper/gradle-wrapper.properties <<EOF
distributionUrl=https\://services.gradle.org/distributions/gradle-8.2-bin.zip
EOF

if command -v gradle >/dev/null; then
    (cd android && gradle wrapper --gradle-version 8.2 --quiet)
else
    echo "⚠️  gradle не установлен. Скачай gradle-8.2 или установи через SDK."
fi

# ---------- Сборка APK ----------
echo "🔨 Собираю APK..."
cd android
if [ -f ./gradlew ]; then
    ./gradlew assembleRelease
else
    gradle assembleRelease
fi
cd ..

APK_PATH="android/app/build/outputs/apk/release/app-release.apk"
if [ -f "$APK_PATH" ]; then
    echo ""
    echo "✅ APK готов: $APK_PATH"
    echo "   Размер: $(du -h "$APK_PATH" | cut -f1)"
    echo "   Установка: adb install -r $APK_PATH"
else
    echo "❌ APK не найден. Проверь вывод gradle."
    exit 1
fi
