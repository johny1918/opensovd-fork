// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0

// Fault endpoints — reading and clearing DTCs from a component's fault provider.

use axum::{
    Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
};
use axum_extra::extract::WithRejection;
use opensovd_core::{FaultFilter, Topology};
use opensovd_models::Items;
use opensovd_models::faults::{Fault, FaultData, FaultQueryParams, FaultResponse, FaultStatus};

use super::AppState;
use super::error::{Error, Result};

pub fn routes<V>() -> Router<AppState<V>>
where
    V: Clone + Send + Sync + 'static,
{
    Router::new()
        .route(
            "/components/{component_id}/faults",
            get(get_faults).delete(delete_faults),
        )
        .route(
            "/components/{component_id}/faults/{fault_code}",
            get(get_fault_code).delete(delete_fault_code),
        )
}

/// GET /components/{component_id}/faults - List active faults.
///
/// Supports optional query filtering by status key, severity, and scope.
async fn get_faults(
    State(topology): State<Topology>,
    Path(component_id): Path<String>,
    WithRejection(Query(query), _): WithRejection<Query<FaultQueryParams>, Error>,
) -> Result<Json<Items<Fault>>> {
    let topo = topology.read().await;
    let entity = topo
        .get_component(&component_id)
        .map_err(|_| Error::EntityNotFound(component_id.clone()))?;
    let provider = entity
        .fault_provider()
        .ok_or_else(|| Error::ProviderNotAvailable("faults".into()))?;

    let filter = FaultFilter {
        status: query.status.map(|s| status_key_to_str(&s.key).to_owned()),
        severity: query.severity,
        scope: query.scope,
    };

    let faults = provider.list(filter).await?;

    let items = faults.into_iter().map(fault_info_to_model).collect();

    Ok(Json(Items { items }))
}

/// GET /components/{component_id}/faults/{fault_code} - Get a specific fault.
async fn get_fault_code(
    State(topology): State<Topology>,
    Path((component_id, fault_code)): Path<(String, String)>,
) -> Result<Json<FaultResponse>> {
    let topo = topology.read().await;
    let entity = topo
        .get_component(&component_id)
        .map_err(|_| Error::EntityNotFound(component_id.clone()))?;
    let provider = entity
        .fault_provider()
        .ok_or_else(|| Error::ProviderNotAvailable("faults".into()))?;

    let fault_info = provider.get(&fault_code).await?;

    Ok(Json(FaultResponse {
        items: fault_info_to_model(fault_info),
        schema: None,
    }))
}

/// DELETE /components/{component_id}/faults - Clear all faults.
///
/// An optional `scope` query parameter restricts clearing to a specific scope.
async fn delete_faults(
    State(topology): State<Topology>,
    Path(component_id): Path<String>,
    WithRejection(Query(query), _): WithRejection<Query<FaultData>, Error>,
) -> Result<StatusCode> {
    let topo = topology.read().await;
    let entity = topo
        .get_component(&component_id)
        .map_err(|_| Error::EntityNotFound(component_id.clone()))?;
    let provider = entity
        .fault_provider()
        .ok_or_else(|| Error::ProviderNotAvailable("faults".into()))?;

    provider.clear_all(query.scope.as_deref()).await?;

    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /components/{component_id}/faults/{fault_code} - Clear a specific fault.
async fn delete_fault_code(
    State(topology): State<Topology>,
    Path((component_id, fault_code)): Path<(String, String)>,
) -> Result<StatusCode> {
    let topo = topology.read().await;
    let entity = topo
        .get_component(&component_id)
        .map_err(|_| Error::EntityNotFound(component_id.clone()))?;
    let provider = entity
        .fault_provider()
        .ok_or_else(|| Error::ProviderNotAvailable("faults".into()))?;

    provider.clear(&fault_code).await?;

    Ok(StatusCode::NO_CONTENT)
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn fault_info_to_model(f: opensovd_core::FaultInfo) -> Fault {
    Fault {
        code: f.code,
        display_code: f.display_code,
        scope: f.scope,
        fault_name: f.fault_name,
        severity: f.severity,
        status: f.status.map(|s| FaultStatus {
            test_failed: s.get("testFailed").map(|v| v == "1"),
            test_failed_this_operation_cycle: s
                .get("testFailedThisOperationCycle")
                .map(|v| v == "1"),
            pending_dtc: s.get("pendingDTC").map(|v| v == "1"),
            confirmed_dtc: s.get("confirmedDTC").map(|v| v == "1"),
            test_not_completed_since_last_clear: s
                .get("testNotCompletedSinceLastClear")
                .map(|v| v == "1"),
            test_failed_since_last_clear: s.get("testFailedSinceLastClear").map(|v| v == "1"),
            test_not_completed_this_operation_cycle: s
                .get("testNotCompletedThisOperationCycle")
                .map(|v| v == "1"),
            warning_indicator_requested: s.get("warningIndicatorRequested").map(|v| v == "1"),
            mask: None,
        }),
    }
}

fn status_key_to_str(key: &opensovd_models::faults::FaultStatusKeys) -> &'static str {
    use opensovd_models::faults::FaultStatusKeys;
    match key {
        FaultStatusKeys::ConfirmedDtc => "confirmedDTC",
        FaultStatusKeys::Mask => "mask",
        FaultStatusKeys::PendingDtc => "pendingDTC",
        FaultStatusKeys::TestFailed => "testFailed",
        FaultStatusKeys::TestFailedSinceLastClear => "testFailedSinceLastClear",
        FaultStatusKeys::TestFailedThisOperationCycle => "testFailedThisOperationCycle",
        FaultStatusKeys::TestNotCompletedSinceLastClear => "testNotCompletedSinceLastClear",
        FaultStatusKeys::TestNotCompletedThisOperationCycle => {
            "testNotCompletedThisOperationCycle"
        }
        FaultStatusKeys::WarningIndicatorRequested => "warningIndicatorRequested",
    }
}
