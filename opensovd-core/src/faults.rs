// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0

//! Fault provider trait and types.

use std::collections::HashMap;

use async_trait::async_trait;

/// Filter criteria for listing faults.
#[derive(Debug, Clone, Default)]
pub struct FaultFilter {
    /// Filter by a specific DTC status key (e.g. `"confirmedDTC"`).
    pub status: Option<String>,
    /// Filter by severity level (0 = Trace … 5 = Fatal).
    pub severity: Option<u32>,
    /// Restrict results to a specific scope.
    pub scope: Option<String>,
}

/// A single fault entry returned by a [`FaultProvider`].
#[derive(Debug, Clone)]
pub struct FaultInfo {
    pub code: String,
    pub display_code: Option<String>,
    pub scope: Option<String>,
    pub fault_name: String,
    pub severity: Option<u32>,
    /// DTC status flags encoded as `"0"` / `"1"` strings.
    /// Keys match those produced by the DFM: `"testFailed"`, `"confirmedDTC"`, etc.
    pub status: Option<HashMap<String, String>>,
}

/// Errors that can occur when accessing faults.
#[derive(Debug, Clone, thiserror::Error)]
pub enum FaultError {
    #[error("fault not found: {0}")]
    NotFound(String),
    #[error("internal error: {0}")]
    Internal(String),
}

/// A `Result` alias where the `Err` variant is [`FaultError`].
pub type FaultResult<T> = std::result::Result<T, FaultError>;

/// Abstracts access to a component's Diagnostic Fault Manager.
#[async_trait]
pub trait FaultProvider: Send + Sync + 'static {
    /// List all active faults, applying the optional filter.
    async fn list(&self, filter: FaultFilter) -> FaultResult<Vec<FaultInfo>>;

    /// Retrieve a single fault by its code.
    async fn get(&self, fault_code: &str) -> FaultResult<FaultInfo>;

    /// Clear all faults, optionally restricted to a scope.
    async fn clear_all(&self, scope: Option<&str>) -> FaultResult<()>;

    /// Clear a single fault by its code.
    async fn clear(&self, fault_code: &str) -> FaultResult<()>;
}
