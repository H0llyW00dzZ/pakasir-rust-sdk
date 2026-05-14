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
//! Localized error messages.
//!
//! Every user-facing message the SDK can produce goes through here. Add a
//! new variant to [`MessageKey`] and supply both the English and Indonesian
//! translation in [`get`]; the compiler will tell you if you forget one.
//!
//! Some templates contain placeholders such as `%d` or `%s`. They are
//! substituted by the caller (see the constructors on [`crate::Error`]) and
//! intentionally left as literal placeholders here so the strings remain
//! plain `&'static str`.

/// Language used to format error messages produced by the SDK.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    /// English. Default.
    #[default]
    English,
    /// Bahasa Indonesia.
    Indonesian,
}

/// Identifier for one entry in the message catalog.
///
/// Each variant maps to one English and one Indonesian string in [`get`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKey {
    /// "project slug is required".
    InvalidProject,
    /// "API key is required".
    InvalidApiKey,
    /// "amount must be greater than 0".
    InvalidAmount,
    /// "order ID is required".
    InvalidOrderId,
    /// "unsupported payment method: %s" — `%s` is filled in by the caller.
    InvalidPaymentMethod,
    /// "failed to encode request as JSON".
    FailedToEncode,
    /// "failed to decode response".
    FailedToDecode,
    /// "request failed due to permanent error".
    RequestFailedPermanent,
    /// "request failed after %d retries" — `%d` is filled in by the caller.
    RequestFailedAfterRetries,
}

/// Look up a message by `lang` and `key`.
///
/// Always returns a static string; missing combinations are a compile-time
/// error because the match is exhaustive.
pub fn get(lang: Language, key: MessageKey) -> &'static str {
    match lang {
        Language::English => match key {
            MessageKey::InvalidProject => "project slug is required",
            MessageKey::InvalidApiKey => "API key is required",
            MessageKey::InvalidAmount => "amount must be greater than 0",
            MessageKey::InvalidOrderId => "order ID is required",
            MessageKey::InvalidPaymentMethod => "unsupported payment method: %s",
            MessageKey::FailedToEncode => "failed to encode request as JSON",
            MessageKey::FailedToDecode => "failed to decode response",
            MessageKey::RequestFailedPermanent => "request failed due to permanent error",
            MessageKey::RequestFailedAfterRetries => "request failed after %d retries",
        },
        Language::Indonesian => match key {
            MessageKey::InvalidProject => "slug proyek wajib diisi",
            MessageKey::InvalidApiKey => "API key wajib diisi",
            MessageKey::InvalidAmount => "jumlah harus lebih dari 0",
            MessageKey::InvalidOrderId => "ID pesanan wajib diisi",
            MessageKey::InvalidPaymentMethod => "metode pembayaran tidak didukung: %s",
            MessageKey::FailedToEncode => "gagal mengenkode permintaan sebagai JSON",
            MessageKey::FailedToDecode => "gagal mendekode respons",
            MessageKey::RequestFailedPermanent => "permintaan gagal karena kesalahan permanen",
            MessageKey::RequestFailedAfterRetries => "permintaan gagal setelah %d percobaan ulang",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `(Language, MessageKey)` pair must resolve to a non-empty,
    /// distinct-per-language string. The exhaustive table also guards
    /// against accidental message reuse across languages.
    #[test]
    fn every_combination_resolves_to_a_unique_string() {
        let keys = [
            MessageKey::InvalidProject,
            MessageKey::InvalidApiKey,
            MessageKey::InvalidAmount,
            MessageKey::InvalidOrderId,
            MessageKey::InvalidPaymentMethod,
            MessageKey::FailedToEncode,
            MessageKey::FailedToDecode,
            MessageKey::RequestFailedPermanent,
            MessageKey::RequestFailedAfterRetries,
        ];

        for key in keys {
            let en = get(Language::English, key);
            let id = get(Language::Indonesian, key);
            assert!(!en.is_empty(), "missing English for {key:?}");
            assert!(!id.is_empty(), "missing Indonesian for {key:?}");
            assert_ne!(en, id, "English and Indonesian must differ for {key:?}");
        }
    }

    #[test]
    fn english_messages_match_expected_text() {
        assert_eq!(
            get(Language::English, MessageKey::InvalidProject),
            "project slug is required"
        );
        assert_eq!(
            get(Language::English, MessageKey::InvalidApiKey),
            "API key is required"
        );
        assert_eq!(
            get(Language::English, MessageKey::InvalidAmount),
            "amount must be greater than 0"
        );
        assert_eq!(
            get(Language::English, MessageKey::InvalidOrderId),
            "order ID is required"
        );
        assert_eq!(
            get(Language::English, MessageKey::InvalidPaymentMethod),
            "unsupported payment method: %s"
        );
        assert_eq!(
            get(Language::English, MessageKey::FailedToEncode),
            "failed to encode request as JSON"
        );
        assert_eq!(
            get(Language::English, MessageKey::FailedToDecode),
            "failed to decode response"
        );
        assert_eq!(
            get(Language::English, MessageKey::RequestFailedPermanent),
            "request failed due to permanent error"
        );
        assert_eq!(
            get(Language::English, MessageKey::RequestFailedAfterRetries),
            "request failed after %d retries"
        );
    }

    #[test]
    fn indonesian_messages_match_expected_text() {
        assert_eq!(
            get(Language::Indonesian, MessageKey::InvalidProject),
            "slug proyek wajib diisi"
        );
        assert_eq!(
            get(Language::Indonesian, MessageKey::InvalidApiKey),
            "API key wajib diisi"
        );
        assert_eq!(
            get(Language::Indonesian, MessageKey::InvalidAmount),
            "jumlah harus lebih dari 0"
        );
        assert_eq!(
            get(Language::Indonesian, MessageKey::InvalidOrderId),
            "ID pesanan wajib diisi"
        );
        assert_eq!(
            get(Language::Indonesian, MessageKey::InvalidPaymentMethod),
            "metode pembayaran tidak didukung: %s"
        );
        assert_eq!(
            get(Language::Indonesian, MessageKey::FailedToEncode),
            "gagal mengenkode permintaan sebagai JSON"
        );
        assert_eq!(
            get(Language::Indonesian, MessageKey::FailedToDecode),
            "gagal mendekode respons"
        );
        assert_eq!(
            get(Language::Indonesian, MessageKey::RequestFailedPermanent),
            "permintaan gagal karena kesalahan permanen"
        );
        assert_eq!(
            get(Language::Indonesian, MessageKey::RequestFailedAfterRetries),
            "permintaan gagal setelah %d percobaan ulang"
        );
    }

    #[test]
    fn language_default_is_english() {
        assert_eq!(Language::default(), Language::English);
    }
}
