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

//! Webhook payload parser.
//!
//! Pakasir delivers webhook events as a JSON `POST` body. This module turns
//! that body into an [`Event`] and rejects payloads that are missing, too
//! large, or malformed. Use [`Parser::parse_reader`] for streaming sources
//! (e.g. the body of an HTTP handler) and [`Parser::parse_bytes`] for
//! payloads you already have in memory.
//!
//! The parser caps the body it will read at [`DEFAULT_MAX_BODY_SIZE`]
//! (1 MiB) to keep a misbehaving sender from forcing the process to buffer
//! arbitrary amounts. Override with [`Parser::with_max_body_size`].

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::Read;

use crate::constants::{PaymentMethod, TransactionStatus};
use crate::timefmt;

/// Default cap on a single webhook body, in bytes (1 MiB).
pub const DEFAULT_MAX_BODY_SIZE: usize = 1 << 20;

/// Things that can go wrong while parsing a webhook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookError {
    /// Reader is missing where one was expected.
    ///
    /// Kept for parity with the original API surface — the typed
    /// [`Parser::parse_reader`] signature makes it impossible to hit from
    /// safe Rust code today.
    NilReader,
    /// The body was zero bytes.
    EmptyBody,
    /// The body grew past the configured size cap.
    BodyTooLarge {
        /// The cap that was exceeded.
        limit: usize,
    },
    /// Reading from the supplied source failed. `String` is the underlying
    /// IO error message, kept by value so [`WebhookError`] can stay
    /// `Clone + PartialEq`.
    ReadBody(String),
    /// JSON decoding failed. `String` is the underlying `serde_json` error
    /// message, for the same reason as above.
    DecodeBody(String),
    /// [`Event::validate`] saw an empty `order_id`.
    InvalidOrderId,
    /// [`Event::validate`] saw a non-positive `amount`.
    InvalidAmount,
}

impl fmt::Display for WebhookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NilReader => f.write_str("webhook: nil reader"),
            Self::EmptyBody => f.write_str("webhook: empty body"),
            Self::BodyTooLarge { limit } => {
                write!(f, "webhook: body too large: exceeds {limit} bytes")
            }
            Self::ReadBody(message) => write!(f, "webhook: read body failed: {message}"),
            Self::DecodeBody(message) => write!(f, "webhook: decode body failed: {message}"),
            Self::InvalidOrderId => f.write_str("webhook: invalid order id"),
            Self::InvalidAmount => f.write_str("webhook: invalid amount"),
        }
    }
}

impl std::error::Error for WebhookError {}

/// Reusable parser carrying the configured body size limit.
#[derive(Debug, Clone)]
pub struct Parser {
    max_body_size: usize,
}

impl Default for Parser {
    fn default() -> Self {
        Self {
            max_body_size: DEFAULT_MAX_BODY_SIZE,
        }
    }
}

impl Parser {
    /// New parser with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the body size limit. Zero is ignored so the default is
    /// kept.
    pub fn with_max_body_size(mut self, max_body_size: usize) -> Self {
        if max_body_size > 0 {
            self.max_body_size = max_body_size;
        }
        self
    }

    /// Read at most `max_body_size + 1` bytes from `reader`, then parse.
    ///
    /// The extra byte is what lets us detect "body grew past the limit"
    /// reliably: if the read came back with more than the limit, we know
    /// the sender had more to give and reject with
    /// [`WebhookError::BodyTooLarge`] before touching `serde_json`.
    pub fn parse_reader<R>(&self, mut reader: R) -> Result<Event, WebhookError>
    where
        R: Read,
    {
        let mut limited = (&mut reader).take((self.max_body_size + 1) as u64);
        let mut data = Vec::new();
        limited
            .read_to_end(&mut data)
            .map_err(|err| WebhookError::ReadBody(err.to_string()))?;

        if data.len() > self.max_body_size {
            return Err(WebhookError::BodyTooLarge {
                limit: self.max_body_size,
            });
        }
        if data.is_empty() {
            return Err(WebhookError::EmptyBody);
        }

        self.parse_bytes(&data)
    }

    /// Parse `data` directly. Use this when you already have the body in
    /// memory (e.g. a framework that buffered it for you).
    pub fn parse_bytes(&self, data: &[u8]) -> Result<Event, WebhookError> {
        if data.is_empty() {
            return Err(WebhookError::EmptyBody);
        }

        serde_json::from_slice(data).map_err(|err| WebhookError::DecodeBody(err.to_string()))
    }
}

/// Decoded webhook event.
///
/// Shape matches the JSON body Pakasir sends; field order in this struct is
/// purely cosmetic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// Transaction amount.
    pub amount: i64,
    /// Order identifier.
    pub order_id: String,
    /// Project slug the transaction belongs to.
    pub project: String,
    /// Lifecycle status at the moment the webhook fired.
    pub status: TransactionStatus,
    /// Payment method used for the transaction.
    pub payment_method: PaymentMethod,
    /// RFC 3339 completion timestamp.
    pub completed_at: String,
    /// `true` when this event came from a sandbox project.
    pub is_sandbox: bool,
}

impl Event {
    /// Parse [`Event::completed_at`] into a [`DateTime`].
    pub fn parse_time(&self) -> Result<DateTime<FixedOffset>, chrono::ParseError> {
        timefmt::parse_rfc3339(&self.completed_at)
    }

    /// Sanity-check the fields most handlers rely on.
    ///
    /// Returns [`WebhookError::InvalidOrderId`] for an empty `order_id` and
    /// [`WebhookError::InvalidAmount`] for a non-positive `amount`. Other
    /// fields are not inspected.
    pub fn validate(&self) -> Result<(), WebhookError> {
        if self.order_id.is_empty() {
            return Err(WebhookError::InvalidOrderId);
        }
        if self.amount <= 0 {
            return Err(WebhookError::InvalidAmount);
        }
        Ok(())
    }
}
