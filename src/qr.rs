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

//! QR code rendering.
//!
//! Mostly used to turn a QRIS payload returned by the API into a PNG you can
//! show to the customer, but it works for any string content. Output is
//! always a PNG byte buffer; either keep it in memory ([`QrGenerator::encode`]),
//! stream it to a writer ([`QrGenerator::write`]), or drop it on disk
//! ([`QrGenerator::write_file`]).

use image::{DynamicImage, ImageFormat, Rgba};
use qrcode::{EcLevel, QrCode};
use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;
use thiserror::Error;

/// Error-correction levels, mirroring [`qrcode::EcLevel`] but using names
/// that don't require knowing the standard's single-letter codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryLevel {
    /// ~7% recovery. Smallest code.
    Low,
    /// ~15% recovery. Default.
    Medium,
    /// ~25% recovery.
    High,
    /// ~30% recovery. Largest code.
    Highest,
}

impl From<RecoveryLevel> for EcLevel {
    fn from(value: RecoveryLevel) -> Self {
        match value {
            RecoveryLevel::Low => EcLevel::L,
            RecoveryLevel::Medium => EcLevel::M,
            RecoveryLevel::High => EcLevel::Q,
            RecoveryLevel::Highest => EcLevel::H,
        }
    }
}

/// Rendering options for [`QrGenerator`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Minimum image dimension in pixels. The renderer will pick a module
    /// size that produces an image at least this big in each direction.
    pub size: u32,
    /// Error-correction level used when encoding.
    pub recovery_level: RecoveryLevel,
    /// RGBA color for the dark modules.
    pub foreground: [u8; 4],
    /// RGBA color for the light modules / background.
    pub background: [u8; 4],
}

impl Default for Options {
    /// 256 px, medium recovery, black on white.
    fn default() -> Self {
        Self {
            size: 256,
            recovery_level: RecoveryLevel::Medium,
            foreground: [0, 0, 0, 255],
            background: [255, 255, 255, 255],
        }
    }
}

impl Options {
    /// Override the minimum image size. Zero is ignored.
    pub fn with_size(mut self, size: u32) -> Self {
        if size > 0 {
            self.size = size;
        }
        self
    }

    /// Override the error-correction level.
    pub fn with_recovery_level(mut self, recovery_level: RecoveryLevel) -> Self {
        self.recovery_level = recovery_level;
        self
    }

    /// Override the dark-module color.
    pub fn with_foreground(mut self, rgba: [u8; 4]) -> Self {
        self.foreground = rgba;
        self
    }

    /// Override the light-module / background color.
    pub fn with_background(mut self, rgba: [u8; 4]) -> Self {
        self.background = rgba;
        self
    }
}

/// Things that can go wrong while rendering a QR code.
#[derive(Debug, Error)]
pub enum QrError {
    /// Caller passed an empty string. The QR spec requires at least one
    /// byte.
    #[error("qr: content must not be empty")]
    EmptyContent,
    /// The `qrcode` crate refused to encode the payload (usually because it
    /// is too large for the selected recovery level).
    #[error("qr: encode failed: {message}")]
    EncodeFailed { message: String },
    /// Writing the image to PNG failed.
    #[error("qr: failed to write png: {source}")]
    PngWrite {
        #[source]
        source: image::ImageError,
    },
    /// [`QrGenerator::write_file`] could not write to the target path.
    #[error("qr: failed to write file: {source}")]
    WriteFile {
        #[source]
        source: std::io::Error,
    },
    /// [`QrGenerator::write`] could not push bytes to the supplied writer.
    #[error("qr: failed to write output: {source}")]
    WriteOutput {
        #[source]
        source: std::io::Error,
    },
}

/// Stateless renderer that holds the rendering [`Options`] and turns
/// strings into PNG byte buffers.
#[derive(Debug, Clone)]
pub struct QrGenerator {
    options: Options,
}

