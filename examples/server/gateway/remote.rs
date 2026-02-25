// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use opensovd_client::Client;
use opensovd_core::{
    App, Component, Data, DataError, DataFilter, DataProvider, DiscoveryError, EntityCollection,
    FaultError, FaultFilter, FaultInfo, FaultProvider, FaultResult, Metadata,
};
use opensovd_models::Items;
use opensovd_models::data::DataList;
use opensovd_models::discovery::EntityCapabilities;
use opensovd_models::faults::{Fault, FaultResponse};

// ─── RemoteDataProvider ──────────────────────────────────────────────────────

// Proxies data operations to the originating slave over HTTP.
pub(crate) struct RemoteDataProvider {
    pub(crate) client: Arc<Client>,
    entity_id: String,
    is_app: bool,
}

impl RemoteDataProvider {
    pub(crate) fn for_component(client: Arc<Client>, entity_id: impl Into<String>) -> Self {
        Self { client, entity_id: entity_id.into(), is_app: false }
    }

    pub(crate) fn for_app(client: Arc<Client>, entity_id: impl Into<String>) -> Self {
        Self { client, entity_id: entity_id.into(), is_app: true }
    }

    async fn fetch_data_list(&self) -> Result<DataList, opensovd_client::Error> {
        if self.is_app {
            self.client.app(&self.entity_id).list_data().send().await.map(|r| r.data)
        } else {
            self.client.component(&self.entity_id).list_data().send().await.map(|r| r.data)
        }
    }
}

#[async_trait]
impl DataProvider for RemoteDataProvider {
    async fn list(&self, _filter: DataFilter) -> Result<Vec<Metadata>, DataError> {
        let list = self
            .fetch_data_list()
            .await
            .map_err(|e| DataError::Internal(e.to_string()))?;

        Ok(list
            .items
            .into_iter()
            .map(|item| Metadata {
                id: item.id,
                name: item.name,
                category: item.category.as_str().to_owned(),
                translation_id: item.translation_id,
                groups: item.groups.unwrap_or_default(),
                tags: item.tags.unwrap_or_default(),
                schema: None,
                is_readable: true,
                is_writable: false,
            })
            .collect())
    }

    async fn read(&self, data_id: &str, _include_schema: bool) -> Result<Data, DataError> {
        let response = if self.is_app {
            self.client.app(&self.entity_id).data(data_id).read().send().await
        } else {
            self.client.component(&self.entity_id).data(data_id).read().send().await
        }
        .map_err(|e| DataError::Internal(e.to_string()))?;

        Ok(Data { data: response.data, schema: response.schema })
    }

    async fn write(&self, data_id: &str, value: serde_json::Value) -> Result<(), DataError> {
        if self.is_app {
            let entity = self.client.app(&self.entity_id);
            entity
                .data(data_id)
                .write(&value)
                .map_err(|e| DataError::Internal(e.to_string()))?
                .send()
                .await
                .map_err(|e| DataError::Internal(e.to_string()))
        } else {
            let entity = self.client.component(&self.entity_id);
            entity
                .data(data_id)
                .write(&value)
                .map_err(|e| DataError::Internal(e.to_string()))?
                .send()
                .await
                .map_err(|e| DataError::Internal(e.to_string()))
        }
    }
}


// Proxies fault operations to the originating slave over HTTP.
pub(crate) struct RemoteFaultProvider {
    client: Arc<Client>,
    entity_id: String,
}

impl RemoteFaultProvider {
    pub(crate) fn for_component(client: Arc<Client>, entity_id: impl Into<String>) -> Self {
        Self { client, entity_id: entity_id.into() }
    }
}

// Converts an opensovd-models `Fault` (bool status fields) into a `FaultInfo`
// (HashMap status flags in "0"/"1" encoding expected by the core trait).
fn fault_model_to_info(f: Fault) -> FaultInfo {
    FaultInfo {
        code: f.code,
        display_code: f.display_code,
        scope: f.scope,
        fault_name: f.fault_name,
        severity: f.severity,
        status: f.status.map(|s| {
            let mut map = HashMap::new();
            for (key, val) in [
                ("testFailed", s.test_failed),
                ("testFailedThisOperationCycle", s.test_failed_this_operation_cycle),
                ("pendingDTC", s.pending_dtc),
                ("confirmedDTC", s.confirmed_dtc),
                ("testNotCompletedSinceLastClear", s.test_not_completed_since_last_clear),
                ("testFailedSinceLastClear", s.test_failed_since_last_clear),
                ("testNotCompletedThisOperationCycle", s.test_not_completed_this_operation_cycle),
                ("warningIndicatorRequested", s.warning_indicator_requested),
            ] {
                if let Some(v) = val {
                    map.insert(key.to_string(), if v { "1" } else { "0" }.to_string());
                }
            }
            map
        }),
    }
}

