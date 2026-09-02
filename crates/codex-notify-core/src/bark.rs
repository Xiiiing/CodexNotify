use crate::error::{CoreError, CoreResult};
use crate::notification::Notification;
use crate::settings::AppSettings;
use aes::{Aes128, Aes256};
use base64::{engine::general_purpose::STANDARD, Engine};
use cbc::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
use rand::{distributions::Alphanumeric, Rng};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use url::Url;

type Aes128CbcEnc = cbc::Encryptor<Aes128>;
type Aes256CbcEnc = cbc::Encryptor<Aes256>;

fn endpoint(server: &str, key: &str) -> CoreResult<Url> {
    if key.starts_with("http://") || key.starts_with("https://") {
        return Url::parse(key).map_err(|_| CoreError::InvalidBarkServer);
    }
    if key.trim().is_empty() {
        return Err(CoreError::InvalidConfig(
            "Bark device key is missing".into(),
        ));
    }
    let mut url = Url::parse(if server.trim().is_empty() {
        "https://api.day.app"
    } else {
        server
    })
    .map_err(|_| CoreError::InvalidBarkServer)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(CoreError::InvalidBarkServer);
    }
    url.path_segments_mut()
        .map_err(|_| CoreError::InvalidBarkServer)?
        .pop_if_empty()
        .push(key);
    Ok(url)
}

fn payload(notification: &Notification) -> Value {
    let mut value = json!({"title":notification.title,"body":notification.body,"group":notification.group,"level":notification.level});
    let object = value.as_object_mut().unwrap();
    for (key, item) in [
        ("subtitle", &notification.subtitle),
        ("sound", &notification.sound),
        ("icon", &notification.icon),
        ("url", &notification.url),
    ] {
        if !item.is_empty() {
            object.insert(key.into(), Value::String(item.clone()));
        }
    }
    value
}

fn encrypt(value: &Value, key: &str, algorithm: &str) -> CoreResult<HashMap<&'static str, String>> {
    let expected = if algorithm == "AES-128-CBC" { 16 } else { 32 };
    if key.len() != expected {
        return Err(CoreError::InvalidEncryptionKey);
    }
    let iv: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect();
    let raw = serde_json::to_vec(value)?;
    let mut buffer = vec![0_u8; raw.len() + 16];
    buffer[..raw.len()].copy_from_slice(&raw);
    let encrypted = if expected == 16 {
        Aes128CbcEnc::new_from_slices(key.as_bytes(), iv.as_bytes())
            .unwrap()
            .encrypt_padded_mut::<Pkcs7>(&mut buffer, raw.len())
    } else {
        Aes256CbcEnc::new_from_slices(key.as_bytes(), iv.as_bytes())
            .unwrap()
            .encrypt_padded_mut::<Pkcs7>(&mut buffer, raw.len())
    }
    .map_err(|_| CoreError::InvalidConfig("unable to pad Bark payload".into()))?;
    let ciphertext = encrypted.to_vec();
    Ok(HashMap::from([
        ("ciphertext", STANDARD.encode(ciphertext)),
        ("iv", iv),
    ]))
}

