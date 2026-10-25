// SPDX-License-Identifier: MIT
//! AI Assistant's native-only profile and bounded chat request.
use super::plugin_views::ViewAuthorization;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tauri::AppHandle;

const KEY_SERVICE: &str = "CobbleLauncher.AiAssistant";
const KEY_USER: &str = "profile-v1";
const MAX_REPLY: usize = 64 * 1024;
static CONFIG_LOCK: Mutex<()> = Mutex::new(());
type ActiveRequests = Mutex<HashMap<(String, String), Arc<AtomicBool>>>;
static REQUESTS: OnceLock<ActiveRequests> = OnceLock::new();
fn requests() -> &'static ActiveRequests {
    REQUESTS.get_or_init(Default::default)
}
struct RequestLease(String, String);
impl Drop for RequestLease {
    fn drop(&mut self) {
        if let Ok(mut active) = requests().lock() {
            active.remove(&(self.0.clone(), self.1.clone()));
        }
    }
}
fn profile_path() -> Result<PathBuf, String> {
    Ok(dirs::config_dir()
        .ok_or("AI_CONFIG_UNAVAILABLE")?
        .join("CobbleLauncher/ai-assistant.json"))
}
fn key_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEY_SERVICE, KEY_USER).map_err(|_| "AI_KEYRING_UNAVAILABLE".into())
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Profile {
    endpoint: String,
    model: String,
}
#[derive(Serialize)]
pub struct Status {
    configured: bool,
    endpoint: Option<String>,
    model: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Configure {
    endpoint: String,
    model: String,
    key: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    role: Role,
    content: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Diagnostic {
    game_id: String,
    diagnostic_id: String,
    revision: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Chat {
    request_id: String,
    messages: Vec<Message>,
    diagnostic: Option<Diagnostic>,
}
impl Chat {
    pub(super) fn diagnostic_game_id(&self) -> Option<&str> {
        self.diagnostic.as_ref().map(|value| value.game_id.as_str())
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Cancel {
    request_id: String,
}

fn valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn endpoint(value: &str) -> Result<String, String> {
    if value.len() > 1024 {
        return Err("AI_CONFIG_INVALID".into());
    }
    let parsed = url::Url::parse(value).map_err(|_| "AI_CONFIG_INVALID")?;
    let host = parsed.host_str().ok_or("AI_CONFIG_INVALID")?;
    let loopback = host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "[::1]";
    if !((parsed.scheme() == "https") || (parsed.scheme() == "http" && loopback))
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.path() == "/"
        || parsed.path().len() > 512
    {
        return Err("AI_CONFIG_INVALID".into());
    }
    Ok(parsed.to_string())
}
fn local_endpoint(value: &str) -> bool {
    url::Url::parse(value).ok().is_some_and(|url| {
        url.scheme() == "http"
            && url.host_str().is_some_and(|host| {
                host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "[::1]"
            })
    })
}
fn profile() -> Result<Option<Profile>, String> {
    let bytes = match fs::read(profile_path()?) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("AI_CONFIG_UNAVAILABLE".into()),
    };
    let value: Profile = serde_json::from_slice(&bytes).map_err(|_| "AI_CONFIG_INVALID")?;
    endpoint(&value.endpoint)?;
    if value.model.is_empty() || value.model.len() > 128 {
        return Err("AI_CONFIG_INVALID".into());
    }
    Ok(Some(value))
}
fn status_value(profile: Option<Profile>) -> Result<Status, String> {
    let configured = if let Some(profile) = profile.as_ref() {
        match key_entry()?.get_password() {
            Ok(key) => !key.is_empty() || local_endpoint(&profile.endpoint),
            Err(keyring::Error::NoEntry) => local_endpoint(&profile.endpoint),
            Err(_) => return Err("AI_KEYRING_UNAVAILABLE".into()),
        }
    } else {
        false
    };
    Ok(Status {
        configured,
        endpoint: profile.as_ref().map(|p| p.endpoint.clone()),
        model: profile.map(|p| p.model),
    })
}
pub async fn status() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let _guard = CONFIG_LOCK.lock().map_err(|_| "AI_CONFIG_UNAVAILABLE")?;
        serde_json::to_value(status_value(profile()?)?).map_err(|_| "AI_CONFIG_UNAVAILABLE".into())
    })
    .await
    .map_err(|_| "AI_CONFIG_UNAVAILABLE".to_string())?
}
pub async fn configure(args: Configure) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = CONFIG_LOCK.lock().map_err(|_| "AI_CONFIG_UNAVAILABLE")?;
        let endpoint = endpoint(&args.endpoint)?;
        let local = local_endpoint(&endpoint);
        if args.model.is_empty()
            || args.model.len() > 128
            || args.model.chars().any(char::is_control)
            || (!local && args.key.is_empty())
            || args.key.len() > 4096
            || args.key.chars().any(char::is_control)
        {
            return Err("AI_CONFIG_INVALID".into());
        }
        let profile = Profile {
            endpoint,
            model: args.model,
        };
        let path = profile_path()?;
        fs::create_dir_all(path.parent().ok_or("AI_CONFIG_UNAVAILABLE")?)
            .map_err(|_| "AI_CONFIG_UNAVAILABLE")?;
        let entry = key_entry()?;
        let previous_profile = fs::read(&path).ok();
        let previous_key = match entry.get_password() {
            Ok(key) => Some(key),
            Err(keyring::Error::NoEntry) => None,
            Err(_) => return Err("AI_KEYRING_UNAVAILABLE".into()),
        };
        if args.key.is_empty() {
            match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(_) => return Err("AI_KEYRING_UNAVAILABLE".into()),
            }
        } else {
            entry
                .set_password(&args.key)
                .map_err(|_| "AI_KEYRING_UNAVAILABLE")?;
        }
        if fs::write(
            &path,
            serde_json::to_vec(&profile).map_err(|_| "AI_CONFIG_UNAVAILABLE")?,
        )
        .is_err()
        {
            match previous_profile {
                Some(bytes) => {
                    let _ = fs::write(&path, bytes);
                }
                None => {
                    let _ = fs::remove_file(&path);
                }
            }
            match previous_key {
                Some(key) => {
                    let _ = entry.set_password(&key);
                }
                None => {
                    let _ = entry.delete_credential();
                }
            }
            return Err("AI_CONFIG_UNAVAILABLE".into());
        }
        serde_json::to_value(status_value(Some(profile))?)
            .map_err(|_| "AI_CONFIG_UNAVAILABLE".into())
    })
    .await
    .map_err(|_| "AI_CONFIG_UNAVAILABLE".to_string())?
}
fn configured_profile() -> Result<(Profile, Option<String>), String> {
    let _guard = CONFIG_LOCK.lock().map_err(|_| "AI_CONFIG_UNAVAILABLE")?;
    let profile = profile()?.ok_or("AI_NOT_CONFIGURED")?;
    let key = match key_entry()?.get_password() {
        Ok(key) if !key.is_empty() => Some(key),
        Ok(_) | Err(keyring::Error::NoEntry) if local_endpoint(&profile.endpoint) => None,
        Ok(_) | Err(keyring::Error::NoEntry) => return Err("AI_NOT_CONFIGURED".into()),
        Err(_) => return Err("AI_KEYRING_UNAVAILABLE".into()),
    };
    Ok((profile, key))
}
pub async fn clear() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let _guard = CONFIG_LOCK.lock().map_err(|_| "AI_CONFIG_UNAVAILABLE")?;
        match key_entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(_) => return Err("AI_KEYRING_UNAVAILABLE".into()),
        }
        let path = profile_path()?;
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("AI_CONFIG_UNAVAILABLE".into()),
        }
        serde_json::to_value(status_value(None)?).map_err(|_| "AI_CONFIG_UNAVAILABLE".into())
    })
    .await
    .map_err(|_| "AI_CONFIG_UNAVAILABLE".to_string())?
}

