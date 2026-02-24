// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0

//! Data resource endpoints.
//!
//! Provides routes for:
//! - GET /components/{component_id}/faults                 - Get list of all faults, supports queries aswell.
//! - GET /components/{component_id}/faults/{fault_code}    - Get specific fault code.
//! - DELETE /components/{component_id}/faults              - Delete faults based on scope.
//! - DELETE /components/{component_id}/faults/{fault_code} - Delete specific fault code.

use axum::{
    Json,
    Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get},
};
use opensovd_models::faults::{FaultQueryParams, FaultData};
use opensovd_core::Topology;
use super::error::Error;

use super::AppState;

pub fn routes<V>() -> Router<AppState<V>>
where
    V: Clone + Send + Sync + 'static,
{
    Router::new()
        .route(
            "/components/{component_id}/faults",
            get(get_faults),
        )
        .route(
            "/components/{component_id}/faults/{fault_code}",
            get(get_fault_code),
        )
        .route("/components/{component_id}/data", delete(delete_faults))
        .route(
            "/components/{component_id}/data/{data_id}", delete(delete_fault_code)
        )
}

async fn get_faults(
    State(topology): State<Topology>,
    Path(component_id) : Path<String>,
    Query(_query_params): Query<FaultQueryParams>,
) -> Json<(StatusCode, FaultData)> {

    let topo = topology.read().await;

    let entity = topo
    .get_component(&component_id)
    .map_err(|_| Error::EntityNotFound(component_id.clone()));

    if let Ok(provider) = entity {
        provider.data_provider()
        .ok_or_else(|| Error::ProviderNotAvailable("fault".into()));
    }

    let 


    Json(StatusCode::OK, )
}

async fn get_fault_code(

) -> StatusCode {
    StatusCode::OK
}

async fn delete_faults() -> StatusCode {
    StatusCode::OK
}

async fn delete_fault_code() -> StatusCode {
    StatusCode::OK
}