fn invalid_device_key(status: reqwest::StatusCode, body: &str) -> bool {
    if status != reqwest::StatusCode::BAD_REQUEST {
        return false;
    }
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| body.to_owned())
        .to_ascii_lowercase();
    [
        "device key",
        "device token",
        "get device token",
        "invalid key",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

fn request_error(error: reqwest::Error) -> CoreError {
    if error.is_timeout() {
        CoreError::BarkTimeout
    } else {
        CoreError::BarkUnreachable
    }
}

fn parse_response(status: reqwest::StatusCode, text: &str) -> CoreResult<Value> {
    if status.is_server_error() {
        return Err(CoreError::BarkServer);
    }
    if !status.is_success() {
        return Err(if invalid_device_key(status, text) {
            CoreError::BarkInvalidKey
        } else {
            CoreError::BarkRejected
        });
    }
    if text.trim().is_empty() {
        return Ok(json!({}));
    }
    let result: Value = serde_json::from_str(text).map_err(|_| CoreError::BarkRejected)?;
    if let Some(code) = result.get("code").and_then(Value::as_i64) {
        if code != 200 {
            let synthetic_status = reqwest::StatusCode::from_u16(code as u16)
                .unwrap_or(reqwest::StatusCode::BAD_REQUEST);
            return Err(if invalid_device_key(synthetic_status, text) {
                CoreError::BarkInvalidKey
            } else {
                CoreError::BarkRejected
            });
        }
    }
    Ok(result)
}

pub fn send(
    notification: &Notification,
    settings: &AppSettings,
    bark_key: &str,
    encryption_key: Option<&str>,
) -> CoreResult<Value> {
    send_with_timeouts(
        notification,
        settings,
        bark_key,
        encryption_key,
        Duration::from_secs(settings.request_timeout),
        None,
    )
}

pub fn send_test(
    notification: &Notification,
    settings: &AppSettings,
    bark_key: &str,
    encryption_key: Option<&str>,
) -> CoreResult<Value> {
    send_with_timeouts(
        notification,
        settings,
        bark_key,
        encryption_key,
        Duration::from_secs(8),
        Some(Duration::from_secs(3)),
    )
}

fn send_with_timeouts(
    notification: &Notification,
    settings: &AppSettings,
    bark_key: &str,
    encryption_key: Option<&str>,
    timeout: Duration,
    connect_timeout: Option<Duration>,
) -> CoreResult<Value> {
    let mut builder = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .user_agent(concat!("CodexNotify/", env!("CARGO_PKG_VERSION")));
    if let Some(connect_timeout) = connect_timeout {
        builder = builder.connect_timeout(connect_timeout);
    }
    let client = builder.build().map_err(request_error)?;
    let url = endpoint(&settings.bark_server, bark_key)?;
    let request = if settings.encryption_enabled {
        let key = encryption_key.filter(|v| !v.is_empty()).ok_or_else(|| {
            CoreError::InvalidConfig("Bark encryption is enabled but its key is missing".into())
        })?;
        client.post(url).form(&encrypt(
            &payload(notification),
            key,
            &settings.encryption_algorithm,
        )?)
    } else {
        client.post(url).json(&payload(notification))
    };
    let response = request.send().map_err(request_error)?;
    let status = response.status();
    let text = response.text().map_err(request_error)?;
    parse_response(status, &text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cbc::cipher::{BlockDecryptMut, KeyIvInit};

    #[test]
    fn aes_form_round_trips() {
        type Aes128CbcDec = cbc::Decryptor<Aes128>;
        let value = json!({"title":"画图","body":"完成"});
        let form = encrypt(&value, "1234567890123456", "AES-128-CBC").unwrap();
        let iv = form.get("iv").unwrap();
        let mut encrypted = STANDARD.decode(form.get("ciphertext").unwrap()).unwrap();
        let decrypted = Aes128CbcDec::new_from_slices(b"1234567890123456", iv.as_bytes())
            .unwrap()
            .decrypt_padded_mut::<Pkcs7>(&mut encrypted)
            .unwrap();
        let decoded: Value = serde_json::from_slice(decrypted).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn posts_json_to_self_hosted_bark() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            use std::io::{Read, Write};
            let mut request = [0_u8; 4096];
            let length = socket.read(&mut request).unwrap();
            let text = String::from_utf8_lossy(&request[..length]);
            assert!(text.starts_with("POST /device-key HTTP/1.1"));
            assert!(text.contains("CodexNotify"));
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 12\r\nConnection: close\r\n\r\n{\"code\":200}").unwrap();
        });
        let settings = AppSettings {
            bark_server: format!("http://{address}"),
            ..AppSettings::default()
        };
        let notification = Notification {
            event_key: "test".into(),
            event_type: "Test".into(),
            session_id: String::new(),
            turn_id: String::new(),
            project: "CodexNotify".into(),
            cwd: String::new(),
            title: "CodexNotify".into(),
            subtitle: String::new(),
            body: "works".into(),
            group: "Codex".into(),
            level: "active".into(),
            sound: String::new(),
            icon: String::new(),
            url: String::new(),
            suppressed: false,
            suppress_reason: String::new(),
        };
        let response = send(&notification, &settings, "device-key", None).unwrap();
        assert_eq!(response["code"], 200);
        server.join().unwrap();
    }

    #[test]
    fn classifies_invalid_device_key_without_leaking_it() {
        let error = parse_response(
            reqwest::StatusCode::BAD_REQUEST,
            r#"{"code":400,"message":"failed to get device token: super-secret-key"}"#,
        )
        .unwrap_err();
        assert!(matches!(error, CoreError::BarkInvalidKey));
        assert!(!error.to_string().contains("super-secret-key"));
    }

    #[test]
    fn classifies_rejected_malformed_and_server_responses() {
        assert!(matches!(
            parse_response(reqwest::StatusCode::OK, "not-json").unwrap_err(),
            CoreError::BarkRejected
        ));
        assert!(matches!(
            parse_response(
                reqwest::StatusCode::OK,
                r#"{"code":403,"message":"denied"}"#
            )
            .unwrap_err(),
            CoreError::BarkRejected
        ));
        assert!(matches!(
            parse_response(reqwest::StatusCode::INTERNAL_SERVER_ERROR, "oops").unwrap_err(),
            CoreError::BarkServer
        ));
    }

    #[test]
    fn interactive_test_has_a_bounded_timeout() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (_socket, _) = listener.accept().unwrap();
            std::thread::sleep(Duration::from_millis(150));
        });
        let settings = AppSettings {
            bark_server: format!("http://{address}"),
            ..AppSettings::default()
        };
        let notification = Notification {
            event_key: "timeout-test".into(),
            event_type: "Test".into(),
            session_id: String::new(),
            turn_id: String::new(),
            project: "CodexNotify".into(),
            cwd: String::new(),
            title: "Test".into(),
            subtitle: String::new(),
            body: "Test".into(),
            group: "Codex".into(),
            level: "active".into(),
            sound: String::new(),
            icon: String::new(),
            url: String::new(),
            suppressed: false,
            suppress_reason: String::new(),
        };
        let error = send_with_timeouts(
            &notification,
            &settings,
            "device-key",
            None,
            Duration::from_millis(40),
            Some(Duration::from_millis(20)),
        )
        .unwrap_err();
        assert!(matches!(error, CoreError::BarkTimeout));
        server.join().unwrap();
    }
}
