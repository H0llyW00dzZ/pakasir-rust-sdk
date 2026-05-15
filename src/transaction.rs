// Copyright 2026 H0llyW00dzZ
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Transaction service: create, cancel, and detail.
//!
//! All three operations share the same validation rules (`order_id` must
//! not be empty, `amount` must be positive) and the same wire body shape;
//! the helpers at the bottom of this module centralize both.
//!
//! `detail` is the odd one out: it uses `GET` and puts everything,
//! including the API key, on the query string because that is what the
//! upstream API requires.

use chrono::{DateTime, FixedOffset};
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::constants::{
    PATH_TRANSACTION_CANCEL, PATH_TRANSACTION_CREATE, PATH_TRANSACTION_DETAIL, PaymentMethod,
    TransactionStatus,
};
use crate::error::{Error, Result};
use crate::timefmt;

/// Service handle wrapping a [`Client`]. Cheap to clone.
#[derive(Debug, Clone)]
pub struct TransactionService {
    client: Client,
}

/// Input for [`TransactionService::create`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateRequest {
    /// Merchant-side order identifier.
    pub order_id: String,
    /// Amount in the smallest currency unit (rupiah). Must be positive.
    pub amount: i64,
}

/// Response of [`TransactionService::create`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateResponse {
    /// Payment information returned by the API.
    pub payment: PaymentInfo,
}

/// Body of [`CreateResponse`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentInfo {
    /// Project slug echoed back by the API.
    pub project: String,
    /// Order identifier echoed back by the API.
    pub order_id: String,
    /// Amount before fee.
    pub amount: i64,
    /// Gateway fee, in the same units as `amount`.
    pub fee: i64,
    /// `amount + fee`. The number the customer actually has to pay.
    pub total_payment: i64,
    /// Payment method used for this transaction.
    pub payment_method: PaymentMethod,
    /// Gateway-issued reference (VA number, QRIS payload, …).
    pub payment_number: String,
    /// RFC 3339 expiration timestamp as returned by the API.
    pub expired_at: String,
}

impl PaymentInfo {
    /// Parse [`PaymentInfo::expired_at`] into a [`DateTime`].
    pub fn parse_time(&self) -> std::result::Result<DateTime<FixedOffset>, chrono::ParseError> {
        timefmt::parse_rfc3339(&self.expired_at)
    }
}

/// Input for [`TransactionService::cancel`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelRequest {
    /// Order identifier to cancel.
    pub order_id: String,
    /// Amount of the transaction being cancelled. Must match the original.
    pub amount: i64,
}

/// Input for [`TransactionService::detail`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailRequest {
    /// Order identifier to look up.
    pub order_id: String,
    /// Amount of the transaction being looked up.
    pub amount: i64,
}

/// Response of [`TransactionService::detail`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailResponse {
    /// Transaction details returned by the API.
    pub transaction: TransactionInfo,
}

/// Body of [`DetailResponse`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionInfo {
    /// Transaction amount.
    pub amount: i64,
    /// Order identifier echoed back by the API.
    pub order_id: String,
    /// Project slug echoed back by the API.
    pub project: String,
    /// Lifecycle status.
    pub status: TransactionStatus,
    /// Payment method used for this transaction.
    pub payment_method: PaymentMethod,
    /// RFC 3339 completion timestamp.
    ///
    /// Only populated once the transaction has actually been paid. The
    /// upstream API omits the field (or sends `null`) for `pending`,
    /// `expired`, and cancelled transactions, so it is modelled as
    /// optional. Use [`TransactionInfo::parse_time`] to obtain a parsed
    /// [`DateTime`].
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub completed_at: Option<String>,
}

impl TransactionInfo {
    /// Parse [`TransactionInfo::completed_at`] into a [`DateTime`].
    ///
    /// Returns `None` when the API did not include a completion timestamp
    /// (e.g. for `pending` or `expired` transactions). When a timestamp is
    /// present, the inner `Result` carries any parse failure.
    pub fn parse_time(
        &self,
    ) -> Option<std::result::Result<DateTime<FixedOffset>, chrono::ParseError>> {
        self.completed_at.as_deref().map(timefmt::parse_rfc3339)
    }
}

/// Treat both a missing field and an explicit `null` as `None` while still
/// accepting a regular string value. The upstream API has been observed to
/// use both shapes for absent timestamps.
fn deserialize_optional_string<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)
}

/// Wire-format body. Project / API key come from the client so callers
/// don't have to repeat them on every request.
#[derive(Debug, Serialize)]
struct RequestBody<'a> {
    project: &'a str,
    order_id: &'a str,
    amount: i64,
    api_key: &'a str,
}

