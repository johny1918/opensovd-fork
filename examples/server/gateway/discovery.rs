// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use futures::channel::mpsc;
use mdns_sd::{ServiceDaemon, ServiceEvent};
use opensovd_client::Client;
use opensovd_core::{
    DiscoveryError, DiscoveryProvider, DiscoveryStream, EntityCollection, EntityRef,
};

use crate::remote::fetch_entities_from_slave;

// browses _sovd._tcp.local. and emits add/remove events as slaves come and go
pub(crate) struct MdnsDiscoveryProvider;

#[async_trait]
impl DiscoveryProvider for MdnsDiscoveryProvider {
    async fn discover(&self) -> Result<DiscoveryStream, DiscoveryError> {
        let (tx, rx) = mpsc::unbounded();

        tokio::spawn(async move {
            let mdns = match ServiceDaemon::new() {
                Ok(d) => d,
                Err(e) => {
                    let _ = tx.unbounded_send(Err(DiscoveryError::Transport(e.to_string())));
                    return;
                }
            };

            let receiver = match mdns.browse("_sovd._tcp.local.") {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.unbounded_send(Err(DiscoveryError::Transport(e.to_string())));
                    return;
                }
            };

            // mdns-sd's Receiver is blocking; bridge to an async channel.
            let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<ServiceEvent>();
            std::thread::spawn(move || {
                while let Ok(event) = receiver.recv() {
                    if event_tx.send(event).is_err() {
                        break;
                    }
                }
            });

            let mut known: HashMap<String, Vec<EntityRef>> = HashMap::new();

            loop {
                let event = match event_rx.recv().await {
                    Some(e) => e,
                    None => break,
                };

                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        let fullname = info.get_fullname().to_string();
                        if known.contains_key(&fullname) {
                            continue;
                        }

                        let ip = match info.get_addresses().iter().find(|a| a.is_ipv4()) {
                            Some(addr) => addr.to_string(),
                            None => {
                                tracing::warn!("[mDNS] {} has no IPv4 address", fullname);
                                continue;
                            }
                        };
                        let url = format!("http://{}:{}/sovd/v1", ip, info.get_port());
                        tracing::info!("[mDNS] {} → {}", fullname, url);

                        let client = match Client::connect(&url) {
                            Ok(c) => Arc::new(c),
                            Err(e) => {
                                tracing::warn!("[mDNS] connect to {} failed: {}", url, e);
                                continue;
                            }
                        };

                        match fetch_entities_from_slave(&client, &fullname).await {
                            Ok(collection) => {
                                let refs = collection.entity_refs();
                                known.insert(fullname, refs);
                                if tx.unbounded_send(Ok((vec![], collection))).is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::warn!("[mDNS] fetch from {} failed: {}", fullname, e);
                            }
                        }
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => {
                        if let Some(refs) = known.remove(&fullname) {
                            tracing::warn!("[mDNS] {} removed ({} entities)", fullname, refs.len());
                            if tx
                                .unbounded_send(Ok((refs, EntityCollection::default())))
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }

            drop(mdns);
        });

        Ok(Box::pin(rx))
    }
}
