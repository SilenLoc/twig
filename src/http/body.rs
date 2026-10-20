use std::sync::Arc;

use actix_web::{FromRequest, HttpRequest, web};
use serde::de::DeserializeOwned;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub const DEFAULT_SMALL_BODY_LIMIT: usize = 1 << 20;
pub const DEFAULT_LARGE_BODY_LIMIT: usize = 1 << 29;
const DEFAULT_SMALL_BODY_SLOTS: usize = 8;
const DEFAULT_LARGE_BODY_SLOTS: usize = 1;

#[derive(Clone, Copy)]
pub enum BodyClass {
    Small,
    Large,
}

pub struct RequestBodyLimits {
    git_read_bytes: usize,
    git_write_bytes: usize,
    binary_upload_bytes: usize,
    edit_form_bytes: usize,
    small_body_slots: Arc<Semaphore>,
    large_body_slots: Arc<Semaphore>,
}

pub struct BoundedBody {
    pub bytes: Vec<u8>,
    _permit: OwnedSemaphorePermit,
}

pub struct BoundedForm<T> {
    pub value: T,
    _permit: OwnedSemaphorePermit,
}

#[derive(Debug)]
pub enum BodyReadError {
    TooLarge,
    Payload(actix_web::Error),
    CapacityClosed,
}

impl RequestBodyLimits {
    /// Constructs limits for isolated tests without sending large bodies.
    #[cfg(test)]
    pub fn for_tests(
        small_body_bytes: usize,
        large_body_bytes: usize,
        small_body_slots: usize,
        large_body_slots: usize,
    ) -> Self {
        Self {
            git_read_bytes: small_body_bytes,
            git_write_bytes: large_body_bytes,
            binary_upload_bytes: large_body_bytes,
            edit_form_bytes: small_body_bytes,
            small_body_slots: Arc::new(Semaphore::new(small_body_slots)),
            large_body_slots: Arc::new(Semaphore::new(large_body_slots)),
        }
    }

    pub fn git_read_bytes(&self) -> usize {
        self.git_read_bytes
    }

    pub fn git_write_bytes(&self) -> usize {
        self.git_write_bytes
    }

    pub fn binary_upload_bytes(&self) -> usize {
        self.binary_upload_bytes
    }

    pub fn edit_form_bytes(&self) -> usize {
        self.edit_form_bytes
    }

    pub async fn read_payload(
        &self,
        payload: web::Payload,
        class: BodyClass,
        max_bytes: usize,
    ) -> Result<BoundedBody, BodyReadError> {
        let semaphore = match class {
            BodyClass::Small => &self.small_body_slots,
            BodyClass::Large => &self.large_body_slots,
        };
        let permit = Arc::clone(semaphore)
            .acquire_owned()
            .await
            .map_err(|_| BodyReadError::CapacityClosed)?;

        let bytes = match payload.to_bytes_limited(max_bytes).await {
            Err(_) => return Err(BodyReadError::TooLarge),
            Ok(Err(error)) => return Err(BodyReadError::Payload(error)),
            Ok(Ok(bytes)) => bytes,
        };

        Ok(BoundedBody {
            bytes: bytes.to_vec(),
            _permit: permit,
        })
    }

