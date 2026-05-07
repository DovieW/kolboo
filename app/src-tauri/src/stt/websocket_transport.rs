//! Shared realtime WebSocket transport policy.
//!
//! Streaming provider protocol state machines still live in the concrete adapters. This Module
//! only owns the connection policy that sits underneath them: manual proxy tunnelling, manual
//! no-proxy bypass, and TLS connector customization for trusted CA / invalid-cert overrides.

use base64::Engine as _;
use native_tls::{Certificate, TlsConnector};
use reqwest::Url;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};
use tokio_tungstenite::tungstenite::http::Request;
use tokio_tungstenite::{client_async_tls_with_config, Connector, MaybeTlsStream, WebSocketStream};

use crate::settings::{ManualProxySettings, ProxyMode, ProxySettings, TrustedCaCertFormat};
use crate::stt::SttError;

const MAX_PROXY_RESPONSE_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone)]
struct WsEndpoint {
    host: String,
    port: u16,
    authority: String,
    is_tls: bool,
}

pub(crate) fn describe_websocket_transport_policy_gap(
    proxy_settings: &ProxySettings,
) -> Option<String> {
    if proxy_settings.mode == ProxyMode::Manual
        && proxy_settings
            .manual
            .proxy_url
            .trim()
            .to_lowercase()
            .starts_with("https://")
    {
        return Some(
			"Realtime streaming transport note: HTTPS proxy URLs are not yet supported for realtime WebSocket STT connections."
				.to_string(),
		);
    }

    None
}

pub(crate) async fn connect_ws_with_transport_policy(
    req: Request<()>,
    connect_timeout: Duration,
    proxy_settings: &ProxySettings,
) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, SttError> {
    let endpoint = endpoint_from_request(&req)?;
    let stream = match proxy_settings.mode {
        ProxyMode::Manual => {
            connect_with_manual_proxy_policy(&endpoint, proxy_settings, connect_timeout).await?
        }
        ProxyMode::NoProxy | ProxyMode::System => {
            connect_tcp_stream(&endpoint.host, endpoint.port, connect_timeout).await?
        }
    };

    let connector = if endpoint.is_tls {
        Some(build_tls_connector(proxy_settings)?)
    } else {
        None
    };

    let handshake = client_async_tls_with_config(req, stream, None, connector);
    let (ws_stream, _) = timeout(connect_timeout, handshake)
        .await
        .map_err(|_| SttError::Timeout)?
        .map_err(|error| SttError::NetworkMessage(format!("WS connect failed: {}", error)))?;

    Ok(ws_stream)
}

fn endpoint_from_request(req: &Request<()>) -> Result<WsEndpoint, SttError> {
    let uri = req.uri();
    let host = uri
        .host()
        .ok_or_else(|| SttError::Config("WS request is missing a host".to_string()))?
        .to_string();
    let is_tls = matches!(uri.scheme_str(), Some("wss" | "https"));
    let port = uri.port_u16().unwrap_or(if is_tls { 443 } else { 80 });
    let authority = uri
        .authority()
        .map(|authority| authority.as_str().to_string())
        .unwrap_or_else(|| format!("{}:{}", host, port));

    Ok(WsEndpoint {
        host,
        port,
        authority,
        is_tls,
    })
}

