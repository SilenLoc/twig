use actix_web::http::Uri;
use maud::html;
use resend_rs::{Config, Resend, types::CreateEmailBaseOptions};

use crate::auth::NamespaceRole;

pub const RESEND_API_BASE_URL: &str = "https://api.resend.com";

#[derive(Debug, PartialEq, Eq)]
pub enum InvitationDelivery {
    Sent,
    SkippedNoApiKey,
    Failed(String),
}

pub struct ResendMailer {
    client: Resend,
    from: String,
}

impl ResendMailer {
    pub fn new(api_key: &str, from: &str) -> Self {
        Self::with_base_url(api_key, from, RESEND_API_BASE_URL)
            .expect("the built-in Resend API URL is valid")
    }

    pub fn with_base_url(api_key: &str, from: &str, base_url: &str) -> Result<Self, String> {
        let base_url = base_url
            .parse()
            .map_err(|error| format!("Invalid Resend API URL: {error}"))?;
        let config = Config::builder(api_key).base_url(base_url).build();
        Ok(Self {
            client: Resend::with_config(config),
            from: from.to_string(),
        })
    }

    pub async fn send_namespace_invitation(
        &self,
        recipient: &str,
        namespace: &str,
        role: NamespaceRole,
        invite_url: &str,
    ) -> Result<(), String> {
        let subject = format!("You've been invited to join {namespace} on Twig");
        let body = html! {
            p { "You have been invited to join the " (namespace) " namespace on Twig. Your role is " (role.display_name()) "." }
            p { a href=(invite_url) { "Set up your account" } }
            p { "Your inviter chose how long this invitation link remains valid." }
        }
        .into_string();
        let text = format!(
            "You have been invited to join the {namespace} namespace on Twig. Your role is {}.\n\nSet up your account: {invite_url}",
            role.display_name()
        );
        let email = CreateEmailBaseOptions::new(&self.from, [recipient], subject)
            .with_html(&body)
            .with_text(&text);

        self.client
            .emails
            .send(email)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

pub fn invitation_url(public_base_url: &str, token: &str) -> Result<String, String> {
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Invalid invitation token".to_string());
    }
    let uri = public_base_url
        .parse::<Uri>()
        .map_err(|error| format!("Invalid PUBLIC_BASE_URL: {error}"))?;
    let scheme = uri
        .scheme_str()
        .filter(|scheme| matches!(*scheme, "http" | "https"))
        .ok_or_else(|| "PUBLIC_BASE_URL must use http or https".to_string())?;
    let authority = uri
        .authority()
        .ok_or_else(|| "PUBLIC_BASE_URL must include a host".to_string())?;
    if authority.as_str().contains('@') || !matches!(uri.path(), "" | "/") || uri.query().is_some()
    {
        return Err("PUBLIC_BASE_URL must be an origin without credentials, path, or query".into());
    }

    Ok(format!("{scheme}://{authority}/auth/accept-invite/{token}"))
}

pub async fn send_invitation_if_configured(
    api_key: Option<&str>,
    from: &str,
    public_base_url: Option<&str>,
    recipient: &str,
    namespace: &str,
    role: NamespaceRole,
    token: &str,
) -> InvitationDelivery {
    send_invitation_to(
        api_key,
        from,
        public_base_url,
        recipient,
        namespace,
        role,
        token,
        None,
    )
    .await
}

async fn send_invitation_to(
    api_key: Option<&str>,
    from: &str,
    public_base_url: Option<&str>,
    recipient: &str,
    namespace: &str,
    role: NamespaceRole,
    token: &str,
    resend_api_base_url: Option<&str>,
) -> InvitationDelivery {
    let Some(api_key) = api_key.filter(|key| !key.trim().is_empty()) else {
        return InvitationDelivery::SkippedNoApiKey;
    };
    let Some(public_base_url) = public_base_url else {
        return InvitationDelivery::Failed(
            "PUBLIC_BASE_URL is required before email can be sent".to_string(),
        );
    };
    let invite_url = match invitation_url(public_base_url, token) {
        Ok(url) => url,
        Err(error) => return InvitationDelivery::Failed(error),
    };
    let mailer = match resend_api_base_url {
        Some(base_url) => match ResendMailer::with_base_url(api_key, from, base_url) {
            Ok(mailer) => mailer,
            Err(error) => return InvitationDelivery::Failed(error),
        },
        None => ResendMailer::new(api_key, from),
    };

    match mailer
        .send_namespace_invitation(recipient, namespace, role, &invite_url)
        .await
    {
        Ok(()) => InvitationDelivery::Sent,
        Err(error) => InvitationDelivery::Failed(error),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        sync::mpsc::{self, Receiver},
        thread,
        time::Duration,
    };

    use super::*;

    fn spawn_fake_resend(status: u16, reason: &str, body: &str) -> (String, Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake Resend");
        let address = listener.local_addr().unwrap();
        let (request_sender, request_receiver) = mpsc::channel();
        let reason = reason.to_string();
        let body = body.to_string();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept Resend request");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let request = read_http_request(&mut stream);
            request_sender.send(request).unwrap();
            write!(
                stream,
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            stream.flush().unwrap();
        });
        (format!("http://{address}"), request_receiver)
    }

