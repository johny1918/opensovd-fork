// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use async_trait::async_trait;
use mock_dfm::MockSovdFaultManager;
use opensovd_core::{Component, FaultError, FaultFilter, FaultInfo, FaultProvider, FaultResult};
use opensovd_server::Topology;
use tokio::sync::Mutex;

/// Wraps the mock DFM and exposes its faults through the SOVD `FaultProvider` trait.
pub(crate) struct DfmFaultProvider {
    dfm: Arc<Mutex<MockSovdFaultManager>>,
    path: String,
}

impl DfmFaultProvider {
    pub(crate) fn new(dfm: Arc<Mutex<MockSovdFaultManager>>, path: impl Into<String>) -> Self {
        Self { dfm, path: path.into() }
    }
}

#[async_trait]
impl FaultProvider for DfmFaultProvider {
    async fn list(&self, filter: FaultFilter) -> FaultResult<Vec<FaultInfo>> {
        let dfm = self.dfm.lock().await;

        let faults = dfm
            .get_all_faults(&self.path)
            .map_err(|e| FaultError::Internal(format!("dfm: {e:?}")))?;

        Ok(faults
            .into_iter()
            .filter(|f| {
                filter.severity.is_none_or(|s| f.severity <= s)
                    && filter.scope.as_deref().is_none_or(|sc| f.scope == sc)
                    && filter.status.as_deref().is_none_or(|key| {
                        f.status.get(key).map(|v| v == "1").unwrap_or(false)
                    })
            })
            .map(|f| FaultInfo {
                code: f.code,
                display_code: Some(f.display_code),
                scope: Some(f.scope),
                fault_name: f.fault_name,
                severity: Some(f.severity),
                status: Some(f.status),
            })
            .collect())
    }

    async fn get(&self, fault_code: &str) -> FaultResult<FaultInfo> {
        let dfm = self.dfm.lock().await;

        let (fault, _env) = dfm
            .get_fault(&self.path, fault_code)
            .map_err(|_| FaultError::NotFound(fault_code.to_string()))?;

        Ok(FaultInfo {
            code: fault.code,
            display_code: Some(fault.display_code),
            scope: Some(fault.scope),
            fault_name: fault.fault_name,
            severity: Some(fault.severity),
            status: Some(fault.status),
        })
    }

    async fn clear_all(&self, scope: Option<&str>) -> FaultResult<()> {
        let mut dfm = self.dfm.lock().await;
        // If a scope is provided clear just that scope, otherwise clear the component's own path.
        dfm.clear_faults(scope.unwrap_or(&self.path));
        Ok(())
    }

    async fn clear(&self, fault_code: &str) -> FaultResult<()> {
        let mut dfm = self.dfm.lock().await;
        dfm.remove_fault(&self.path, fault_code);
        Ok(())
    }
}

/// Two components (hvac, ivi) backed by the mock fault manager.
pub(crate) async fn dfm_topology() -> Topology {
    let dfm = Arc::new(Mutex::new(MockSovdFaultManager::new()));

    let hvac = Component::new("hvac", "HVAC System")
        .with_fault_provider(DfmFaultProvider::new(Arc::clone(&dfm), "hvac"));

    let ivi = Component::new("ivi", "In-Vehicle Infotainment")
        .with_fault_provider(DfmFaultProvider::new(Arc::clone(&dfm), "ivi"));

    let topology = Topology::new();
    {
        let mut t = topology.write().await;
        t.add_component(hvac);
        t.add_component(ivi);
    }
    topology
}
