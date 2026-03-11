// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::expect_used)]

/*
    Register and discover SOVD servers on the local network via mDNS-SD.

    Run two instances on different ports to see discovery in action:

    cargo run -p opensovd-examples-server --example mdns --features mdns -- \
        --addr 10.0.0.5:7690 --name server-a

    cargo run -p opensovd-examples-server --example mdns --features mdns -- \
        --addr 10.0.0.5:7691 --name server-b

    Use your actual LAN IP (not 127.0.0.1 — mDNS requires a real interface).
*/
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use clap::Parser;
use opensovd_core::{Component, Topology};
use opensovd_providers::mdns::{MdnsDiscoveryProvider, MdnsWrapper};
use opensovd_server::Server;
use tokio::net::TcpListener;

#[derive(Parser)]
struct Args {
    // Local address to listen on (must be a routable LAN IP, not 127.0.0.1).
    #[arg(long, default_value = "127.0.0.1:7690")]
    addr: SocketAddr,

    // mDNS instance name for this server.
    #[arg(long, default_value = "opensovd-example")]
    name: String,

    /* 
        VIN or device ID for the mDNS TXT `identification` record.
        Defaults to --name if not set.
    */
    #[arg(long)]
    identification: Option<String>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    libcli::init_tracing("info,mdns=debug", None)?;

    let args = Args::parse();

    let wrapper = Arc::new(MdnsWrapper::new()?);

    let host_ip: IpAddr = args.addr.ip();
    let port = args.addr.port();
    let identification = args.identification.as_deref().unwrap_or(args.name.as_str());
    let access_url = format!("http://{host_ip}:{port}/sovd");
    wrapper.register(&args.name, identification, &access_url, host_ip, port)?;

    tracing::info!(name = %args.name, %identification, %access_url, "Registered on mDNS");

    let topology = Topology::new();
    {
        let mut t = topology.write().await;
        t.add_component(Component::new("local", args.name.clone()));
    }

    let provider = MdnsDiscoveryProvider::from_wrapper(Arc::clone(&wrapper));

    let listener = TcpListener::bind(args.addr).await?;
    let server = Server::builder()
        .listener(listener)
        .topology(topology)
        .discovery(Box::new(provider))
        .layer(libcli::trace::trace_layer())
        .base_uri("/sovd")?
        .build()?;

    tracing::info!(addr = %args.addr, "SOVD server started — browsing for peers");
    tracing::info!("API: http://{}/sovd/v1/components", args.addr);

    server.serve().await?;

    wrapper.shutdown()?;
    Ok(())
}
