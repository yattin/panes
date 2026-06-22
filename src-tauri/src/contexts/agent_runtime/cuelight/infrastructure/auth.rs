use std::sync::Mutex;

static GLOBAL_AUTH_TOKEN: Mutex<Option<String>> = Mutex::new(None);

pub fn set_global_auth_token(token: String) {
    if let Ok(mut guard) = GLOBAL_AUTH_TOKEN.lock() {
        *guard = Some(token);
    }
}

pub fn get_global_auth_token() -> Option<String> {
    GLOBAL_AUTH_TOKEN
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
}
