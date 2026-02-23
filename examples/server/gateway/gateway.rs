// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0

/* 
    Master gateway that discovers slave SOVD servers via mDNS and aggregates
    their topologies into a single endpoint on `127.0.0.1:9100`.
    Run with: `cargo run -p opensovd-examples-server --example gateway`
*/

#![allow(clippy::expect_used)]

mod dfm;
mod discovery;
mod remote;
mod slaves;

use std::{sync::Arc, time::Duration};

use mdns_sd::ServiceDaemon;
use opensovd_server::Server;
use tokio::net::TcpListener;

use dfm::dfm_topology;
use discovery::MdnsDiscoveryProvider;
use slaves::{body_topology, ecu_topology, spawn_slave};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    libcli::init_tracing("info", None)?;

    let reg_daemon = Arc::new(ServiceDaemon::new()?);

    let _ecu = spawn_slave(ecu_topology().await, "ecu-slave", &reg_daemon).await;
    let body = spawn_slave(body_topology().await, "body-slave", &reg_daemon).await;
    let _dfm = spawn_slave(dfm_topology().await, "dfm-slave", &reg_daemon).await;

    // Simulate body-slave going offline after 20 s. Unregistering sends a
    // goodbye packet so the master removes its entities immediately.
    let daemon = Arc::clone(&reg_daemon);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(20)).await;
        tracing::warn!("body-slave going offline");
        body.abort();
        let _ = daemon.unregister("body-slave._sovd._tcp.local.");
    });

    let listener = TcpListener::bind("0.0.0.0:9100").await?;
    tracing::info!("master gateway on 0.0.0.0:9100");

    let server = Server::builder()
        .listener(listener)
        .base_uri("http://0.0.0.0:9100/sovd")?
        .discovery(Box::new(MdnsDiscoveryProvider))
        .build()?;

    server.serve().await?;
    Ok(())
}