    pub fn declared_length_exceeds(request: &HttpRequest, max_bytes: usize) -> bool {
        request
            .headers()
            .get(actix_web::http::header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .is_some_and(|length| length > max_bytes as u64)
    }

    pub async fn extract_authorized_form<T>(
        &self,
        request: &HttpRequest,
        payload: web::Payload,
    ) -> Result<BoundedForm<T>, actix_web::HttpResponse>
    where
        T: DeserializeOwned + 'static,
    {
        let max_bytes = self.edit_form_bytes();
        if Self::declared_length_exceeds(request, max_bytes) {
            return Err(actix_web::HttpResponse::PayloadTooLarge().body(format!(
                "Repository edit form exceeds the {max_bytes}-byte limit"
            )));
        }
        let body = self
            .read_payload(payload, BodyClass::Small, max_bytes)
            .await
            .map_err(|error| match error {
                BodyReadError::TooLarge => actix_web::HttpResponse::PayloadTooLarge().body(
                    format!("Repository edit form exceeds the {max_bytes}-byte limit"),
                ),
                BodyReadError::Payload(error) => error.as_response_error().error_response(),
                BodyReadError::CapacityClosed => actix_web::HttpResponse::ServiceUnavailable()
                    .body("Request body capacity is unavailable"),
            })?;

        let BoundedBody {
            bytes,
            _permit: permit,
        } = body;
        let mut payload = actix_web::dev::Payload::from(bytes);
        let form = web::Form::<T>::from_request(request, &mut payload)
            .await
            .map_err(|error| error.as_response_error().error_response())?;

        Ok(BoundedForm {
            value: form.into_inner(),
            _permit: permit,
        })
    }
}

impl Default for RequestBodyLimits {
    fn default() -> Self {
        Self {
            git_read_bytes: DEFAULT_SMALL_BODY_LIMIT,
            git_write_bytes: DEFAULT_LARGE_BODY_LIMIT,
            binary_upload_bytes: DEFAULT_LARGE_BODY_LIMIT,
            edit_form_bytes: DEFAULT_SMALL_BODY_LIMIT,
            small_body_slots: Arc::new(Semaphore::new(DEFAULT_SMALL_BODY_SLOTS)),
            large_body_slots: Arc::new(Semaphore::new(DEFAULT_LARGE_BODY_SLOTS)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::FromRequest;

    async fn payload_from_bytes(bytes: &'static [u8]) -> web::Payload {
        let mut payload = actix_web::dev::Payload::from(bytes.to_vec());
        let request = actix_web::test::TestRequest::default().to_http_request();
        web::Payload::from_request(&request, &mut payload)
            .await
            .expect("payload extractor succeeds")
    }

    #[actix_web::test]
    async fn body_collector_accepts_exact_limit_and_rejects_one_byte_over() {
        let limits = RequestBodyLimits::for_tests(4, 8, 1, 1);
        let exact = limits
            .read_payload(payload_from_bytes(b"1234").await, BodyClass::Small, 4)
            .await
            .expect("body at the limit is accepted");
        assert_eq!(exact.bytes, b"1234");
        drop(exact);

        assert!(matches!(
            limits
                .read_payload(payload_from_bytes(b"12345").await, BodyClass::Small, 4)
                .await,
            Err(BodyReadError::TooLarge)
        ));
    }

    #[test]
    fn declared_content_length_is_checked_without_reading_body() {
        let request = actix_web::test::TestRequest::default()
            .insert_header(("Content-Length", "9"))
            .to_http_request();
        assert!(RequestBodyLimits::declared_length_exceeds(&request, 8));
        assert!(!RequestBodyLimits::declared_length_exceeds(&request, 9));
    }

    #[tokio::test]
    async fn large_body_slots_are_shared_and_small_body_slots_are_independent() {
        let limits = RequestBodyLimits::for_tests(8, 8, 1, 1);
        // Git pushes and binary uploads both use the same large-body class.
        let git_push_body = limits
            .read_payload(payload_from_bytes(b"large").await, BodyClass::Large, 8)
            .await
            .unwrap();
        assert!(limits.large_body_slots.clone().try_acquire_owned().is_err());

        // A large operation does not consume the independent small-body slot.
        let small = limits
            .read_payload(payload_from_bytes(b"small").await, BodyClass::Small, 8)
            .await
            .expect("small requests have an independent slot pool");
        assert!(limits.small_body_slots.clone().try_acquire_owned().is_err());
        drop(small);
        assert!(limits.small_body_slots.clone().try_acquire_owned().is_ok());

        drop(git_push_body);
        assert!(limits.large_body_slots.clone().try_acquire_owned().is_ok());
    }
}