    fn read_http_request(stream: &mut TcpStream) -> String {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let bytes_read = stream.read(&mut buffer).expect("read fake Resend request");
            if bytes_read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..bytes_read]);
            let Some(headers_end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n")
            else {
                continue;
            };
            let headers = String::from_utf8_lossy(&request[..headers_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if request.len() >= headers_end + 4 + content_length {
                break;
            }
        }
        String::from_utf8(request).expect("request is UTF-8")
    }

    #[tokio::test]
    async fn missing_api_key_skips_sender_construction_and_network_request() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind no-request sentinel");
        listener.set_nonblocking(true).unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let delivery = send_invitation_if_configured(
            None,
            "onboarding@resend.dev",
            Some(&base_url),
            "person@example.com",
            "acme",
            NamespaceRole::Contributor,
            &"a".repeat(64),
        )
        .await;

        assert_eq!(delivery, InvitationDelivery::SkippedNoApiKey);
        assert!(
            matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
        );
    }

    #[tokio::test]
    async fn missing_public_url_skips_delivery_without_network_request() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind no-request sentinel");
        listener.set_nonblocking(true).unwrap();
        let delivery = send_invitation_if_configured(
            Some("re_test_only"),
            "onboarding@resend.dev",
            None,
            "person@example.com",
            "acme",
            NamespaceRole::Contributor,
            &"b".repeat(64),
        )
        .await;

        assert!(
            matches!(delivery, InvitationDelivery::Failed(message) if message.contains("PUBLIC_BASE_URL"))
        );
        assert!(
            matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
        );
    }

    #[tokio::test]
    async fn resend_sends_invitation_payload_to_the_configured_api() {
        let (base_url, requests) = spawn_fake_resend(200, "OK", r#"{"id":"email_123"}"#);
        let mailer =
            ResendMailer::with_base_url("re_test_only", "Twig <from@example.com>", &base_url)
                .unwrap();
        let invite_url = invitation_url(&base_url, &"c".repeat(64)).unwrap();
        mailer
            .send_namespace_invitation(
                "person@example.com",
                "<acme>",
                NamespaceRole::Owner,
                &invite_url,
            )
            .await
            .expect("fake Resend should accept email");

        let request = requests.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(request.starts_with("POST /emails HTTP/1.1"));
        assert!(request.contains("authorization: Bearer re_test_only"));
        let body_start = request.find("\r\n\r\n").unwrap() + 4;
        let body: serde_json::Value = serde_json::from_str(&request[body_start..]).unwrap();
        assert_eq!(body["from"], "Twig <from@example.com>");
        assert_eq!(body["to"][0], "person@example.com");
        assert_eq!(
            body["subject"],
            "You've been invited to join <acme> on Twig"
        );
        assert!(body["html"].as_str().unwrap().contains("&lt;acme&gt;"));
        assert!(body["text"].as_str().unwrap().contains("role is Owner"));
        assert!(body["html"].as_str().unwrap().contains(&invite_url));
    }

    #[tokio::test]
    async fn resend_failure_is_reported_for_retry_and_recovery() {
        let (base_url, requests) = spawn_fake_resend(
            422,
            "Unprocessable Entity",
            r#"{"statusCode":422,"message":"invalid sender","name":"validation_error"}"#,
        );
        let delivery = send_invitation_to(
            Some("re_test_only"),
            "invalid@example.com",
            Some(&base_url),
            "person@example.com",
            "acme",
            NamespaceRole::Contributor,
            &"d".repeat(64),
            Some(&base_url),
        )
        .await;

        assert!(matches!(delivery, InvitationDelivery::Failed(_)));
        let request = requests.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(request.starts_with("POST /emails HTTP/1.1"));
    }

    #[test]
    fn invitation_url_requires_a_safe_origin_and_token() {
        let token = "e".repeat(64);
        assert_eq!(
            invitation_url("https://twig.example/", &token).unwrap(),
            format!("https://twig.example/auth/accept-invite/{token}")
        );
        for (base, token) in [
            ("javascript:alert(1)", token.as_str()),
            ("https://user:pass@example.com", token.as_str()),
            ("https://example.com/path", token.as_str()),
            ("https://example.com?next=evil", token.as_str()),
            ("https://example.com", "not-a-token"),
        ] {
            assert!(invitation_url(base, token).is_err(), "accepted {base:?}");
        }
    }
}