impl TransactionService {
    /// Wrap an existing [`Client`].
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Create a transaction with the given payment `method`.
    ///
    /// The endpoint is `POST /api/transactioncreate/{method}` and the
    /// `method` segment uses the wire identifier from
    /// [`PaymentMethod::as_str`].
    pub async fn create(
        &self,
        method: PaymentMethod,
        request: &CreateRequest,
    ) -> Result<CreateResponse> {
        validate_request(self.client.language(), &request.order_id, request.amount)?;

        let body = encode_body(
            self.client.language(),
            &RequestBody {
                project: self.client.project(),
                order_id: &request.order_id,
                amount: request.amount,
                api_key: self.client.api_key(),
            },
        )?;

        let path = format!("{PATH_TRANSACTION_CREATE}/{}", method.as_str());
        let bytes = self
            .client
            .do_request(Method::POST, &path, Some(body))
            .await?;
        serde_json::from_slice(&bytes)
            .map_err(|err| Error::decode_json(self.client.language(), err))
    }

    /// Cancel a pending transaction.
    ///
    /// `POST /api/transactioncancel`. The endpoint returns 200 with no
    /// useful body on success, so this just returns `()`.
    pub async fn cancel(&self, request: &CancelRequest) -> Result<()> {
        validate_request(self.client.language(), &request.order_id, request.amount)?;

        let body = encode_body(
            self.client.language(),
            &RequestBody {
                project: self.client.project(),
                order_id: &request.order_id,
                amount: request.amount,
                api_key: self.client.api_key(),
            },
        )?;

        self.client
            .do_request(Method::POST, PATH_TRANSACTION_CANCEL, Some(body))
            .await
            .map(|_| ())
    }

    /// Look up a transaction.
    ///
    /// `GET /api/transactiondetail?project=…&amount=…&order_id=…&api_key=…`.
    /// All parameters go on the query string, the API key included. This
    /// matches the upstream API shape; the SDK does not transform it.
    pub async fn detail(&self, request: &DetailRequest) -> Result<DetailResponse> {
        validate_request(self.client.language(), &request.order_id, request.amount)?;

        let path = format!(
            "{PATH_TRANSACTION_DETAIL}?project={}&amount={}&order_id={}&api_key={}",
            urlencoding(self.client.project()),
            request.amount,
            urlencoding(&request.order_id),
            urlencoding(self.client.api_key()),
        );

        let bytes = self.client.do_request(Method::GET, &path, None).await?;
        serde_json::from_slice(&bytes)
            .map_err(|err| Error::decode_json(self.client.language(), err))
    }
}

/// Shared validation for `order_id` and `amount`. Returns the matching
/// localized [`Error`] variant on the first failure.
fn validate_request(language: crate::i18n::Language, order_id: &str, amount: i64) -> Result<()> {
    if order_id.is_empty() {
        return Err(Error::invalid_order_id(language));
    }
    if amount <= 0 {
        return Err(Error::invalid_amount(language));
    }
    Ok(())
}

/// Serialize a request body to JSON, wrapping any failure into a localized
/// [`Error::EncodeJson`].
fn encode_body<T>(language: crate::i18n::Language, value: &T) -> Result<Vec<u8>>
where
    T: Serialize,
{
    serde_json::to_vec(value).map_err(|err| Error::encode_json(language, err))
}

