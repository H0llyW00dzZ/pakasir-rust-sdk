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

//! Crate-wide error type.
//!
//! Everything fallible in this SDK ends up as an [`enum@Error`]. The
//! variants are kept flat so callers can match on them without juggling
//! nested types.
//!
//! Most variants carry a pre-localized `message` so a plain `{err}` print is
//! already in the configured language. Variants that wrap an underlying
//! error use `#[source]` so [`std::error::Error::source`] walks the chain
//! cleanly.

use reqwest::StatusCode;
use std::error::Error as StdError;
use thiserror::Error;

use crate::i18n::{self, Language, MessageKey};

/// Boxed error used internally to keep retry plumbing object-safe.
pub type BoxError = Box<dyn StdError + Send + Sync>;
/// Convenience `Result` alias bound to [`enum@Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// Every error the SDK can produce.
#[derive(Debug, Error)]
pub enum Error {
    /// The project slug is empty. Triggered by [`crate::Client::do_request`]
    /// before any network call.
    #[error("{message}")]
    InvalidProject { message: String },
    /// The API key is empty. Triggered by [`crate::Client::do_request`]
    /// before any network call.
    #[error("{message}")]
    InvalidApiKey { message: String },
    /// `amount` was zero or negative.
    #[error("{message}")]
    InvalidAmount { message: String },
    /// `order_id` was empty.
    #[error("{message}")]
    InvalidOrderId { message: String },
    /// A payment method outside the supported set was used. Currently unused
    /// because the typed [`crate::PaymentMethod`] enum makes invalid values
    /// unrepresentable, but kept here so future runtime checks have a slot
    /// to land in.
    #[error("{message}")]
    InvalidPaymentMethod { message: String },
    /// `serde_json` refused to encode the request body.
    #[error("{message}: {source}")]
    EncodeJson {
        message: String,
        #[source]
        source: serde_json::Error,
    },
    /// `serde_json` refused to decode the response body.
    #[error("{message}: {source}")]
    DecodeJson {
        message: String,
        #[source]
        source: serde_json::Error,
    },
    /// The configured base URL combined with a path did not parse as a URL.
    #[error("client: failed to create request: {source}")]
    BuildRequest {
        #[source]
        source: url::ParseError,
    },
    /// The API returned a non-2xx response.
    ///
    /// `body` is the raw response body decoded as UTF-8 with lossy
    /// substitution. Use [`Error::api_status`] to read the status code
    /// programmatically.
    #[error("pakasir api error: status {status}: {body}")]
    Api { status: StatusCode, body: String },
    /// A request hit a permanent transport-level failure (TLS, invalid URL
    /// after the builder accepted it, response too large, …).
    #[error("{message}: {source}")]
    RequestFailed {
        message: String,
        #[source]
        source: BoxError,
    },
    /// The retry loop ran out of attempts. `source` is the last transient
    /// failure observed.
    #[error("{message}: {source}")]
    RequestFailedAfterRetries {
        message: String,
        #[source]
        source: BoxError,
    },
    /// The response body exceeded the configured size cap. Returned without
    /// fully buffering the offending response.
    #[error("response body too large: exceeds {limit} bytes")]
    ResponseTooLarge { limit: usize },
}

impl Error {
    /// Build an [`Error::InvalidProject`] in the given language.
    pub(crate) fn invalid_project(lang: Language) -> Self {
        Self::InvalidProject {
            message: i18n::get(lang, MessageKey::InvalidProject).to_owned(),
        }
    }

    /// Build an [`Error::InvalidApiKey`] in the given language.
    pub(crate) fn invalid_api_key(lang: Language) -> Self {
        Self::InvalidApiKey {
            message: i18n::get(lang, MessageKey::InvalidApiKey).to_owned(),
        }
    }

    /// Build an [`Error::InvalidAmount`] in the given language.
    pub(crate) fn invalid_amount(lang: Language) -> Self {
        Self::InvalidAmount {
            message: i18n::get(lang, MessageKey::InvalidAmount).to_owned(),
        }
    }

    /// Build an [`Error::InvalidOrderId`] in the given language.
    pub(crate) fn invalid_order_id(lang: Language) -> Self {
        Self::InvalidOrderId {
            message: i18n::get(lang, MessageKey::InvalidOrderId).to_owned(),
        }
    }

    /// Build an [`Error::EncodeJson`] from a `serde_json` failure.
    pub(crate) fn encode_json(lang: Language, source: serde_json::Error) -> Self {
        Self::EncodeJson {
            message: i18n::get(lang, MessageKey::FailedToEncode).to_owned(),
            source,
        }
    }

    /// Build an [`Error::DecodeJson`] from a `serde_json` failure.
    pub(crate) fn decode_json(lang: Language, source: serde_json::Error) -> Self {
        Self::DecodeJson {
            message: i18n::get(lang, MessageKey::FailedToDecode).to_owned(),
            source,
        }
    }

    /// Wrap a permanent transport failure in [`Error::RequestFailed`].
    pub(crate) fn request_failed(lang: Language, source: BoxError) -> Self {
        Self::RequestFailed {
            message: i18n::get(lang, MessageKey::RequestFailedPermanent).to_owned(),
            source,
        }
    }

    /// Wrap "retries exhausted" in [`Error::RequestFailedAfterRetries`].
    ///
    /// The `%d` placeholder in the localized template is replaced with the
    /// configured retry count.
    pub(crate) fn request_failed_after_retries(
        lang: Language,
        retries: usize,
        source: BoxError,
    ) -> Self {
        let template = i18n::get(lang, MessageKey::RequestFailedAfterRetries);
        let message = template.replacen("%d", &retries.to_string(), 1);
        Self::RequestFailedAfterRetries { message, source }
    }

    /// Return the HTTP status when this is an [`Error::Api`], otherwise
    /// `None`. Handy for branch logic without having to `match` on the full
    /// enum.
    pub fn api_status(&self) -> Option<StatusCode> {
        match self {
            Self::Api { status, .. } => Some(*status),
            _ => None,
        }
    }
}
