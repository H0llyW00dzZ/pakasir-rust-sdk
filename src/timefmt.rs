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

//! Tiny RFC 3339 parsing helper.
//!
//! The Pakasir API returns timestamps as RFC 3339 strings such as
//! `2024-09-10T08:07:02.819+07:00`. The SDK keeps the raw string on the
//! response structs (so callers who only need the textual form pay nothing)
//! and exposes [`parse_rfc3339`] for callers that want a real
//! [`chrono::DateTime`].
//!
//! Pulled out into its own module so [`crate::transaction`] and
//! [`crate::webhook`] can share the implementation without re-exporting
//! [`chrono`] details from each module.

use chrono::{DateTime, FixedOffset};

/// Parse an RFC 3339 timestamp, preserving its original offset.
///
/// Returns the underlying [`chrono::ParseError`] when the input is not a
/// valid RFC 3339 string.
pub fn parse_rfc3339(value: &str) -> Result<DateTime<FixedOffset>, chrono::ParseError> {
    DateTime::parse_from_rfc3339(value)
}