/// Percent-encode a single query-string value.
fn urlencoding(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Language;

    fn dummy_client(language: Language) -> Client {
        Client::builder("project", "key")
            .base_url("http://127.0.0.1:1")
            .retries(0)
            .language(language)
            .build()
    }

    #[test]
    fn validate_request_accepts_well_formed_input() {
        validate_request(Language::English, "INV1", 1).unwrap();
    }

    #[test]
    fn validate_request_rejects_empty_order_id() {
        let err = validate_request(Language::English, "", 1).unwrap_err();
        assert!(matches!(err, Error::InvalidOrderId { .. }));

        let err = validate_request(Language::Indonesian, "", 1).unwrap_err();
        assert_eq!(err.to_string(), "ID pesanan wajib diisi");
    }

    #[test]
    fn validate_request_rejects_non_positive_amount() {
        for amount in [0_i64, -1, i64::MIN] {
            let err = validate_request(Language::English, "x", amount).unwrap_err();
            assert!(
                matches!(err, Error::InvalidAmount { .. }),
                "amount={amount}"
            );
        }
    }

    #[test]
    fn encode_body_serializes_request_body_struct() {
        let bytes = encode_body(
            Language::English,
            &RequestBody {
                project: "p",
                order_id: "INV1",
                amount: 5_000,
                api_key: "k",
            },
        )
        .unwrap();
        let decoded: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded["project"], "p");
        assert_eq!(decoded["order_id"], "INV1");
        assert_eq!(decoded["amount"], 5_000);
        assert_eq!(decoded["api_key"], "k");
    }

    #[test]
    fn urlencoding_escapes_unsafe_chars() {
        assert_eq!(urlencoding("a b/c"), "a+b%2Fc");
        assert_eq!(urlencoding("plain"), "plain");
        assert_eq!(urlencoding("@#$&="), "%40%23%24%26%3D");
    }

    #[test]
    fn payment_info_parse_time_parses_iso8601() {
        let info = PaymentInfo {
            project: "p".into(),
            order_id: "INV1".into(),
            amount: 1,
            fee: 0,
            total_payment: 1,
            payment_method: PaymentMethod::Qris,
            payment_number: "x".into(),
            expired_at: "2026-12-25T12:00:00+07:00".into(),
        };
        let parsed = info.parse_time().unwrap();
        assert_eq!(parsed.to_rfc3339(), "2026-12-25T12:00:00+07:00");
    }

    #[test]
    fn payment_info_parse_time_surfaces_parse_errors() {
        let info = PaymentInfo {
            project: "p".into(),
            order_id: "INV1".into(),
            amount: 1,
            fee: 0,
            total_payment: 1,
            payment_method: PaymentMethod::Qris,
            payment_number: "x".into(),
            expired_at: "not a date".into(),
        };
        assert!(info.parse_time().is_err());
    }

    #[test]
    fn transaction_info_parse_time_parses_iso8601() {
        let info = TransactionInfo {
            amount: 1,
            order_id: "INV1".into(),
            project: "p".into(),
            status: TransactionStatus::Completed,
            payment_method: PaymentMethod::Qris,
            completed_at: Some("2026-12-25T12:00:00+07:00".into()),
        };
        let parsed = info.parse_time().unwrap().unwrap();
        assert_eq!(parsed.to_rfc3339(), "2026-12-25T12:00:00+07:00");
    }

    #[test]
    fn transaction_info_parse_time_returns_none_when_completed_at_missing() {
        let info = TransactionInfo {
            amount: 1,
            order_id: "INV1".into(),
            project: "p".into(),
            status: TransactionStatus::Pending,
            payment_method: PaymentMethod::Qris,
            completed_at: None,
        };
        assert!(info.parse_time().is_none());
    }

    #[test]
    fn transaction_info_deserializes_with_missing_completed_at() {
        // Pending / expired transactions arrive without `completed_at`.
        let payload = br#"{"amount":1,"order_id":"INV1","project":"p","status":"pending","payment_method":"qris"}"#;
        let info: TransactionInfo = serde_json::from_slice(payload).unwrap();
        assert_eq!(info.completed_at, None);
        assert!(info.parse_time().is_none());
    }

    #[test]
    fn transaction_info_deserializes_with_null_completed_at() {
        let payload = br#"{"amount":1,"order_id":"INV1","project":"p","status":"pending","payment_method":"qris","completed_at":null}"#;
        let info: TransactionInfo = serde_json::from_slice(payload).unwrap();
        assert_eq!(info.completed_at, None);
    }

    #[tokio::test]
    async fn create_rejects_invalid_input_before_network() {
        let service = TransactionService::new(dummy_client(Language::English));
        let err = service
            .create(
                PaymentMethod::Qris,
                &CreateRequest {
                    order_id: String::new(),
                    amount: 1,
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::InvalidOrderId { .. }));
    }

    #[tokio::test]
    async fn cancel_rejects_invalid_input_before_network() {
        let service = TransactionService::new(dummy_client(Language::English));
        let err = service
            .cancel(&CancelRequest {
                order_id: "x".into(),
                amount: 0,
            })
            .await
            .unwrap_err();
        assert!(matches!(err, Error::InvalidAmount { .. }));
    }

    #[tokio::test]
    async fn detail_rejects_invalid_input_before_network() {
        let service = TransactionService::new(dummy_client(Language::English));
        let err = service
            .detail(&DetailRequest {
                order_id: String::new(),
                amount: 1,
            })
            .await
            .unwrap_err();
        assert!(matches!(err, Error::InvalidOrderId { .. }));
    }

    #[test]
    fn transaction_service_is_clone() {
        let service = TransactionService::new(dummy_client(Language::English));
        let _ = service.clone();
    }
}
