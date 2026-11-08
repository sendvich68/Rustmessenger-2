use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

// ==================== ТИПЫ ====================
type SharedStream = Arc<Mutex<TcpStream>>;
type Clients = Arc<Mutex<HashMap<String, SharedStream>>>;

// Файлы для сохранения данных
const HISTORY_FILE: &str = "chat_history.log";
const USERS_FILE: &str = "users.log";
const HOST_NICK_FILE: &str = "host_nick.txt";

// ==================== УТИЛИТЫ ====================
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
    // Простое представление: дней с эпохи + время
    let days = secs / 86400;
    let rem = secs % 86400;
    let h = rem / 3600;
    let m = (rem / 60) % 60;
    let s = rem % 60;
    format!("day{} {:02}:{:02}:{:02}", days, h, m, s)
}

/// Записать строку в файл истории (append)
fn append_history(line: &str) {
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(HISTORY_FILE)
    {
        let _ = writeln!(f, "[{}] {}", full_datetime(), line);
    }
}

/// Сохранить список пользователей (перезапись)
fn save_users(names: &[String]) {
    if let Ok(mut f) = fs::File::create(USERS_FILE) {
        let _ = writeln!(f, "# Список пользователей (обновлено {})", full_datetime());
        for n in names {
            let _ = writeln!(f, "{}", n);
        }
    }
}

/// Загрузить список пользователей (не используется для входа, но полезно)
#[allow(dead_code)]
fn load_users() -> Vec<String> {
    let mut v = Vec::new();
    if let Ok(content) = fs::read_to_string(USERS_FILE) {
        for line in content.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                v.push(line.to_string());
            }
        }
    }
    v
}

/// Сохранить ник хоста
fn save_host_nick(nick: &str) {
    if let Ok(mut f) = fs::File::create(HOST_NICK_FILE) {
        let _ = writeln!(f, "{}", nick);
    }
}

/// Загрузить ник хоста
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

// ==================== MAIN ====================
fn main() {
    print!("\x1b[2J\x1b[1;1H");
    println!("\x1b[1;36m╔══════════════════════════════════════════╗");
    println!("║       RUST MESSENGER v2.2 🦀             ║");
    println!("║   Сервер + Клиент + Сохранение данных    ║");
    println!("╚══════════════════════════════════════════╝\x1b[0m");

    if let Some(saved) = load_host_nick() {
        println!("\x1b[33mСохранённый ник хоста: {}\x1b[0m", saved);
    }
    println!("1. Создать сервер (хост)");
    println!("2. Подключиться к серверу (клиент)");
    println!("3. Показать историю чата");
    println!("4. Очистить историю чата");
    print!("\nВыбери (1-4): ");
    io::stdout().flush().unwrap();

    let mut choice = String::new();
    io::stdin().read_line(&mut choice).unwrap();

    match choice.trim() {
        "1" => run_server(),
        "2" => run_client(),
        "3" => show_history(),
        "4" => clear_history(),
        _ => println!("Неверный выбор."),
    }
}

// ==================== СЕРВЕР ====================
fn run_server() {
    let listener = TcpListener::bind("0.0.0.0:8080").expect("Не удалось запустить сервер");
    println!("\n\x1b[32m✅ Сервер запущен на порту 8080!\x1b[0m");
    println!("Твой IP-адрес для подключения:");
    if let Ok(ip) = local_ip() {
        println!("\x1b[1;33m👉 {}\x1b[0m", ip);
    } else {
        println!("(Узнай IP через настройки Wi-Fi / ipconfig)");
    }
    println!("\nФайлы:");
    println!("  📜 История: {}", HISTORY_FILE);
    println!("  👥 Пользователи: {}", USERS_FILE);
    println!("  🏷  Ник хоста: {}", HOST_NICK_FILE);
    println!("\nЖдем подключения клиентов... (Ctrl+C для выхода)\n");

    append_history("=== СЕРВЕР ЗАПУЩЕН ===");

    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let addr = match stream.peer_addr() {
                    Ok(a) => a.to_string(),
                    Err(_) => continue,
                };
                let clients_clone = Arc::clone(&clients);
                thread::spawn(move || {
                    handle_client(stream, addr, clients_clone);
                });
            }
            Err(e) => eprintln!("Ошибка подключения: {}", e),
        }
    }
}