async fn connect_with_manual_proxy_policy(
    endpoint: &WsEndpoint,
    proxy_settings: &ProxySettings,
    connect_timeout: Duration,
) -> Result<TcpStream, SttError> {
    let manual = &proxy_settings.manual;
    let proxy_url = manual.proxy_url.trim();
    if proxy_url.is_empty() {
        return Err(SttError::Config(
            "Manual proxy mode enabled but proxy_url is empty".to_string(),
        ));
    }

    if manual_proxy_bypasses_host(manual, &endpoint.host, endpoint.port) {
        return connect_tcp_stream(&endpoint.host, endpoint.port, connect_timeout).await;
    }

    let proxy = Url::parse(proxy_url)
        .map_err(|error| SttError::Config(format!("Invalid websocket proxy URL: {}", error)))?;
    if proxy.scheme() != "http" {
        return Err(SttError::Config(format!(
			"Unsupported websocket proxy URL scheme '{}': currently only http:// proxies are supported",
			proxy.scheme()
		)));
    }

    let proxy_host = proxy.host_str().ok_or_else(|| {
        SttError::Config("Manual websocket proxy URL is missing a host".to_string())
    })?;
    let proxy_port = proxy.port_or_known_default().ok_or_else(|| {
        SttError::Config("Manual websocket proxy URL is missing a port".to_string())
    })?;

    let mut stream = connect_tcp_stream(proxy_host, proxy_port, connect_timeout).await?;
    send_proxy_connect_request(&mut stream, endpoint, manual, connect_timeout).await?;
    Ok(stream)
}

async fn connect_tcp_stream(
    host: &str,
    port: u16,
    connect_timeout: Duration,
) -> Result<TcpStream, SttError> {
    timeout(connect_timeout, TcpStream::connect((host, port)))
        .await
        .map_err(|_| SttError::Timeout)?
        .map_err(|error| {
            SttError::NetworkMessage(format!(
                "TCP connect to {}:{} failed: {}",
                host, port, error
            ))
        })
}

async fn send_proxy_connect_request(
    stream: &mut TcpStream,
    endpoint: &WsEndpoint,
    manual: &ManualProxySettings,
    connect_timeout: Duration,
) -> Result<(), SttError> {
    let mut request = format!(
        "CONNECT {} HTTP/1.1\r\nHost: {}\r\nProxy-Connection: Keep-Alive\r\n",
        endpoint.authority, endpoint.authority,
    );

    let username = manual.username.trim();
    if !username.is_empty() {
        let credentials = format!("{}:{}", username, manual.password);
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials);
        request.push_str(&format!("Proxy-Authorization: Basic {}\r\n", encoded,));
    }
    request.push_str("\r\n");

    timeout(connect_timeout, stream.write_all(request.as_bytes()))
        .await
        .map_err(|_| SttError::Timeout)?
        .map_err(|error| {
            SttError::NetworkMessage(format!("WS proxy CONNECT send failed: {}", error))
        })?;

    timeout(connect_timeout, stream.flush())
        .await
        .map_err(|_| SttError::Timeout)?
        .map_err(|error| {
            SttError::NetworkMessage(format!("WS proxy CONNECT flush failed: {}", error))
        })?;

    let mut response = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let read = timeout(connect_timeout, stream.read(&mut buffer))
            .await
            .map_err(|_| SttError::Timeout)?
            .map_err(|error| {
                SttError::NetworkMessage(format!("WS proxy CONNECT read failed: {}", error))
            })?;

        if read == 0 {
            return Err(SttError::NetworkMessage(
                "WS proxy CONNECT closed before returning a response".to_string(),
            ));
        }

        response.extend_from_slice(&buffer[..read]);
        if response.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if response.len() > MAX_PROXY_RESPONSE_BYTES {
            return Err(SttError::NetworkMessage(
                "WS proxy CONNECT response exceeded the allowed header size".to_string(),
            ));
        }
    }

    let response_text = String::from_utf8_lossy(&response);
    let status_line = response_text.lines().next().unwrap_or_default().trim();
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or_default();

    if !(200..300).contains(&status_code) {
        return Err(SttError::NetworkMessage(format!(
            "WS proxy CONNECT failed: {}",
            status_line
        )));
    }

    Ok(())
}

