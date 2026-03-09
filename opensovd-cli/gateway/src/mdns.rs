// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0

use std::net::IpAddr;
use std::sync::Arc;

use opensovd_providers::mdns::{MdnsDiscoveryProvider, MdnsError, MdnsWrapper};

use crate::cli::MdnsArgs;

const TARGET: &str = "mdns";

// Returns the IP to advertise on mDNS.
pub fn resolve_host(args: &MdnsArgs, url_host: &str) -> Option<IpAddr> {
    if let Some(ip) = args.host {
        return Some(ip);
    }
    url_host.parse::<IpAddr>().ok()
}

/* 
    Creates the mDNS daemon, registers this server, and returns the
    wrapper (keep alive for the process lifetime) and discovery provider.
*/ 
pub fn setup(
    args: &MdnsArgs,
    url_host: &str,
    port: u16,
    base_path: &str,
) -> Result<(Arc<MdnsWrapper>, MdnsDiscoveryProvider), MdnsError> {
    let wrapper = Arc::new(MdnsWrapper::new()?);

    match resolve_host(args, url_host) {
        Some(ip) => {
            let identification = args
                .identification
                .as_deref()
                .unwrap_or(args.name.as_str());
            let access_url = format!("http://{ip}:{port}{base_path}");

            if let Err(e) =
                wrapper.register(&args.name, identification, &access_url, ip, port)
            {
                tracing::warn!(
                    target: TARGET,
                    error = %e,
                    "mDNS registration failed discovery will still run"
                );
            }
        }
        None => {
            tracing::warn!(
                target: TARGET,
                "Cannot determine advertise IP — use --mdns-host to set it explicitly. \
                 mDNS discovery will still run."
            );
        }
    }

    let provider = MdnsDiscoveryProvider::from_wrapper(Arc::clone(&wrapper));
    Ok((wrapper, provider))
}
