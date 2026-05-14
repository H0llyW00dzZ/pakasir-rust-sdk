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

//! Unofficial Rust SDK for the [Pakasir](https://pakasir.com) payment gateway.
//!
//! The crate covers the REST surface of the API plus a few helpers most
//! integrations end up writing anyway:
//!
//! - [`Client`] – async HTTP client with retries, jittered backoff, and
//!   `Retry-After` handling.
//! - [`TransactionService`] – create / cancel / detail.
//! - [`SimulationService`] – sandbox payment simulation.
//! - [`WebhookParser`] – parse and validate webhook payloads.
//! - [`build_payment_url`] – build the hosted checkout redirect URL.
//! - [`QrGenerator`] – render QRIS strings (or anything else) to PNG bytes.
//!
//! Error messages are localized through [`Language`] (English by default,
//! Indonesian when the client is configured for it).
//!
//! # Quick start
//!
//! ```no_run
//! use pakasir_sdk::{Client, PaymentMethod, TransactionService};
//! use pakasir_sdk::transaction::CreateRequest;
//!
//! # async fn run() -> Result<(), pakasir_sdk::Error> {
//! let client = Client::builder("your-project-slug", "your-api-key").build();
//! let transactions = TransactionService::new(client);
//!
//! let response = transactions
//!     .create(
//!         PaymentMethod::Qris,
//!         &CreateRequest {
//!             order_id: "INV123456".into(),
//!             amount: 99_000,
//!         },
//!     )
//!     .await?;
//!
//! println!("payment number: {}", response.payment.payment_number);
//! println!("total payment:  {}", response.payment.total_payment);
//! # Ok(())
//! # }
//! ```
//!
//! # Modules
//!
//! | Module          | What it does                                                  |
//! |-----------------|---------------------------------------------------------------|
//! | [`client`]      | HTTP transport, retry policy, builder.                        |
//! | [`constants`]   | SDK metadata, API paths, payment / status enums.              |
//! | [`error`]       | The crate-wide [`Error`] enum and [`Result`] alias.           |
//! | [`i18n`]        | Language selector and message catalog.                        |
//! | [`payment_url`] | Hosted checkout redirect URL builder.                         |
//! | [`qr`]          | QR PNG generation.                                            |
//! | [`simulation`]  | Sandbox payment simulation.                                   |
//! | [`timefmt`]     | Small RFC 3339 parsing helper.                                |
//! | [`transaction`] | Transaction service.                                          |
//! | [`webhook`]     | Webhook parser and event validation.                          |
//!
//! The gRPC server layer from the original implementation is not part of
//! this crate.

pub mod client;
pub mod constants;
pub mod error;
pub mod i18n;
pub mod payment_url;
#[cfg(feature = "qr")]
pub mod qr;
#[cfg(feature = "simulation")]
pub mod simulation;
pub mod timefmt;
pub mod transaction;
#[cfg(feature = "webhook")]
pub mod webhook;

pub use client::{Client, ClientBuilder};
pub use constants::{
    PaymentMethod, SDK_NAME, SDK_REPOSITORY, SDK_VERSION, TransactionStatus, user_agent,
};
pub use error::{Error, Result};
pub use i18n::Language;
pub use payment_url::{Options as PaymentUrlOptions, UrlBuildError, build as build_payment_url};
#[cfg(feature = "qr")]
pub use qr::{Options as QrOptions, QrError, QrGenerator, RecoveryLevel};
#[cfg(feature = "simulation")]
pub use simulation::SimulationService;
pub use transaction::TransactionService;
#[cfg(feature = "webhook")]
pub use webhook::{Event as WebhookEvent, Parser as WebhookParser, WebhookError};
