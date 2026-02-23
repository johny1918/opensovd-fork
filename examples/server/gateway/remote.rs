// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use async_trait::async_trait;
use opensovd_client::Client;
use opensovd_core::{
    App, Component, Data, DataError, DataFilter, DataProvider, DiscoveryError, EntityCollection,
    Metadata,
};
use opensovd_models::data::DataList;

// proxies data ops to the originating slave over HTTP
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

// pulls components, apps and areas from a slave and wraps each with a RemoteDataProvider
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

    let components: Vec<Component> = raw_components
        .data
        .items
        .into_iter()
        .map(|item| {
            tracing::info!("[{}] component '{}'", label, item.id);
            let provider = RemoteDataProvider::for_component(Arc::clone(client), &item.id);
            Component::new(&item.id, &item.name).with_data_provider(provider)
        })
        .collect();

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