fn handle_client(stream: TcpStream, addr: String, clients: Clients) {
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });

    let stream_arc: SharedStream = Arc::new(Mutex::new(stream));

    // Запрос никнейма
    {
        let mut s = stream_arc.lock().unwrap();
        let _ = writeln!(s, "\x1b[36mВведи свой ник:\x1b[0m");
        let _ = s.flush();
    }

    let mut nickname = String::new();
    if reader.read_line(&mut nickname).is_err() {
        return;
    }
    let nickname = nickname.trim().to_string();
    if nickname.is_empty() {
        return;
    }

    // Проверка на уникальность
    {
        let c = clients.lock().unwrap();
        if c.contains_key(&nickname) {
            let mut s = stream_arc.lock().unwrap();
            let _ = writeln!(s, "\x1b[31mНик занят. Переподключись.\x1b[0m");
            let _ = s.flush();
            return;
        }
    }

    println!("\x1b[32m✅ Подключился:\x1b[0m {} ({})", nickname, addr);
    append_history(&format!("[+] {} подключился ({})", nickname, addr));

    // Регистрация
    clients
        .lock()
        .unwrap()
        .insert(nickname.clone(), Arc::clone(&stream_arc));

    // Сохраняем список пользователей
    {
        let names: Vec<String> = clients.lock().unwrap().keys().cloned().collect();
        save_users(&names);
    }

    // Приветствие
    {
        let mut s = stream_arc.lock().unwrap();
        let _ = writeln!(s, "\x1b[1;32m╔══════════════════════════════════════╗\x1b[0m");
        let _ = writeln!(s, "\x1b[1;32m║      Добро пожаловать, {}!\x1b[0m", nickname);
        let _ = writeln!(s, "\x1b[1;32m╚══════════════════════════════════════╝\x1b[0m");
        let _ = writeln!(s, "Команды:");
        let _ = writeln!(s, "  /help - справка");
        let _ = writeln!(s, "  /list - кто онлайн");
        let _ = writeln!(s, "  /history [N] - последние N сообщений (по умолч. 10)");
        let _ = writeln!(s, "  /w ник сообщение - приватно");
        let _ = writeln!(s, "  /exit - выход");
        let _ = writeln!(s, "─────────────────────────────────────");
        let _ = s.flush();
    }

    // Уведомить остальных
    broadcast(
        &clients,
        &nickname,
        &format!("\x1b[33m[+] {} присоединился к чату\x1b[0m", nickname),
    );

    // Цикл чтения
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let msg = line.trim();
        if msg.is_empty() {
            continue;
        }
        if msg == "/exit" {
            break;
        }
        if msg == "/help" {
            let mut s = stream_arc.lock().unwrap();
            let _ = writeln!(s, "\x1b[36m/help — справка\x1b[0m");
            let _ = writeln!(s, "\x1b[36m/list — список онлайн\x1b[0m");
            let _ = writeln!(s, "\x1b[36m/history [N] — последние N сообщений\x1b[0m");
            let _ = writeln!(s, "\x1b[36m/w ник сообщение — приватно\x1b[0m");
            let _ = writeln!(s, "\x1b[36m/exit — выйти\x1b[0m");
            let _ = s.flush();
            continue;
        }
        if msg == "/list" {
            let names: Vec<String> = clients.lock().unwrap().keys().cloned().collect();
            let mut s = stream_arc.lock().unwrap();
            let _ = writeln!(s, "\x1b[36mОнлайн ({}):\x1b[0m", names.len());
            for n in names {
                let _ = writeln!(s, "  • {}", n);
            }
            let _ = s.flush();
            continue;
        }

        // /history [N] — показать последние N строк
        if msg == "/history" || msg.starts_with("/history ") {
            let n: usize = msg
                .strip_prefix("/history ")
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(10);
            let mut s = stream_arc.lock().unwrap();
            let _ = writeln!(s, "\x1b[36m─── Последние {} сообщений ───\x1b[0m", n);
            if let Ok(content) = fs::read_to_string(HISTORY_FILE) {
                let lines: Vec<&str> = content.lines().collect();
                let start = lines.len().saturating_sub(n);
                for l in &lines[start..] {
                    let _ = writeln!(s, "  {}", l);
                }
            } else {
                let _ = writeln!(s, "  (история пуста)");
            }
            let _ = writeln!(s, "\x1b[36m─────────────────────────────\x1b[0m");
            let _ = s.flush();
            continue;
        }

        // Приватное сообщение
        if msg.starts_with("/w ") {
            let parts: Vec<&str> = msg.splitn(3, ' ').collect();
            if parts.len() < 3 {
                let mut s = stream_arc.lock().unwrap();
                let _ = writeln!(s, "\x1b[31mИспользование: /w ник сообщение\x1b[0m");
                let _ = s.flush();
                continue;
            }
            let target = parts[1];
            let text = parts[2];

            let target_stream_opt = {
                let c = clients.lock().unwrap();
                c.get(target).cloned()
            };

            if let Some(target_stream) = target_stream_opt {
                {
                    let mut ts = target_stream.lock().unwrap();
                    let _ = writeln!(
                        ts,
                        "\x1b[35m[{}] {} → вы: {}\x1b[0m",
                        timestamp(),
                        nickname,
                        text
                    );
                    let _ = ts.flush();
                }
                let mut s = stream_arc.lock().unwrap();
                let _ = writeln!(
                    s,
                    "\x1b[35m[{}] вы → {}: {}\x1b[0m",
                    timestamp(),
                    target,
                    text
                );
                let _ = s.flush();
                append_history(&format!("[PM] {} → {}: {}", nickname, target, text));
            } else {
                let mut s = stream_arc.lock().unwrap();
                let _ = writeln!(s, "\x1b[31mПользователь {} не найден\x1b[0m", target);
                let _ = s.flush();
            }
            continue;
        }

        // Обычное сообщение — всем
        let full = format!("\x1b[35m[{}] {}:\x1b[0m {}", timestamp(), nickname, msg);
        broadcast_all(&clients, &full);
        append_history(&format!("{}: {}", nickname, msg));
    }

    // Отключение
    clients.lock().unwrap().remove(&nickname);
    println!("\x1b[31m❌ Отключился: {}\x1b[0m", nickname);
    append_history(&format!("[-] {} отключился", nickname));
    broadcast(
        &clients,
        &nickname,
        &format!("\x1b[33m[-] {} покинул чат\x1b[0m", nickname),
    );

    // Обновить файл пользователей
    {
        let names: Vec<String> = clients.lock().unwrap().keys().cloned().collect();
        save_users(&names);
    }
}

