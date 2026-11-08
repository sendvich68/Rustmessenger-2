use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

type SharedStream = Arc<Mutex<TcpStream>>;
type Clients = Arc<Mutex<HashMap<String, SharedStream>>>;

const HISTORY_FILE: &str = "chat_history.log";
const USERS_FILE: &str = "users.log";
const HOST_NICK_FILE: &str = "host_nick.txt";

fn timestamp() -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let secs = now.as_secs();
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

fn full_datetime() -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let secs = now.as_secs();
    let days = secs / 86400;
    let rem = secs % 86400;
    let h = rem / 3600;
    let m = (rem / 60) % 60;
    let s = rem % 60;
    format!("day{} {:02}:{:02}:{:02}", days, h, m, s)
}

fn append_history(line: &str) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(HISTORY_FILE) {
        let _ = writeln!(f, "[{}] {}", full_datetime(), line);
    }
}

fn save_users(names: &[String]) {
    if let Ok(mut f) = fs::File::create(USERS_FILE) {
        let _ = writeln!(f, "# Список пользователей (обновлено {})", full_datetime());
        for n in names {
            let _ = writeln!(f, "{}", n);
        }
    }
}

fn save_host_nick(nick: &str) {
    if let Ok(mut f) = fs::File::create(HOST_NICK_FILE) {
        let _ = writeln!(f, "{}", nick);
    }
}

fn load_host_nick() -> Option<String> {
    if Path::new(HOST_NICK_FILE).exists() {
        if let Ok(content) = fs::read_to_string(HOST_NICK_FILE) {
            let n = content.trim().to_string();
            if !n.is_empty() {
                return Some(n);
            }
        }
    }
    None
}

// === ТОЧКА ВХОДА ДЛЯ ANDROID (JNI) ===
// Мы вызываем эту функцию из Java
#[no_mangle]
pub extern "C" fn main_android() -> i32 {
    // В целях экономии запускаем сервер сразу. 
    // В идеале тут нужно передавать флаг (сервер/клиент), но для теста сгодится.
    run_server_thread(); 
    0
}

fn run_server_thread() {
    thread::spawn(|| {
        let listener = TcpListener::bind("0.0.0.0:8080").expect("Не удалось запустить сервер");
        println!("\n\x1b[32m✅ Сервер запущен на порту 8080!\x1b[0m");
        
        let clients: Clients = Arc::new(Mutex::new(HashMap::new()));
        for stream in listener.incoming() {
            if let Ok(stream) = stream {
                if let Ok(addr) = stream.peer_addr() {
                    let clients_clone = Arc::clone(&clients);
                    thread::spawn(move || {
                        handle_client(stream, addr.to_string(), clients_clone);
                    });
                }
            }
        }
    });
}

