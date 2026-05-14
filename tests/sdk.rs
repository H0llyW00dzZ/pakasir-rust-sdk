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
use axum::Router;
use axum::extract::{Json, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{any, get, post};
use pakasir_sdk::payment_url;
#[cfg(feature = "qr")]
use pakasir_sdk::qr;
#[cfg(feature = "simulation")]
use pakasir_sdk::simulation::{PayRequest, SimulationService};
use pakasir_sdk::transaction::{CancelRequest, CreateRequest, DetailRequest, TransactionService};
#[cfg(feature = "webhook")]
use pakasir_sdk::WebhookParser;
use pakasir_sdk::{Client, Language, PaymentMethod};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

async fn spawn_app(app: Router) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (format!("http://{address}"), handle)
}

#[tokio::test]
async fn transaction_create_cancel_and_detail_work() {
    async fn create(Json(body): Json<Value>) -> impl IntoResponse {
        assert_eq!(body["project"], "test-project");
        assert_eq!(body["order_id"], "INV123");
        assert_eq!(body["amount"], 99_000);
        assert_eq!(body["api_key"], "test-key");

        (
            StatusCode::OK,
            Json(json!({
                "payment": {
                    "project": "test-project",
                    "order_id": "INV123",
                    "amount": 99000,
                    "fee": 1003,
                    "total_payment": 100003,
                    "payment_method": "qris",
                    "payment_number": "0002010112345",
                    "expired_at": "2026-12-25T12:00:00Z"
                }
            })),
        )
    }

    async fn cancel(Json(body): Json<Value>) -> impl IntoResponse {
        assert_eq!(body["order_id"], "INV123");
        StatusCode::OK
    }

    async fn detail(Query(query): Query<HashMap<String, String>>) -> impl IntoResponse {
        assert_eq!(query.get("project").unwrap(), "test-project");
        assert_eq!(query.get("order_id").unwrap(), "INV123");
        assert_eq!(query.get("amount").unwrap(), "99000");
        assert_eq!(query.get("api_key").unwrap(), "test-key");

        (
            StatusCode::OK,
            Json(json!({
                "transaction": {
                    "amount": 99000,
                    "order_id": "INV123",
                    "project": "test-project",
                    "status": "completed",
                    "payment_method": "qris",
                    "completed_at": "2026-12-25T12:00:00Z"
                }
            })),
        )
    }

    let app = Router::new()
        .route("/api/transactioncreate/qris", post(create))
        .route("/api/transactioncancel", post(cancel))
        .route("/api/transactiondetail", get(detail));
    let (base_url, handle) = spawn_app(app).await;

    let client = Client::builder("test-project", "test-key")
        .base_url(base_url)
        .retries(0)
        .build();
    let transactions = TransactionService::new(client.clone());

    let created = transactions
        .create(
            PaymentMethod::Qris,
            &CreateRequest {
                order_id: "INV123".into(),
                amount: 99_000,
            },
        )
        .await
        .unwrap();
    assert_eq!(created.payment.total_payment, 100_003);

    transactions
        .cancel(&CancelRequest {
            order_id: "INV123".into(),
            amount: 99_000,
        })
        .await
        .unwrap();

    let detailed = transactions
        .detail(&DetailRequest {
            order_id: "INV123".into(),
            amount: 99_000,
        })
        .await
        .unwrap();
    assert_eq!(detailed.transaction.status.as_str(), "completed");
    assert_eq!(
        detailed.transaction.parse_time().unwrap().to_rfc3339(),
        "2026-12-25T12:00:00+00:00"
    );

    handle.abort();
}

#[cfg(feature = "simulation")]
#[tokio::test]
async fn simulation_pay_works() {
    async fn pay(Json(body): Json<Value>) -> impl IntoResponse {
        assert_eq!(body["project"], "test-project");
        assert_eq!(body["order_id"], "INV123");
        assert_eq!(body["amount"], 99_000);
        StatusCode::OK
    }

    let app = Router::new().route("/api/paymentsimulation", post(pay));
    let (base_url, handle) = spawn_app(app).await;

    let client = Client::builder("test-project", "test-key")
        .base_url(base_url)
        .retries(0)
        .build();
    let simulation = SimulationService::new(client);

    simulation
        .pay(&PayRequest {
            order_id: "INV123".into(),
            amount: 99_000,
        })
        .await
        .unwrap();

    handle.abort();
}