impl Default for QrGenerator {
    fn default() -> Self {
        Self::new(Options::default())
    }
}

impl QrGenerator {
    /// New generator with the given options.
    pub fn new(options: Options) -> Self {
        Self { options }
    }

    /// Borrow the options this generator was built with.
    pub fn options(&self) -> &Options {
        &self.options
    }

    /// Encode `content` and return the resulting PNG bytes.
    ///
    /// Returns [`QrError::EmptyContent`] when called with an empty string,
    /// and [`QrError::EncodeFailed`] when the payload is too large for the
    /// configured recovery level.
    pub fn encode(&self, content: &str) -> Result<Vec<u8>, QrError> {
        if content.is_empty() {
            return Err(QrError::EmptyContent);
        }

        let code = QrCode::with_error_correction_level(
            content.as_bytes(),
            self.options.recovery_level.into(),
        )
        .map_err(|err| QrError::EncodeFailed {
            message: err.to_string(),
        })?;

        let image = code
            .render::<Rgba<u8>>()
            .min_dimensions(self.options.size, self.options.size)
            .dark_color(Rgba(self.options.foreground))
            .light_color(Rgba(self.options.background))
            .build();

        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .map_err(|source| QrError::PngWrite { source })?;

        Ok(cursor.into_inner())
    }

    /// Encode `content` and stream the PNG bytes into `writer`.
    pub fn write<W: Write>(&self, writer: &mut W, content: &str) -> Result<(), QrError> {
        let png = self.encode(content)?;
        writer
            .write_all(&png)
            .map_err(|source| QrError::WriteOutput { source })
    }

