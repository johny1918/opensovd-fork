// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_trait::async_trait;
use futures_core::Stream;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use opensovd_core::{
    Component, DiscoveryError, DiscoveryProvider, DiscoveryStream, EntityCollection, EntityRef,
};
use tokio::sync::mpsc;

type DiscoveryResult<T> = std::result::Result<T, DiscoveryError>;

// mDNS service type for SOVD servers.
pub const SERVICE_TYPE: &str = "_sovd._tcp.local.";

pub const TXT_IDENTIFICATION: &str = "identification";
pub const TXT_ACCESS_URL: &str = "accessurl";

// Errors returned by MdnsWrapper operations.
#[derive(Debug, thiserror::Error)]
pub enum MdnsError {
    #[error("mdns-sd daemon error: {0}")]
    Daemon(#[from] mdns_sd::Error),
    #[error("invalid service info: {0}")]
    ServiceInfo(String),
}

/* 
    Thin wrapper around the mdns-sd ServiceDaemon.
    Create one per process and wrap in Arc to share between
    register(MdnsWrapper::register) and MdnsDiscoveryProvider.
*/
pub struct MdnsWrapper {
    daemon: ServiceDaemon,
}

impl MdnsWrapper {
    pub fn new() -> Result<Self, MdnsError> {
        let daemon = ServiceDaemon::new()?;
        Ok(Self { daemon })
    }

    /* 
        Registers this process as a SOVD server on the local network.
        Sets the identification and accessurl TXT records.
    */
    pub fn register(
        &self,
        instance_name: &str,
        identification: &str,
        access_url: &str,
        host_ip: IpAddr,
        port: u16,
    ) -> Result<(), MdnsError> {
        let host_name = format!("{instance_name}.local.");
        let txt = [
            (TXT_IDENTIFICATION, identification),
            (TXT_ACCESS_URL, access_url),
        ];
        let service_info = ServiceInfo::new(
            SERVICE_TYPE,
            instance_name,
            &host_name,
            host_ip,
            port,
            &txt[..],
        )
        .map_err(|e| MdnsError::ServiceInfo(e.to_string()))?;
        self.daemon.register(service_info)?;
        tracing::info!(
            target: "mdns",
            service = %instance_name,
            %identification,
            %access_url,
            ip = %host_ip,
            %port,
            "Registered SOVD service on mDNS"
        );
        Ok(())
    }

    // Starts browsing for _sovd._tcp.local. services.
    pub fn browse(&self) -> Result<mdns_sd::Receiver<ServiceEvent>, MdnsError> {
        let receiver = self.daemon.browse(SERVICE_TYPE)?;
        Ok(receiver)
    }

    // Shuts down the mDNS daemon and unregisters all services.
    pub fn shutdown(&self) -> Result<(), MdnsError> {
        self.daemon.shutdown()?;
        Ok(())
    }
}

// Implements DiscoveryProvider using mDNS-SD.
pub struct MdnsDiscoveryProvider {
    wrapper: Arc<MdnsWrapper>,
}

impl MdnsDiscoveryProvider {
    pub fn new() -> Result<Self, MdnsError> {
        Ok(Self {
            wrapper: Arc::new(MdnsWrapper::new()?),
        })
    }

    /// Creates a discovery provider sharing an existing MdnsWrapper.
    #[must_use]
    pub fn from_wrapper(wrapper: Arc<MdnsWrapper>) -> Self {
        Self { wrapper }
    }
}

#[async_trait]
impl DiscoveryProvider for MdnsDiscoveryProvider {
    async fn discover(&self) -> DiscoveryResult<DiscoveryStream> {
        let receiver = self
            .wrapper
            .browse()
            .map_err(|e| DiscoveryError::Transport(e.to_string()))?;

        let (tx, rx) = mpsc::channel::<DiscoveryResult<(Vec<EntityRef>, EntityCollection)>>(32);

        tokio::task::spawn_blocking(move || {
            while let Ok(event) = receiver.recv() {
                let Some(diff) = convert_event(event) else {
                    continue;
                };
                if tx.blocking_send(Ok(diff)).is_err() {
                    break;
                }
            }
        });

        Ok(Box::pin(MpscStream(rx)))
    }
}

fn convert_event(event: ServiceEvent) -> Option<(Vec<EntityRef>, EntityCollection)> {
    match event {
        ServiceEvent::ServiceResolved(info) => {
            let id = info.get_fullname().to_string();
            let name = info.get_hostname().trim_end_matches('.').to_string();
            let port = info.get_port();

            let access_url = info
                .get_properties()
                .get(TXT_ACCESS_URL)
                .map(|p| p.val_str().to_string())
                .unwrap_or_else(|| {
                    info.get_addresses()
                        .iter()
                        .next()
                        .map_or_else(
                            || format!("http://{name}:{port}/sovd"),
                            |ip| format!("http://{ip}:{port}/sovd"),
                        )
                });

            let mut metadata = HashMap::new();
            metadata.insert(TXT_ACCESS_URL.to_string(), access_url.clone());

            if let Some(ident) = info
                .get_properties()
                .get(TXT_IDENTIFICATION)
                .map(|p| p.val_str())
            {
                metadata.insert(TXT_IDENTIFICATION.to_string(), ident.to_string());
            }

            let component = Component::new(id, name).with_metadata(metadata);
            tracing::info!(target: "mdns", %access_url, "Discovered SOVD server");

            let mut collection = EntityCollection::default();
            collection.add_component(component);
            Some((vec![], collection))
        }

        ServiceEvent::ServiceRemoved(_service_type, fullname) => {
            tracing::info!(target: "mdns", service = %fullname, "SOVD server left network");
            Some((
                vec![EntityRef::component(fullname)],
                EntityCollection::default(),
            ))
        }

        _ => None,
    }
}

struct MpscStream<T>(mpsc::Receiver<T>);

impl<T> Stream for MpscStream<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        self.0.poll_recv(cx)
    }
}