#[tokio::test]
async fn client_retries_gateway_errors_then_succeeds() {
    #[derive(Clone)]
    struct AppState {
        attempts: Arc<AtomicUsize>,
    }

    async fn handler(State(state): State<AppState>) -> impl IntoResponse {
        let attempt = state.attempts.fetch_add(1, Ordering::SeqCst);
        if attempt < 2 {
            return (StatusCode::SERVICE_UNAVAILABLE, "unavailable").into_response();
        }

        (StatusCode::OK, Json(json!({ "ok": true }))).into_response()
    }

    let state = AppState {
        attempts: Arc::new(AtomicUsize::new(0)),
    };
    let app = Router::new()
        .route("/test", any(handler))
        .with_state(state.clone());
    let (base_url, handle) = spawn_app(app).await;

    let client = Client::builder("test-project", "test-key")
        .base_url(base_url)
        .retries(3)
        .retry_wait(Duration::from_millis(1), Duration::from_millis(5))
        .build();

    let body = client
        .do_request(reqwest::Method::GET, "/test", None)
        .await
        .unwrap();
    assert_eq!(String::from_utf8(body).unwrap(), "{\"ok\":true}");
    assert_eq!(state.attempts.load(Ordering::SeqCst), 3);

    handle.abort();
}

#[tokio::test]
async fn client_does_not_retry_http_500() {
    #[derive(Clone)]
    struct AppState {
        attempts: Arc<AtomicUsize>,
    }

    async fn handler(State(state): State<AppState>) -> impl IntoResponse {
        state.attempts.fetch_add(1, Ordering::SeqCst);
        (StatusCode::INTERNAL_SERVER_ERROR, "server bug")
    }

    let state = AppState {
        attempts: Arc::new(AtomicUsize::new(0)),
    };
    let app = Router::new()
        .route("/test", any(handler))
        .with_state(state.clone());
    let (base_url, handle) = spawn_app(app).await;

    let client = Client::builder("test-project", "test-key")
        .base_url(base_url)
        .retries(3)
        .retry_wait(Duration::from_millis(1), Duration::from_millis(5))
        .build();

    let err = client
        .do_request(reqwest::Method::GET, "/test", None)
        .await
        .unwrap_err();
    assert_eq!(
        err.api_status(),
        Some(reqwest::StatusCode::INTERNAL_SERVER_ERROR)
    );
    assert_eq!(state.attempts.load(Ordering::SeqCst), 1);

    handle.abort();
}

#[tokio::test]
async fn client_enforces_response_size_limit() {
    async fn handler() -> impl IntoResponse {
        (StatusCode::OK, "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx")
    }

    let app = Router::new().route("/test", any(handler));
    let (base_url, handle) = spawn_app(app).await;

    let client = Client::builder("test-project", "test-key")
        .base_url(base_url)
        .retries(0)
        .max_response_size(8)
        .build();

    let err = client
        .do_request(reqwest::Method::GET, "/test", None)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("permanent error"));
    assert!(err.to_string().contains("response body too large"));

    handle.abort();
}

#[tokio::test]
async fn client_localizes_validation_errors() {
    let client = Client::builder("", "test-key")
        .language(Language::Indonesian)
        .retries(0)
        .build();

    let err = client
        .do_request(reqwest::Method::GET, "/test", None)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("slug proyek wajib diisi"));
}

#[cfg(feature = "webhook")]
#[test]
fn webhook_parser_builds_events() {
    let payload = br#"{"amount":22000,"order_id":"240910HDE7C9","project":"depodomain","status":"completed","payment_method":"qris","completed_at":"2024-09-10T08:07:02.819+07:00","is_sandbox":false}"#;

    let event = WebhookParser::new().parse_bytes(payload).unwrap();
    assert_eq!(event.order_id, "240910HDE7C9");
    assert_eq!(event.payment_method, PaymentMethod::Qris);
    event.validate().unwrap();
    assert_eq!(
        event.parse_time().unwrap().to_rfc3339(),
        "2024-09-10T08:07:02.819+07:00"
    );
}

#[test]
fn payment_url_builder_matches_expected_shape() {
    let url = payment_url::build(
        "https://app.pakasir.com/",
        "my project/test",
        22_000,
        &payment_url::Options {
            order_id: "INV123".into(),
            redirect: Some("https://example.com/done".into()),
            qris_only: true,
            use_paypal: true,
        },
    )
    .unwrap();

    assert!(url.starts_with("https://app.pakasir.com/paypal/my%20project%2Ftest/22000?"));
    assert!(url.contains("order_id=INV123"));
    assert!(url.contains("redirect=https%3A%2F%2Fexample.com%2Fdone"));
    assert!(url.contains("qris_only=1"));
}

#[cfg(feature = "qr")]
#[test]
fn qr_generator_returns_png_bytes() {
    let qr = qr::QrGenerator::new(qr::Options::default().with_size(128));
    let png = qr.encode("00020101021226...").unwrap();

    assert!(!png.is_empty());
    assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
}
