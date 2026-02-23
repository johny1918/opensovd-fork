// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use async_trait::async_trait;
use mock_dfm::MockSovdFaultManager;
use opensovd_core::{Component, Data, DataError, DataFilter, DataProvider, Metadata};
use opensovd_server::Topology;

// exposes fault codes from the mock DFM as readable data items
pub(crate) struct DfmDataProvider {
    dfm: Arc<MockSovdFaultManager>,
    path: String,
}

impl DfmDataProvider {
    pub(crate) fn new(dfm: Arc<MockSovdFaultManager>, path: impl Into<String>) -> Self {
        Self { dfm, path: path.into() }
    }
}

#[async_trait]
impl DataProvider for DfmDataProvider {
    async fn list(&self, _filter: DataFilter) -> Result<Vec<Metadata>, DataError> {
        let faults = self
            .dfm
            .get_all_faults(&self.path)
            .map_err(|e| DataError::Internal(format!("dfm: {e:?}")))?;

        Ok(faults
            .into_iter()
            .map(|f| Metadata {
                id: f.code,
                name: f.fault_name,
                category: "x-faultMemory".into(),
                translation_id: if f.fault_translation_id.is_empty() {
                    None
                } else {
                    Some(f.fault_translation_id)
                },
                groups: vec![],
                tags: vec![],
                schema: None,
                is_readable: true,
                is_writable: false,
            })
            .collect())
    }

    async fn read(&self, data_id: &str, _include_schema: bool) -> Result<Data, DataError> {
        let (fault, env) = self
            .dfm
            .get_fault(&self.path, data_id)
            .map_err(|_| DataError::NotFound(data_id.to_string()))?;

        let value = serde_json::json!({
            "value": {
                "code": fault.code,
                "fault_name": fault.fault_name,
                "severity": fault.severity,
                "status": fault.status,
                "env_data": env,
            }
        });

        Ok(Data { data: value, schema: None })
    }

    async fn write(&self, _data_id: &str, _value: serde_json::Value) -> Result<(), DataError> {
        Err(DataError::ReadOnly)
    }
}

// two components backed by the mock fault manager — hvac and ivi
pub(crate) async fn dfm_topology() -> Topology {
    let dfm = Arc::new(MockSovdFaultManager::new());

    let hvac = Component::new("hvac", "HVAC System")
        .with_data_provider(DfmDataProvider::new(Arc::clone(&dfm), "hvac"));

    let ivi = Component::new("ivi", "In-Vehicle Infotainment")
        .with_data_provider(DfmDataProvider::new(Arc::clone(&dfm), "ivi"));

    let topology = Topology::new();
    {
        let mut t = topology.write().await;
        t.add_component(hvac);
        t.add_component(ivi);
    }
    topology
}
