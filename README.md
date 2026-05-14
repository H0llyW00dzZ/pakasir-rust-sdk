# Pakasir Rust SDK (Unofficial)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](#license)
[![codecov](https://codecov.io/gh/H0llyW00dzZ/pakasir-rust-sdk/graph/badge.svg)](https://codecov.io/gh/H0llyW00dzZ/pakasir-rust-sdk)
[![Crates.io](https://img.shields.io/crates/v/pakasir-sdk.svg)](https://crates.io/crates/pakasir-sdk)
[![Documentation](https://docs.rs/pakasir-sdk/badge.svg)](https://docs.rs/pakasir-sdk)

Unofficial Rust port of the [Pakasir Go SDK](https://github.com/H0llyW00dzZ/pakasir-go-sdk) in this repository.

## Status

This crate currently ports the core REST SDK surface:

- HTTP client with retries and `Retry-After` support
- Transaction service (`create`, `cancel`, `detail`)
- Simulation service (`pay`)
- Webhook parsing
- Payment URL builder
- QR PNG generation
- English and Indonesian localized SDK errors

The [Go repository's](https://github.com/H0llyW00dzZ/pakasir-go-sdk) gRPC server layer is not ported yet in this crate.

## Quick Start

```rust
use pakasir_sdk::{Client, PaymentMethod, TransactionService};

#[tokio::main]
async fn main() -> Result<(), pakasir_sdk::Error> {
    let client = Client::builder("your-project-slug", "your-api-key").build();
    let transactions = TransactionService::new(client);

    let response = transactions
        .create(
            PaymentMethod::Qris,
            &pakasir_sdk::transaction::CreateRequest {
                order_id: "INV123456".into(),
                amount: 99_000,
            },
        )
        .await?;

    println!("payment number: {}", response.payment.payment_number);
    println!("total payment: {}", response.payment.total_payment);

    Ok(())
}
```

## Layout

```text
pakasir-rust-sdk/
├── Cargo.toml
├── README.md
└── src/
    ├── client.rs
    ├── constants.rs
    ├── error.rs
    ├── i18n.rs
    ├── lib.rs
    ├── payment_url.rs
    ├── qr.rs
    ├── simulation.rs
    ├── timefmt.rs
    ├── transaction.rs
    └── webhook.rs
```
