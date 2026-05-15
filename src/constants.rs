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

//! SDK metadata, API paths, and the enums shared across the crate.
//!
//! This module is kept dependency-free on purpose so every other module can
//! pull it in without worrying about cycles. It contains:
//!
//! - Identifying constants ([`SDK_NAME`], [`SDK_VERSION`],
//!   [`SDK_REPOSITORY`]) and the [`user_agent`] helper that stitches them
//!   together.
//! - The Pakasir API path constants. Service modules reference these so
//!   endpoint strings don't appear as magic literals at the call site.
//! - [`PaymentMethod`] and [`TransactionStatus`] enums. Their serde tags
//!   match the wire format exactly.

use serde::{Deserialize, Serialize};

/// Crate name, shown in the SDK user-agent string.
pub const SDK_NAME: &str = "pakasir-sdk";
/// Crate version, pulled from `Cargo.toml` at compile time.
pub const SDK_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Repository URL advertised in the user-agent string.
pub const SDK_REPOSITORY: &str = "https://github.com/H0llyW00dzZ/pakasir-rust-sdk";

/// "Create transaction" endpoint.
///
/// The transaction service appends `/{payment_method}` (e.g.
/// `/api/transactioncreate/qris`) before sending the request.
pub const PATH_TRANSACTION_CREATE: &str = "/api/transactioncreate";
/// "Cancel transaction" endpoint.
pub const PATH_TRANSACTION_CANCEL: &str = "/api/transactioncancel";
/// "Transaction detail" endpoint.
///
/// This one is `GET` and takes its parameters (including `api_key`) on the
/// query string, which is why the transaction service builds the query
/// inline instead of sending a JSON body.
pub const PATH_TRANSACTION_DETAIL: &str = "/api/transactiondetail";
/// Sandbox "payment simulation" endpoint.
pub const PATH_PAYMENT_SIMULATION: &str = "/api/paymentsimulation";

/// User-agent string sent on every request.
///
/// Format: `"<name>/<version> (+<repository>)"`, e.g.
/// `pakasir-sdk/0.1.0 (+https://github.com/H0llyW00dzZ/pakasir-rust-sdk)`.
pub fn user_agent() -> String {
    format!("{SDK_NAME}/{SDK_VERSION} (+{SDK_REPOSITORY})")
}

/// Payment methods accepted by the Pakasir API.
///
/// The serde rename tags pin the JSON values to the snake_case slugs the API
/// uses, so the Rust names can stay idiomatic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymentMethod {
    /// CIMB Niaga virtual account (`cimb_niaga_va`).
    #[serde(rename = "cimb_niaga_va")]
    CimbNiagaVa,
    /// BNI virtual account (`bni_va`).
    #[serde(rename = "bni_va")]
    BniVa,
    /// QRIS dynamic QR (`qris`).
    #[serde(rename = "qris")]
    Qris,
    /// Sampoerna virtual account (`sampoerna_va`).
    #[serde(rename = "sampoerna_va")]
    SampoernaVa,
    /// Bank Neo Commerce virtual account (`bnc_va`).
    #[serde(rename = "bnc_va")]
    BncVa,
    /// Maybank virtual account (`maybank_va`).
    #[serde(rename = "maybank_va")]
    MaybankVa,
    /// Permata virtual account (`permata_va`).
    #[serde(rename = "permata_va")]
    PermataVa,
    /// ATM Bersama virtual account (`atm_bersama_va`).
    #[serde(rename = "atm_bersama_va")]
    AtmBersamaVa,
    /// Artha Graha virtual account (`artha_graha_va`).
    #[serde(rename = "artha_graha_va")]
    ArthaGrahaVa,
    /// BRI virtual account (`bri_va`).
    #[serde(rename = "bri_va")]
    BriVa,
    /// PayPal (`paypal`).
    ///
    /// **Unofficial:** not listed in the public Pakasir documentation at
    /// <https://pakasir.com/p/docs>. Kept for parity with the upstream Go
    /// SDK. Requests using this method may be rejected by the live API.
    #[serde(rename = "paypal")]
    Paypal,
}

impl PaymentMethod {
    /// The wire identifier (e.g. `qris`, `bni_va`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CimbNiagaVa => "cimb_niaga_va",
            Self::BniVa => "bni_va",
            Self::Qris => "qris",
            Self::SampoernaVa => "sampoerna_va",
            Self::BncVa => "bnc_va",
            Self::MaybankVa => "maybank_va",
            Self::PermataVa => "permata_va",
            Self::AtmBersamaVa => "atm_bersama_va",
            Self::ArthaGrahaVa => "artha_graha_va",
            Self::BriVa => "bri_va",
            Self::Paypal => "paypal",
        }
    }
}

impl core::fmt::Display for PaymentMethod {
    /// Formats as the wire identifier returned by [`PaymentMethod::as_str`].
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Lifecycle status of a transaction.
///
/// Both `cancelled` and `canceled` are recognized — the upstream API has
/// used either spelling in the past.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionStatus {
    /// Payment completed successfully.
    #[serde(rename = "completed")]
    Completed,
    /// Payment created and still waiting for funds.
    #[serde(rename = "pending")]
    Pending,
    /// Payment window elapsed without completion.
    #[serde(rename = "expired")]
    Expired,
    /// Payment was cancelled (British spelling).
    #[serde(rename = "cancelled")]
    Cancelled,
    /// Payment was cancelled (American spelling).
    #[serde(rename = "canceled")]
    Canceled,
}

impl TransactionStatus {
    /// The wire identifier (e.g. `completed`, `pending`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Pending => "pending",
            Self::Expired => "expired",
            Self::Cancelled => "cancelled",
            Self::Canceled => "canceled",
        }
    }
}

impl core::fmt::Display for TransactionStatus {
    /// Formats as the wire identifier returned by
    /// [`TransactionStatus::as_str`].
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_agent_has_expected_shape() {
        let ua = user_agent();
        assert!(ua.starts_with(&format!("{SDK_NAME}/{SDK_VERSION}")));
        assert!(ua.contains(SDK_REPOSITORY));
        assert!(ua.ends_with(")"));
    }

    #[test]
    fn payment_method_as_str_covers_every_variant() {
        let cases = [
            (PaymentMethod::CimbNiagaVa, "cimb_niaga_va"),
            (PaymentMethod::BniVa, "bni_va"),
            (PaymentMethod::Qris, "qris"),
            (PaymentMethod::SampoernaVa, "sampoerna_va"),
            (PaymentMethod::BncVa, "bnc_va"),
            (PaymentMethod::MaybankVa, "maybank_va"),
            (PaymentMethod::PermataVa, "permata_va"),
            (PaymentMethod::AtmBersamaVa, "atm_bersama_va"),
            (PaymentMethod::ArthaGrahaVa, "artha_graha_va"),
            (PaymentMethod::BriVa, "bri_va"),
            (PaymentMethod::Paypal, "paypal"),
        ];
        for (variant, wire) in cases {
            assert_eq!(variant.as_str(), wire);
            assert_eq!(format!("{variant}"), wire);
        }
    }

    #[test]
    fn transaction_status_as_str_covers_every_variant() {
        let cases = [
            (TransactionStatus::Completed, "completed"),
            (TransactionStatus::Pending, "pending"),
            (TransactionStatus::Expired, "expired"),
            (TransactionStatus::Cancelled, "cancelled"),
            (TransactionStatus::Canceled, "canceled"),
        ];
        for (variant, wire) in cases {
            assert_eq!(variant.as_str(), wire);
            assert_eq!(format!("{variant}"), wire);
        }
    }
}