fn handle_client(stream: TcpStream, addr: String, clients: Clients) {
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });
    let stream_arc: SharedStream = Arc::new(Mutex::new(stream));

    let mut s = stream_arc.lock().unwrap();
    let _ = writeln!(s, "\x1b[36mВведи свой ник:\x1b[0m");
    let _ = s.flush();
    drop(s);

    let mut nickname = String::new();
    if reader.read_line(&mut nickname).is_err() { return; }
    let nickname = nickname.trim().to_string();
    if nickname.is_empty() { return; }

    {
        let c = clients.lock().unwrap();
        if c.contains_key(&nickname) {
            let mut s = stream_arc.lock().unwrap();
            let _ = writeln!(s, "\x1b[31mНик занят. Переподключись.\x1b[0m");
            return;
        }
    }

    println!("\x1b[32m✅ Подключился:\x1b[0m {} ({})", nickname, addr);
    append_history(&format!("[+] {} подключился ({})", nickname, addr));

    clients.lock().unwrap().insert(nickname.clone(), Arc::clone(&stream_arc));
    save_users(&clients.lock().unwrap().keys().cloned().collect::<Vec<_>>());

    {
        let mut s = stream_arc.lock().unwrap();
        let _ = writeln!(s, "\x1b[1;32m╔══════════════════════════════════════╗\x1b[0m");
        let _ = writeln!(s, "\x1b[1;32m║      Добро пожаловать, {}!\x1b[0m", nickname);
        let _ = writeln!(s, "\x1b[1;32m╚══════════════════════════════════════╝\x1b[0m");
        let _ = writeln!(s, "Команды:");
        let _ = writeln!(s, "  /help - справка");
        let _ = writeln!(s, "  /list - кто онлайн");
        let _ = writeln!(s, "  /history [N] - последние N сообщений");
        let _ = writeln!(s, "  /w ник сообщение - приватно");
        let _ = writeln!(s, "  /exit - выход");
        let _ = writeln!(s, "─────────────────────────────────────");
    }

    broadcast(&clients, &nickname, &format!("\x1b[33m[+] {} присоединился к чату\x1b[0m", nickname));

    for line in reader.lines() {
        let line = match line { Ok(l) => l, Err(_) => break };
        let msg = line.trim();
        if msg.is_empty() { continue; }
        if msg == "/exit" { break; }
        if msg == "/help" {
            let mut s = stream_arc.lock().unwrap();
            let _ = writeln!(s, "\x1b[36m/help, /list, /history [N], /w ник текст, /exit\x1b[0m");
            continue;
        }
        if msg == "/list" {
            let names: Vec<String> = clients.lock().unwrap().keys().cloned().collect();
            let mut s = stream_arc.lock().unwrap();
            let _ = writeln!(s, "\x1b[36mОнлайн ({}):\x1b[0m", names.len());
            for n in names { let _ = writeln!(s, "  • {}", n); }
            continue;
        }
        if msg == "/history" || msg.starts_with("/history ") {
            let n: usize = msg.strip_prefix("/history ").and_then(|s| s.trim().parse().ok()).unwrap_or(10);
            let mut s = stream_arc.lock().unwrap();
            let _ = writeln!(s, "\x1b[36m─── Последние {} ───\x1b[0m", n);
            if let Ok(content) = fs::read_to_string(HISTORY_FILE) {
                let lines: Vec<&str> = content.lines().collect();
                for l in &lines[lines.len().saturating_sub(n)..] { let _ = writeln!(s, "  {}", l); }
            }
            continue;
        }
        if msg.starts_with("/w ") {
            let parts: Vec<&str> = msg.splitn(3, ' ').collect();
            if parts.len() < 3 { continue; }
            let (target, text) = (parts[1], parts[2]);
            let target_stream_opt = clients.lock().unwrap().get(target).cloned();
            if let Some(target_stream) = target_stream_opt {
                let mut ts = target_stream.lock().unwrap();
                let _ = writeln!(ts, "\x1b[35m[{}] {} → вы: {}\x1b[0m", timestamp(), nickname, text);
                let mut s = stream_arc.lock().unwrap();
                let _ = writeln!(s, "\x1b[35m[{}] вы → {}: {}\x1b[0m", timestamp(), target, text);
                append_history(&format!("[PM] {} → {}: {}", nickname, target, text));
            }
            continue;
        }
        let full = format!("\x1b[35m[{}] {}:\x1b[0m {}", timestamp(), nickname, msg);
        broadcast_all(&clients, &full);
        append_history(&format!("{}: {}", nickname, msg));
    }

    clients.lock().unwrap().remove(&nickname);
    println!("\x1b[31m❌ Отключился: {}\x1b[0m", nickname);
    append_history(&format!("[-] {} отключился", nickname));
    broadcast(&clients, &nickname, &format!("\x1b[33m[-] {} покинул чат\x1b[0m", nickname));
    save_users(&clients.lock().unwrap().keys().cloned().collect::<Vec<_>>());
}

fn broadcast(clients: &Clients, except: &str, msg: &str) {
    let targets: Vec<SharedStream> = {
        let c = clients.lock().unwrap();
        c.iter().filter(|(name, _)| name.as_str() != except).map(|(_, s)| Arc::clone(s)).collect()
    };
    for stream in targets { if let Ok(mut s) = stream.lock() { let _ = writeln!(s, "{}", msg); } }
}

fn broadcast_all(clients: &Clients, msg: &str) {
    let targets: Vec<SharedStream> = {
        let c = clients.lock().unwrap();
        c.values().map(Arc::clone).collect()
    };
    for stream in targets { if let Ok(mut s) = stream.lock() { let _ = writeln!(s, "{}", msg); } }
}