fn broadcast(clients: &Clients, except: &str, msg: &str) {
    let targets: Vec<SharedStream> = {
        let c = clients.lock().unwrap();
        c.iter()
            .filter(|(name, _)| name.as_str() != except)
            .map(|(_, s)| Arc::clone(s))
            .collect()
    };
    for stream in targets {
        if let Ok(mut s) = stream.lock() {
            let _ = writeln!(s, "{}", msg);
            let _ = s.flush();
        }
    }
}

fn broadcast_all(clients: &Clients, msg: &str) {
    let targets: Vec<SharedStream> = {
        let c = clients.lock().unwrap();
        c.values().map(Arc::clone).collect()
    };
    for stream in targets {
        if let Ok(mut s) = stream.lock() {
            let _ = writeln!(s, "{}", msg);
            let _ = s.flush();
        }
    }
}

// ==================== КЛИЕНТ ====================
fn run_client() {
    print!("\nВведи IP сервера (например, 192.168.1.5, или 127.0.0.1): ");
    io::stdout().flush().unwrap();
    let mut ip = String::new();
    io::stdin().read_line(&mut ip).unwrap();
    let ip = ip.trim().to_string();

    // Пробуем загрузить сохранённый ник
    let saved_nick = load_host_nick();
    if let Some(ref n) = saved_nick {
        print!("Введи свой ник [{}]: ", n);
    } else {
        print!("Введи свой ник: ");
    }
    io::stdout().flush().unwrap();
    let mut nick = String::new();
    io::stdin().read_line(&mut nick).unwrap();
    let nick_input = nick.trim().to_string();
    let nick = if nick_input.is_empty() {
        saved_nick.unwrap_or_default()
    } else {
        nick_input
    };
    if nick.is_empty() {
        println!("\x1b[31mНик не может быть пустым.\x1b[0m");
        return;
    }

    // Сохраняем ник (общий файл — как «последний использованный»)
    save_host_nick(&nick);

    let address = format!("{}:8080", ip);
    println!("Подключаемся к {}...", address);

    let stream = match TcpStream::connect(&address) {
        Ok(s) => s,
        Err(e) => {
            println!("\x1b[31m❌ Не удалось подключиться: {}\x1b[0m", e);
            println!("Проверь IP, Wi-Fi и брандмауэр (порт 8080).");
            return;
        }
    };

    println!("\x1b[32m✅ Подключено к серверу!\x1b[0m\n");

    // Отправляем ник первым сообщением
    {
        let mut s = &stream;
        if writeln!(s, "{}", nick).is_err() {
            println!("\x1b[31mОшибка отправки ника.\x1b[0m");
            return;
        }
        let _ = s.flush();
    }

    let read_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            println!("\x1b[31mОшибка клонирования сокета: {}\x1b[0m", e);
            return;
        }
    };
    let write_stream: SharedStream = Arc::new(Mutex::new(stream));

    // Поток чтения
    let reader = BufReader::new(read_stream);
    thread::spawn(move || {
        for line in reader.lines() {
            match line {
                Ok(msg) => {
                    println!("\r{}\x1b[K", msg);
                    print!("\x1b[32mВы ❯ \x1b[0m");
                    let _ = io::stdout().flush();
                }
                Err(_) => {
                    println!("\r\x1b[31mСоединение с сервером потеряно.\x1b[0m");
                    std::process::exit(0);
                }
            }
        }
        println!("\r\x1b[31mСервер закрыл соединение.\x1b[0m");
        std::process::exit(0);
    });

    // Цикл ввода
    chat_loop(write_stream);
}

