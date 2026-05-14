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

//! Sandbox payment simulation service.
//!
//! Wraps the `/api/paymentsimulation` endpoint. In a sandbox project this
//! marks an existing pending transaction as paid, which is useful for
//! end-to-end testing of webhook handlers without involving a real bank.
//!
//! Validation rules match the rest of the SDK: empty `order_id` ->
//! [`crate::Error::InvalidOrderId`], non-positive `amount` ->
//! [`crate::Error::InvalidAmount`].

use reqwest::Method;
use serde::Serialize;

use crate::client::Client;
use crate::constants::PATH_PAYMENT_SIMULATION;
use crate::error::{Error, Result};

/// Service handle wrapping a [`Client`].
///
/// Cheap to clone; the inner [`Client`] is already reference counted.
#[derive(Debug, Clone)]
pub struct SimulationService {
    client: Client,
}

/// Input for [`SimulationService::pay`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PayRequest {
    /// Order identifier to mark as paid.
    pub order_id: String,
    /// Amount, must match the existing transaction and be greater than 0.
    pub amount: i64,
}

/// Wire-format body. Built from the request plus the credentials stored on
/// the client so callers never have to repeat them.
#[derive(Debug, Serialize)]
struct RequestBody<'a> {
    project: &'a str,
    order_id: &'a str,
    amount: i64,
    api_key: &'a str,
}

impl SimulationService {
    /// Wrap an existing [`Client`].
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Mark `order_id` as paid in the sandbox.
    ///
    /// The endpoint returns 200 with no useful body, so this just returns
    /// `()` on success. Any non-2xx response surfaces as
    /// [`crate::Error::Api`].
    pub async fn pay(&self, request: &PayRequest) -> Result<()> {
        if request.order_id.is_empty() {
            return Err(Error::invalid_order_id(self.client.language()));
        }
        if request.amount <= 0 {
            return Err(Error::invalid_amount(self.client.language()));
        }

        let body = serde_json::to_vec(&RequestBody {
            project: self.client.project(),
            order_id: &request.order_id,
            amount: request.amount,
            api_key: self.client.api_key(),
        })
        .map_err(|err| Error::encode_json(self.client.language(), err))?;

        self.client
            .do_request(Method::POST, PATH_PAYMENT_SIMULATION, Some(body))
            .await
            .map(|_| ())
    }
}