fn validate_messages(messages: &[Message]) -> Result<(), String> {
    if messages.is_empty()
        || messages.len() > 20
        || !matches!(messages.last().map(|m| &m.role), Some(Role::User))
    {
        return Err("AI_REQUEST_INVALID".into());
    }
    let mut total = 0usize;
    for message in messages {
        let size = message.content.len();
        total += size;
        if size == 0 || size > 16 * 1024 || message.content.contains('\0') {
            return Err("AI_REQUEST_INVALID".into());
        }
    }
    if total > 48 * 1024 {
        return Err("AI_REQUEST_INVALID".into());
    }
    Ok(())
}
fn send_chat(
    profile: &Profile,
    key: Option<&str>,
    messages: &[Message],
    cancelled: &AtomicBool,
    view_cancelled: &AtomicBool,
) -> Result<String, String> {
    if cancelled.load(Ordering::Acquire) || view_cancelled.load(Ordering::Acquire) {
        return Err("AI_CANCELLED".into());
    }
    let body =
        serde_json::to_vec(&json!({"model":profile.model,"messages":messages,"stream":false}))
            .map_err(|_| "AI_REQUEST_INVALID")?;
    let mut easy = curl::easy::Easy::new();
    easy.url(&profile.endpoint)
        .map_err(|_| "AI_CONFIG_INVALID")?;
    easy.post(true).map_err(|_| "AI_REQUEST_FAILED")?;
    easy.post_fields_copy(&body)
        .map_err(|_| "AI_REQUEST_FAILED")?;
    easy.follow_location(false)
        .map_err(|_| "AI_REQUEST_FAILED")?;
    easy.proxy("").map_err(|_| "AI_REQUEST_FAILED")?;
    easy.connect_timeout(Duration::from_secs(10))
        .map_err(|_| "AI_REQUEST_FAILED")?;
    easy.timeout(Duration::from_secs(60))
        .map_err(|_| "AI_REQUEST_FAILED")?;
    easy.progress(true).map_err(|_| "AI_REQUEST_FAILED")?;
    let mut headers = curl::easy::List::new();
    headers
        .append("Content-Type: application/json")
        .map_err(|_| "AI_REQUEST_FAILED")?;
    if let Some(key) = key {
        headers
            .append(&format!("Authorization: Bearer {key}"))
            .map_err(|_| "AI_REQUEST_FAILED")?;
    }
    easy.http_headers(headers)
        .map_err(|_| "AI_REQUEST_FAILED")?;
    let mut response = Vec::new();
    {
        let mut transfer = easy.transfer();
        transfer
            .progress_function(|_, _, _, _| {
                !cancelled.load(Ordering::Acquire) && !view_cancelled.load(Ordering::Acquire)
            })
            .map_err(|_| "AI_REQUEST_FAILED")?;
        transfer
            .write_function(|bytes| {
                if response.len().saturating_add(bytes.len()) > 1024 * 1024 {
                    return Ok(0);
                }
                response.extend_from_slice(bytes);
                Ok(bytes.len())
            })
            .map_err(|_| "AI_REQUEST_FAILED")?;
        let result = transfer.perform();
        if cancelled.load(Ordering::Acquire) || view_cancelled.load(Ordering::Acquire) {
            return Err("AI_CANCELLED".into());
        }
        result.map_err(|_| "AI_REQUEST_FAILED")?;
    }
    if easy.response_code().map_err(|_| "AI_REQUEST_FAILED")? != 200 {
        return Err("AI_REQUEST_FAILED".into());
    }
    let response: Value = serde_json::from_slice(&response).map_err(|_| "AI_REQUEST_FAILED")?;
    let reply = response
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .ok_or("AI_REQUEST_FAILED")?;
    if reply.is_empty() || reply.len() > MAX_REPLY {
        return Err("AI_REQUEST_FAILED".into());
    }
    Ok(reply.to_owned())
}
pub async fn chat(app: AppHandle, auth: &ViewAuthorization, args: Chat) -> Result<Value, String> {
    if !valid_request_id(&args.request_id) {
        return Err("AI_REQUEST_INVALID".into());
    }
    validate_messages(&args.messages)?;
    if args.diagnostic.is_some()
        && !auth
            .manifest
            .permissions
            .as_deref()
            .unwrap_or_default()
            .contains(&cobble_plugin_api::PermissionNode::GameDiagnosticsRead {})
    {
        return Err("PLUGIN_PERMISSION_DENIED".into());
    }
    let session = auth.session_id.clone();
    let request_id = args.request_id.clone();
    let flag = Arc::new(AtomicBool::new(false));
    {
        let mut active = requests().lock().map_err(|_| "AI_BUSY")?;
        if active.keys().any(|(owner, _)| owner == &session) {
            return Err("AI_BUSY".into());
        }
        active.insert((session.clone(), request_id.clone()), flag.clone());
    }
    let _lease = RequestLease(session, request_id);
    chat_registered(app, auth, args, flag).await
}
async fn chat_registered(
    app: AppHandle,
    auth: &ViewAuthorization,
    mut args: Chat,
    flag: Arc<AtomicBool>,
) -> Result<Value, String> {
    let (profile, key) = tauri::async_runtime::spawn_blocking(configured_profile)
        .await
        .map_err(|_| "AI_CONFIG_UNAVAILABLE".to_string())??;
    if flag.load(Ordering::Acquire) {
        return Err("AI_CANCELLED".into());
    }
    if let Some(diagnostic) = args.diagnostic.take() {
        if auth
            .instance_id
            .as_deref()
            .is_some_and(|id| id != diagnostic.game_id)
        {
            return Err("PLUGIN_GAME_SCOPE".into());
        }
        let document = crate::games::game_diagnostics::read(
            app.clone(),
            diagnostic.game_id.clone(),
            diagnostic.diagnostic_id.clone(),
        )
        .await?;
        if document.revision != diagnostic.revision {
            return Err("AI_DIAGNOSTIC_CHANGED".into());
        }
        let excerpt = if document.text.len() > 32 * 1024 {
            let mut start = document.text.len() - 32 * 1024;
            while !document.text.is_char_boundary(start) {
                start += 1;
            }
            &document.text[start..]
        } else {
            &document.text
        };
        let details = format!(
            "Send {} ({} redacted bytes) to {} using {}?",
            document.name,
            excerpt.len(),
            profile.endpoint,
            profile.model
        );
        if !super::plugin_confirm::ask(&app, auth, "Send diagnostic", &details).await? {
            return Err("AI_CONFIRM_CANCELLED".into());
        }
        if flag.load(Ordering::Acquire) {
            return Err("AI_CANCELLED".into());
        }
        // Check the source again after the human confirmation.
        let current = crate::games::game_diagnostics::read(
            app.clone(),
            diagnostic.game_id,
            diagnostic.diagnostic_id,
        )
        .await?;
        if current.revision != diagnostic.revision {
            return Err("AI_DIAGNOSTIC_CHANGED".into());
        }
        args.messages.push(Message {
            role: Role::User,
            content: format!(
                "Diagnostic {} (redacted excerpt):\n{}",
                document.name, excerpt
            ),
        });
    }
    let view_cancelled = auth.cancel.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        send_chat(
            &profile,
            key.as_deref(),
            &args.messages,
            &flag,
            &view_cancelled,
        )
    })
    .await;
    outcome
        .map_err(|_| "AI_REQUEST_FAILED".to_string())?
        .map(|reply| json!({"reply":reply}))
}
pub fn cancel(auth: &ViewAuthorization, args: Cancel) -> Result<Value, String> {
    if !valid_request_id(&args.request_id) {
        return Err("AI_REQUEST_INVALID".into());
    }
    let active = requests().lock().map_err(|_| "AI_BUSY")?;
    let flag = active.get(&(auth.session_id.clone(), args.request_id));
    if let Some(flag) = flag {
        flag.store(true, Ordering::Release);
    }
    Ok(json!({"cancelled":flag.is_some()}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    #[test]
    fn endpoint_allows_user_selected_https_and_loopback_http_only() {
        assert!(endpoint("https://ai.example/v1/chat/completions").is_ok());
        assert!(endpoint("http://127.0.0.1:1234/v1/chat/completions").is_ok());
        for value in [
            "http://ai.example/v1/chat/completions",
            "http://192.168.1.2/v1/chat/completions",
            "https://user:secret@ai.example/v1/chat/completions",
            "https://ai.example/v1/chat/completions?key=x",
        ] {
            assert!(endpoint(value).is_err());
        }
    }
    #[test]
    fn local_http_chat_sends_only_the_saved_endpoint_and_parses_a_bounded_reply() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let count = socket.read(&mut chunk).unwrap();
                bytes.extend_from_slice(&chunk[..count]);
                let Some(split) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
                    continue;
                };
                let headers = String::from_utf8_lossy(&bytes[..split]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|n| n.parse::<usize>().ok())
                    })
                    .unwrap();
                if bytes.len() >= split + 4 + content_length {
                    break;
                }
            }
            let request = String::from_utf8(bytes).unwrap();
            assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1"));
            assert!(request.contains("Authorization: Bearer synthetic-key"));
            assert!(request.contains("\"model\":\"fixture\""));
            assert!(request.contains("\"content\":\"hello\""));
            let body = br#"{"choices":[{"message":{"content":"world"}}]}"#;
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n", body.len()).unwrap();
            socket.write_all(body).unwrap();
        });
        let profile = Profile {
            endpoint: format!("http://127.0.0.1:{port}/v1/chat/completions"),
            model: "fixture".into(),
        };
        let message = Message {
            role: Role::User,
            content: "hello".into(),
        };
        assert_eq!(
            send_chat(
                &profile,
                Some("synthetic-key"),
                &[message],
                &AtomicBool::new(false),
                &AtomicBool::new(false)
            )
            .unwrap(),
            "world"
        );
        server.join().unwrap();
    }
    #[test]
    fn cancelled_request_never_opens_a_connection() {
        let profile = Profile {
            endpoint: "http://127.0.0.1:1/v1/chat/completions".into(),
            model: "fixture".into(),
        };
        let message = Message {
            role: Role::User,
            content: "hello".into(),
        };
        assert_eq!(
            send_chat(
                &profile,
                Some("synthetic-key"),
                &[message],
                &AtomicBool::new(true),
                &AtomicBool::new(false)
            )
            .unwrap_err(),
            "AI_CANCELLED"
        );
    }
    #[test]
    fn redirect_does_not_forward_the_key_to_another_origin() {
        let source = TcpListener::bind("127.0.0.1:0").unwrap();
        let destination = TcpListener::bind("127.0.0.1:0").unwrap();
        destination.set_nonblocking(true).unwrap();
        let port = source.local_addr().unwrap().port();
        let target = destination.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = source.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buffer = [0u8; 4096];
            let read = socket.read(&mut buffer).unwrap();
            assert!(
                String::from_utf8_lossy(&buffer[..read])
                    .contains("Authorization: Bearer synthetic-key")
            );
            write!(socket, "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{target}/leak\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
        });
        let profile = Profile {
            endpoint: format!("http://127.0.0.1:{port}/v1/chat/completions"),
            model: "fixture".into(),
        };
        let message = Message {
            role: Role::User,
            content: "hello".into(),
        };
        assert_eq!(
            send_chat(
                &profile,
                Some("synthetic-key"),
                &[message],
                &AtomicBool::new(false),
                &AtomicBool::new(false)
            )
            .unwrap_err(),
            "AI_REQUEST_FAILED"
        );
        server.join().unwrap();
        assert!(destination.accept().is_err());
    }
    #[test]
    fn local_chat_can_omit_authorization() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut buffer = [0u8; 4096];
            let read = socket.read(&mut buffer).unwrap();
            assert!(!String::from_utf8_lossy(&buffer[..read]).contains("Authorization:"));
            let body = br#"{"choices":[{"message":{"content":"local"}}]}"#;
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            socket.write_all(body).unwrap();
        });
        let profile = Profile {
            endpoint: format!("http://127.0.0.1:{port}/v1/chat/completions"),
            model: "fixture".into(),
        };
        let message = Message {
            role: Role::User,
            content: "hello".into(),
        };
        assert_eq!(
            send_chat(
                &profile,
                None,
                &[message],
                &AtomicBool::new(false),
                &AtomicBool::new(false)
            )
            .unwrap(),
            "local"
        );
        server.join().unwrap();
    }
    #[test]
    fn official_manifest_has_separate_chat_config_and_diagnostic_grants() {
        let manifest: cobble_plugin_api::PluginManifest = serde_json::from_str(include_str!(
            "../../../../examples/ai-assistant/plugin.json"
        ))
        .unwrap();
        manifest.validate().unwrap();
        let nodes = manifest.permissions.unwrap();
        assert!(nodes.contains(&cobble_plugin_api::PermissionNode::AiChat {}));
        assert!(nodes.contains(&cobble_plugin_api::PermissionNode::AiConfigure {}));
        assert!(nodes.contains(&cobble_plugin_api::PermissionNode::GameDiagnosticsRead {}));
    }
}