// ==================== ОБЩИЙ ЦИКЛ ЧАТА ====================
fn chat_loop(stream: SharedStream) {
    println!("Начинай писать сообщения (Enter для отправки, /help — справка):\n");
    print!("\x1b[32mВы ❯ \x1b[0m");
    io::stdout().flush().unwrap();

    loop {
        let mut msg = String::new();
        if io::stdin().read_line(&mut msg).is_err() {
            break;
        }
        let msg = msg.trim();
        if msg.is_empty() {
            print!("\x1b[32mВы ❯ \x1b[0m");
            io::stdout().flush().unwrap();
            continue;
        }

        let mut s = stream.lock().unwrap();
        if writeln!(s, "{}", msg).is_err() || s.flush().is_err() {
            println!("\x1b[31mОшибка отправки. Соединение потеряно.\x1b[0m");
            std::process::exit(0);
        }
        drop(s);

        if msg == "/exit" {
            println!("\x1b[33mВыходим...\x1b[0m");
            std::process::exit(0);
        }
    }

    std::process::exit(0);
}

// ==================== ИСТОРИЯ ====================
fn show_history() {
    println!("\n\x1b[36m─── История чата ({}) ───\x1b[0m", HISTORY_FILE);
    match fs::read_to_string(HISTORY_FILE) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            if lines.is_empty() {
                println!("(пусто)");
            } else {
                for l in &lines {
                    println!("{}", l);
                }
                println!("\x1b[36mВсего строк: {}\x1b[0m", lines.len());
            }
        }
        Err(_) => println!("(файл истории ещё не создан)"),
    }
}

fn clear_history() {
    print!("Удалить всю историю? (y/N): ");
    io::stdout().flush().unwrap();
    let mut ans = String::new();
    io::stdin().read_line(&mut ans).unwrap();
    if ans.trim().eq_ignore_ascii_case("y") {
        match fs::write(HISTORY_FILE, "") {
            Ok(_) => println!("\x1b[32m✅ История очищена.\x1b[0m"),
            Err(e) => println!("\x1b[31mОшибка: {}\x1b[0m", e),
        }
    } else {
        println!("Отменено.");
    }
}

// ==================== ПОИСК IP ====================
fn local_ip() -> io::Result<String> {
    use std::net::UdpSocket;
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("8.8.8.8:80")?;
    Ok(socket.local_addr()?.ip().to_string())
}