#[async_trait]
impl FaultProvider for RemoteFaultProvider {
    async fn list(&self, filter: FaultFilter) -> FaultResult<Vec<FaultInfo>> {
        let severity_str = filter.severity.map(|s| s.to_string());
        let mut query: Vec<(&str, &str)> = Vec::new();
        if let Some(ref s) = severity_str {
            query.push(("severity", s.as_str()));
        }
        if let Some(ref sc) = filter.scope {
            query.push(("scope", sc.as_str()));
        }

        let items: Items<Fault> = self
            .client
            .get(&format!("/components/{}/faults", self.entity_id), &query)
            .await
            .map_err(|e| FaultError::Internal(e.to_string()))?;

        let infos: Vec<FaultInfo> = items.items.into_iter().map(fault_model_to_info).collect();

        // The status filter cannot easily be encoded as a nested query param;
        // apply it client-side instead.
        if let Some(status_key) = &filter.status {
            Ok(infos
                .into_iter()
                .filter(|f| {
                    f.status
                        .as_ref()
                        .and_then(|s| s.get(status_key.as_str()))
                        .map(|v| v == "1")
                        .unwrap_or(false)
                })
                .collect())
        } else {
            Ok(infos)
        }
    }

    async fn get(&self, fault_code: &str) -> FaultResult<FaultInfo> {
        let response: FaultResponse = self
            .client
            .get(
                &format!("/components/{}/faults/{}", self.entity_id, fault_code),
                &[],
            )
            .await
            .map_err(|e| FaultError::Internal(e.to_string()))?;

        Ok(fault_model_to_info(response.items))
    }

    async fn clear_all(&self, scope: Option<&str>) -> FaultResult<()> {
        let query: Vec<(&str, &str)> = scope.into_iter().map(|s| ("scope", s)).collect();
        self.client
            .delete(&format!("/components/{}/faults", self.entity_id), &query)
            .await
            .map_err(|e| FaultError::Internal(e.to_string()))
    }

    async fn clear(&self, fault_code: &str) -> FaultResult<()> {
        self.client
            .delete(
                &format!("/components/{}/faults/{}", self.entity_id, fault_code),
                &[],
            )
            .await
            .map_err(|e| FaultError::Internal(e.to_string()))
    }
}



// Pulls components, apps and areas from a slave.
pub(crate) async fn fetch_entities_from_slave(
    client: &Arc<Client>,
    label: &str,
) -> Result<EntityCollection, DiscoveryError> {
    let raw_components = client
        .list_components()
        .send()
        .await
        .map_err(|e| DiscoveryError::Transport(e.to_string()))?;

    let raw_apps = client
        .list_apps()
        .send()
        .await
        .map_err(|e| DiscoveryError::Transport(e.to_string()))?;

    let raw_areas = client
        .list_areas()
        .send()
        .await
        .map_err(|e| DiscoveryError::Transport(e.to_string()))?;

    let mut components: Vec<Component> = Vec::new();
    for item in raw_components.data.items {
        tracing::info!("[{}] component '{}'", label, item.id);

        let caps = client
            .component(&item.id)
            .capabilities()
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(
                    "[{label}] capabilities fetch failed for '{}': {e}",
                    item.id
                );
                EntityCapabilities::default()
            });

        let mut comp = Component::new(&item.id, &item.name);

        if caps.data.is_some() {
            comp = comp.with_data_provider(RemoteDataProvider::for_component(
                Arc::clone(client),
                &item.id,
            ));
        }
        if caps.faults.is_some() {
            comp = comp.with_fault_provider(RemoteFaultProvider::for_component(
                Arc::clone(client),
                &item.id,
            ));
        }

        components.push(comp);
    }

    let apps: Vec<App> = raw_apps
        .data
        .items
        .into_iter()
        .map(|item| {
            tracing::info!("[{}] app '{}'", label, item.id);
            let provider = RemoteDataProvider::for_app(Arc::clone(client), &item.id);
            App::new(&item.id, &item.name, "unknown").with_data_provider(provider)
        })
        .collect();

    let areas = raw_areas
        .data
        .items
        .into_iter()
        .map(|item| {
            tracing::info!("[{}] area '{}'", label, item.id);
            opensovd_core::Area::new(&item.id, &item.name)
        })
        .collect();

    Ok(EntityCollection { components, apps, areas })
}
