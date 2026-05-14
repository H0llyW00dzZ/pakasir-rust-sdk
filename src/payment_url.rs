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

//! Hosted checkout redirect URL builder.
//!
//! Given a base URL, a project slug, an amount, and an [`Options`] struct,
//! [`build`] returns the URL you would redirect the user to in order to
//! complete the payment on Pakasir's hosted page.
//!
//! Path shape:
//!
//! ```text
//! {base}/{pay|paypal}/{project}/{amount}?order_id=...&redirect=...&qris_only=1
//! ```
//!
//! `pay` is the default; the `paypal` segment is used when
//! [`Options::use_paypal`] is set. `qris_only=1` is only added when
//! [`Options::qris_only`] is `true`.

use std::fmt;
use url::Url;
use url::form_urlencoded::Serializer;

/// Knobs accepted by [`build`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    /// Merchant-side order identifier. Required.
    pub order_id: String,
    /// Where the gateway should redirect the user once the payment finishes.
    /// Omitted from the URL when `None` or empty.
    pub redirect: Option<String>,
    /// When `true`, adds `qris_only=1` to the query string so the hosted
    /// page only offers the QRIS option.
    pub qris_only: bool,
    /// When `true`, swaps the `pay` path segment for `paypal`.
    pub use_paypal: bool,
}

/// Reasons [`build`] can refuse to produce a URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlBuildError {
    /// `base_url` was empty.
    EmptyBaseUrl,
    /// `base_url` did not parse as a URL.
    InvalidBaseUrl,
    /// `project` was empty.
    EmptyProject,
    /// [`Options::order_id`] was empty.
    EmptyOrderId,
    /// `amount` was zero or negative.
    InvalidAmount,
}

impl fmt::Display for UrlBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBaseUrl => f.write_str("url: base URL is required"),
            Self::InvalidBaseUrl => f.write_str("url: invalid base URL"),
            Self::EmptyProject => f.write_str("url: project is required"),
            Self::EmptyOrderId => f.write_str("url: order ID is required"),
            Self::InvalidAmount => f.write_str("url: amount must be greater than 0"),
        }
    }
}

impl std::error::Error for UrlBuildError {}

/// Build a hosted-checkout redirect URL.
///
/// Validation runs first and fails fast with a [`UrlBuildError`]. After
/// that, the URL is assembled by parsing `base_url`, pushing the
/// `{pay|paypal}/{project}/{amount}` path segments, and appending the query
/// string. Path segments are percent-encoded by the `url` crate, so slugs
/// containing spaces or `/` come out correctly escaped.
///
/// # Example
///
/// ```
/// let url = pakasir_sdk::payment_url::build(
///     "https://app.pakasir.com",
///     "depodomain",
///     22_000,
///     &pakasir_sdk::payment_url::Options {
///         order_id: "INV123".into(),
///         redirect: Some("https://example.com/done".into()),
///         qris_only: true,
///         use_paypal: false,
///     },
/// ).unwrap();
///
/// assert!(url.starts_with("https://app.pakasir.com/pay/depodomain/22000?"));
/// ```
pub fn build(
    base_url: &str,
    project: &str,
    amount: i64,
    options: &Options,
) -> Result<String, UrlBuildError> {
    if base_url.is_empty() {
        return Err(UrlBuildError::EmptyBaseUrl);
    }
    if project.is_empty() {
        return Err(UrlBuildError::EmptyProject);
    }
    if options.order_id.is_empty() {
        return Err(UrlBuildError::EmptyOrderId);
    }
    if amount <= 0 {
        return Err(UrlBuildError::InvalidAmount);
    }

    let path_prefix = if options.use_paypal { "paypal" } else { "pay" };
    let mut url =
        Url::parse(base_url.trim_end_matches('/')).map_err(|_| UrlBuildError::InvalidBaseUrl)?;

    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| UrlBuildError::InvalidBaseUrl)?;
        // pop any trailing empty segment so we don't end up with `//pay/...`
        segments.pop_if_empty();
        segments.push(path_prefix);
        segments.push(project);
        segments.push(&amount.to_string());
    }

    let mut query = Serializer::new(String::new());
    query.append_pair("order_id", &options.order_id);

    if let Some(redirect) = &options.redirect {
        if !redirect.is_empty() {
            query.append_pair("redirect", redirect);
        }
    }
    if options.qris_only {
        query.append_pair("qris_only", "1");
    }

    url.set_query(Some(&query.finish()));

    Ok(url.into())
}