fn build_tls_connector(proxy_settings: &ProxySettings) -> Result<Connector, SttError> {
    let mut builder = TlsConnector::builder();
    builder.danger_accept_invalid_certs(proxy_settings.danger_accept_invalid_certs);

    for cert in &proxy_settings.trusted_ca_certificates {
        let raw = cert.data_base64.trim();
        if raw.is_empty() {
            continue;
        }

        let bytes = match base64::engine::general_purpose::STANDARD.decode(raw) {
            Ok(bytes) => bytes,
            Err(error) => {
                log::warn!(
                    "Failed to decode trusted WebSocket CA certificate {} ({}): {}",
                    cert.id,
                    cert.file_name,
                    error
                );
                continue;
            }
        };

        let parsed = match cert.format {
            TrustedCaCertFormat::Pem => Certificate::from_pem(&bytes),
            TrustedCaCertFormat::Der => Certificate::from_der(&bytes),
        };

        match parsed {
            Ok(parsed) => {
                builder.add_root_certificate(parsed);
            }
            Err(error) => {
                log::warn!(
                    "Failed to load trusted WebSocket CA certificate {} ({}): {}",
                    cert.id,
                    cert.file_name,
                    error
                );
            }
        }
    }

    builder
        .build()
        .map(Connector::NativeTls)
        .map_err(|error| SttError::Config(format!("Failed to build WS TLS connector: {}", error)))
}

fn manual_proxy_bypasses_host(manual: &ManualProxySettings, host: &str, port: u16) -> bool {
    manual
        .no_proxy
        .split(|ch: char| ch == ',' || ch.is_whitespace())
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .any(|entry| no_proxy_entry_matches_host(entry, host, port))
}

fn no_proxy_entry_matches_host(entry: &str, host: &str, port: u16) -> bool {
    if entry == "*" {
        return true;
    }

    let (entry_host, entry_port) = match entry.rsplit_once(':') {
        Some((candidate_host, candidate_port))
            if candidate_port.chars().all(|ch| ch.is_ascii_digit()) =>
        {
            (candidate_host, candidate_port.parse::<u16>().ok())
        }
        _ => (entry, None),
    };

    if let Some(required_port) = entry_port {
        if required_port != port {
            return false;
        }
    }

    let entry_host = entry_host.trim_start_matches('.');
    if entry_host.is_empty() {
        return false;
    }

    if host.eq_ignore_ascii_case(entry_host) {
        return true;
    }

    host.len() > entry_host.len()
        && host.ends_with(entry_host)
        && host.as_bytes()[host.len() - entry_host.len() - 1] == b'.'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_proxy_matches_exact_hosts_suffixes_and_ports() {
        let manual = ManualProxySettings {
            proxy_url: "http://proxy.example.test:8080".to_string(),
            no_proxy: "localhost, .corp.example.test, api.example.test:8443".to_string(),
            username: String::new(),
            password: String::new(),
        };

        assert!(manual_proxy_bypasses_host(&manual, "localhost", 80));
        assert!(manual_proxy_bypasses_host(
            &manual,
            "voice.corp.example.test",
            443,
        ));
        assert!(manual_proxy_bypasses_host(
            &manual,
            "api.example.test",
            8443
        ));
        assert!(!manual_proxy_bypasses_host(
            &manual,
            "api.example.test",
            443
        ));
    }

    #[test]
    fn http_manual_proxy_has_no_remaining_gap_message() {
        let mut proxy_settings = ProxySettings {
            mode: ProxyMode::Manual,
            ..ProxySettings::default()
        };
        proxy_settings.manual.proxy_url = "http://127.0.0.1:8080".to_string();

        assert_eq!(
            describe_websocket_transport_policy_gap(&proxy_settings),
            None
        );
    }

    #[test]
    fn https_manual_proxy_reports_remaining_gap() {
        let mut proxy_settings = ProxySettings {
            mode: ProxyMode::Manual,
            ..ProxySettings::default()
        };
        proxy_settings.manual.proxy_url = "https://proxy.example.test:8443".to_string();

        let message = describe_websocket_transport_policy_gap(&proxy_settings)
            .expect("expected unsupported https-proxy note");
        assert!(message.contains("HTTPS proxy URLs"));
    }
}