    /// Encode `content` and write the PNG bytes to `path`. Any existing
    /// file is overwritten.
    pub fn write_file<P>(&self, path: P, content: &str) -> Result<(), QrError>
    where
        P: AsRef<Path>,
    {
        let png = self.encode(content)?;
        fs::write(path, png).map_err(|source| QrError::WriteFile { source })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

    #[test]
    fn recovery_level_maps_to_every_ec_level() {
        assert_eq!(EcLevel::from(RecoveryLevel::Low), EcLevel::L);
        assert_eq!(EcLevel::from(RecoveryLevel::Medium), EcLevel::M);
        assert_eq!(EcLevel::from(RecoveryLevel::High), EcLevel::Q);
        assert_eq!(EcLevel::from(RecoveryLevel::Highest), EcLevel::H);
    }

    #[test]
    fn options_default_matches_documented_values() {
        let opts = Options::default();
        assert_eq!(opts.size, 256);
        assert_eq!(opts.recovery_level, RecoveryLevel::Medium);
        assert_eq!(opts.foreground, [0, 0, 0, 255]);
        assert_eq!(opts.background, [255, 255, 255, 255]);
    }

    #[test]
    fn options_setters_chain_and_apply() {
        let opts = Options::default()
            .with_size(512)
            .with_recovery_level(RecoveryLevel::Highest)
            .with_foreground([10, 20, 30, 255])
            .with_background([240, 240, 240, 255]);

        assert_eq!(opts.size, 512);
        assert_eq!(opts.recovery_level, RecoveryLevel::Highest);
        assert_eq!(opts.foreground, [10, 20, 30, 255]);
        assert_eq!(opts.background, [240, 240, 240, 255]);
    }

    #[test]
    fn options_with_size_zero_is_a_no_op() {
        // Zero must not override the configured size — guard against accidental shrink.
        let opts = Options::default().with_size(0);
        assert_eq!(opts.size, 256);
    }

    #[test]
    fn generator_default_uses_options_default() {
        let gen_a = QrGenerator::default();
        let gen_b = QrGenerator::new(Options::default());
        assert_eq!(gen_a.options(), gen_b.options());
    }

    #[test]
    fn generator_options_getter_returns_configured_options() {
        let opts = Options::default().with_size(128);
        let generator = QrGenerator::new(opts.clone());
        assert_eq!(generator.options(), &opts);
    }

    #[test]
    fn encode_rejects_empty_content() {
        let err = QrGenerator::default().encode("").unwrap_err();
        assert!(matches!(err, QrError::EmptyContent));
        assert_eq!(err.to_string(), "qr: content must not be empty");
    }

    #[test]
    fn encode_rejects_payload_too_large_for_recovery_level() {
        // Highest recovery dramatically lowers capacity. A multi-KB payload
        // will exceed the QR-spec maximum, surfacing as `EncodeFailed`.
        let huge = "A".repeat(4_000);
        let generator =
            QrGenerator::new(Options::default().with_recovery_level(RecoveryLevel::Highest));
        let err = generator.encode(&huge).unwrap_err();
        assert!(
            matches!(&err, QrError::EncodeFailed { message } if !message.is_empty()),
            "expected non-empty EncodeFailed, got: {err:?}"
        );
        assert!(err.to_string().starts_with("qr: encode failed:"));
    }

    #[test]
    fn write_streams_png_bytes() {
        let generator = QrGenerator::new(Options::default().with_size(64));
        let mut buffer = Vec::new();
        generator.write(&mut buffer, "hello").unwrap();
        assert_eq!(&buffer[..8], &PNG_SIGNATURE);
    }

    #[test]
    fn write_surfaces_write_output_errors() {
        use std::io::Write;

        struct FailingWriter;
        impl Write for FailingWriter {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("disk full"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let generator = QrGenerator::new(Options::default().with_size(64));
        let mut sink = FailingWriter;
        let err = generator.write(&mut sink, "hi").unwrap_err();
        assert!(matches!(err, QrError::WriteOutput { .. }));
        assert!(err.to_string().starts_with("qr: failed to write output:"));

        // `Write` requires a `flush` impl; exercise it directly so the
        // trivially-Ok body counts as covered.
        FailingWriter.flush().unwrap();
    }

    #[test]
    fn write_file_round_trips_through_temp_path() {
        let mut path = env::temp_dir();
        path.push(format!("pakasir-qr-test-{}.png", std::process::id()));
        let generator = QrGenerator::new(Options::default().with_size(64));
        generator.write_file(&path, "hello disk").unwrap();

        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..8], &PNG_SIGNATURE);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn write_file_surfaces_write_file_errors() {
        // Path that cannot exist on the host filesystem. Using compile-time
        // `cfg` rather than runtime `cfg!(windows)` so coverage doesn't flag
        // the unused arm on the other platform.
        #[cfg(windows)]
        let bad_path = r"Z:\definitely\does\not\exist\nope.png".to_string();
        #[cfg(not(windows))]
        let bad_path = "/proc/this/is/not/writable/nope.png".to_string();

        let generator = QrGenerator::new(Options::default().with_size(64));
        let err = generator.write_file(bad_path, "x").unwrap_err();
        assert!(matches!(err, QrError::WriteFile { .. }));
        assert!(err.to_string().starts_with("qr: failed to write file:"));
    }

    #[test]
    fn write_propagates_encode_failures() {
        // Empty input still goes through `encode` first; both write helpers
        // must surface the resulting error rather than producing junk.
        let generator = QrGenerator::default();
        let mut buffer = Vec::new();
        assert!(matches!(
            generator.write(&mut buffer, "").unwrap_err(),
            QrError::EmptyContent
        ));

        let mut tmp = env::temp_dir();
        tmp.push("pakasir-qr-unused.png");
        assert!(matches!(
            generator.write_file(&tmp, "").unwrap_err(),
            QrError::EmptyContent
        ));
    }

    #[test]
    fn png_write_error_display_includes_prefix() {
        let inner = image::ImageError::IoError(std::io::Error::other("png boom"));
        let err = QrError::PngWrite { source: inner };
        assert!(err.to_string().starts_with("qr: failed to write png:"));
    }
}
