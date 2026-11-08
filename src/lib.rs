use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::net::{TcpListener, TcpStream};
use std::os::raw::c_char;
use std::sync::{Arc, Mutex};
use std::thread;
use std::io::{BufRead, BufReader, Write};

// Глобальное состояние
static mut STATE: Option<Arc<Mutex<AppState>>> = None;

struct AppState {
    is_server: bool,
    server_stream: Option<TcpStream>,
    client_stream: Option<TcpStream>,
    jni_callback: Option<Box<dyn Fn(String) + Send + Sync>>,
}

// Функция для отправки сообщений в Java
fn send_to_java(state: &Arc<Mutex<AppState>>, msg: String) {
    let s = state.lock().unwrap();
    if let Some(ref cb) = s.jni_callback {
        cb(msg);
    }
}

// ---------------------------------------------------------
// JNI ФУНКЦИИ (вызываются из Java)
// ---------------------------------------------------------

#[no_mangle]
pub extern "C" fn init_rust() {
    unsafe {
        STATE = Some(Arc::new(Mutex::new(AppState {
            is_server: false,
            server_stream: None,
            client_stream: None,
            jni_callback: None,
        })));
    }
}

#[no_mangle]
pub extern "C" fn set_callback(cb: extern "C" fn(*const c_char)) {
    unsafe {
        if let Some(ref state) = STATE {
            let state_clone = Arc::clone(state);
            let cb_wrapper = move |msg: String| {
                let c_msg = CString::new(msg).unwrap_or_default();
                cb(c_msg.as_ptr());
                let _ = state_clone;
            };
            state.lock().unwrap().jni_callback = Some(Box::new(cb_wrapper));
        }
    }
}

#[no_mangle]
pub extern "C" fn start_server(port: u16) {
    unsafe {
        if let Some(ref state) = STATE {
            let state_clone = Arc::clone(state);
            state.lock().unwrap().is_server = true;
            
            thread::spawn(move || {
                let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).expect("Ошибка запуска сервера");
                send_to_java(&state_clone, format!("Сервер запущен на порту {}! Ждем клиента...\n", port));

                if let Ok((stream, addr)) = listener.accept() {
                    send_to_java(&state_clone, format!("Клиент подключился: {}\n", addr));
                    let mut s = state_clone.lock().unwrap();
                    s.server_stream = Some(stream.try_clone().unwrap());
                    drop(s);
                    
                    listen_stream(&state_clone, stream);
                }
            });
        }
    }
}

#[no_mangle]
pub extern "C" fn connect_to_server(ip: *const c_char, port: u16) {
    unsafe {
        let ip_str = CStr::from_ptr(ip).to_string_lossy().into_owned();
        if let Some(ref state) = STATE {
            let state_clone = Arc::clone(state);
            thread::spawn(move || {
                let address = format!("{}:{}", ip_str, port);
                match TcpStream::connect(&address) {
                    Ok(stream) => {
                        send_to_java(&state_clone, format!("Подключено к серверу {}!\n", address));
                        let mut s = state_clone.lock().unwrap();
                        s.client_stream = Some(stream.try_clone().unwrap());
                        drop(s);
                        listen_stream(&state_clone, stream);
                    }
                    Err(e) => {
                        send_to_java(&state_clone, format!("Ошибка подключения: {}\n", e));
                    }
                }
            });
        }
    }
}

#[no_mangle]
pub extern "C" fn send_message(msg: *const c_char) {
    unsafe {
        let msg_str = CStr::from_ptr(msg).to_string_lossy().into_owned();
        if let Some(ref state) = STATE {
            let mut s = state.lock().unwrap();
            let stream_opt = if s.is_server { &mut s.server_stream } else { &mut s.client_stream };
            
            if let Some(stream) = stream_opt {
                let _ = writeln!(stream, "{}", msg_str);
                let _ = stream.flush();
            }
        }
    }
}

// Функция чтения входящих сообщений
fn listen_stream(state: &Arc<Mutex<AppState>>, stream: TcpStream) {
    let state_clone = Arc::clone(state);
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        if let Ok(msg) = line {
            send_to_java(&state_clone, format!("[Входящее] {}\n", msg));
        } else {
            break;
        }
    }
    send_to_java(&state_clone, "Соединение закрыто.\n".to_string());
}
