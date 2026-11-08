cat > android/app/src/main/java/$PKG_PATH/MainActivity.java <<'JAVA'
package com.example.rustmessenger;

import android.app.Activity;
import android.os.Bundle;
import android.widget.*;
import android.view.View;
import java.io.*;

public class MainActivity extends Activity {
    static { System.loadLibrary("rust_messenger"); }

    // JNI методы
    public native void init_rust();
    public native void start_server(int port);
    public native void connect_to_server(String ip, int port);
    public native void send_message(String msg);
    public native void set_callback(Callback cb);

    public interface Callback { void onMessage(String msg); }

    private LinearLayout layoutStart, layoutChat;
    private TextView chatOutput;
    private ScrollView chatScroll;
    private EditText inputMessage;

    @Override
    protected void onCreate(Bundle b) {
        super.onCreate(b);
        init_rust();

        // Передаем Java-интерфейс в Rust (Callback)
        set_callback(msg -> runOnUiThread(() -> appendChat(msg)));

        // --- Главный экран (Выбор роли) ---
        layoutStart = new LinearLayout(this);
        layoutStart.setOrientation(LinearLayout.VERTICAL);
        layoutStart.setPadding(30, 100, 30, 30);
        layoutStart.setGravity(android.view.Gravity.CENTER_HORIZONTAL);

        TextView title = new TextView(this);
        title.setText("Rust Messenger");
        title.setTextSize(28);
        layoutStart.addView(title);

        Button btnServer = new Button(this);
        btnServer.setText("Создать сервер (Порт 8080)");
        btnServer.setOnClickListener(v -> {
            start_server(8080);
            showChat();
        });
        layoutStart.addView(btnServer);

        EditText ipInput = new EditText(this);
        ipInput.setHint("IP сервера (например, 192.168.1.5)");
        layoutStart.addView(ipInput);

        Button btnClient = new Button(this);
        btnClient.setText("Подключиться");
        btnClient.setOnClickListener(v -> {
            String ip = ipInput.getText().toString();
            if (!ip.isEmpty()) {
                connect_to_server(ip, 8080);
                showChat();
            }
        });
        layoutStart.addView(btnClient);

        // --- Экран чата ---
        layoutChat = new LinearLayout(this);
        layoutChat.setOrientation(LinearLayout.VERTICAL);

        chatOutput = new TextView(this);
        chatOutput.setTextSize(16);
        chatOutput.setPadding(20, 20, 20, 20);

        chatScroll = new ScrollView(this);
        chatScroll.addView(chatOutput);
        chatScroll.setLayoutParams(new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f));

        inputMessage = new EditText(this);
        inputMessage.setHint("Введи сообщение...");
        inputMessage.setSingleLine(true);

        Button btnSend = new Button(this);
        btnSend.setText("Отправить");
        btnSend.setOnClickListener(v -> sendChat());

        LinearLayout bottomBar = new LinearLayout(this);
        bottomBar.setOrientation(LinearLayout.HORIZONTAL);
        bottomBar.addView(inputMessage, new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f));
        bottomBar.addView(btnSend);

        layoutChat.addView(chatScroll);
        layoutChat.addView(bottomBar);

        setContentView(layoutStart);
    }

    private void sendChat() {
        String text = inputMessage.getText().toString();
        if (!text.isEmpty()) {
            send_message(text);
            appendChat("Вы: " + text);
            inputMessage.setText("");
        }
    }

    private void appendChat(String msg) {
        chatOutput.append(msg + "\n");
        chatScroll.post(() -> chatScroll.fullScroll(ScrollView.FOCUS_DOWN));
    }

    private void showChat() {
        runOnUiThread(() -> setContentView(layoutChat));
    }
}
JAVA
