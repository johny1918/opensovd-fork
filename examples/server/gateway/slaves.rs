// SPDX-FileCopyrightText: Copyright (c) 2026 Contributors to the Eclipse Foundation
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceInfo};
use mock_dfm::{MockSovdFaultManager, SovdFault, dtc_status};
use opensovd_core::{App, Component};
use opensovd_models::data::DataCategory;
use opensovd_providers::data::{Constant, DataProviderBuilder};
use opensovd_server::{Server, Topology};
use tokio::sync::Mutex;
use tokio::{net::TcpListener, task::JoinHandle};

use super::dfm::DfmFaultProvider;


// figures out the local LAN IP without sending any packets
pub(crate) fn local_ip() -> std::net::Ipv4Addr {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").expect("probe socket");
    let _ = socket.connect("8.8.8.8:80");
    match socket.local_addr() {
        Ok(addr) => match addr.ip() {
            std::net::IpAddr::V4(v4) if !v4.is_loopback() => v4,
            _ => std::net::Ipv4Addr::LOCALHOST,
        },
        Err(_) => std::net::Ipv4Addr::LOCALHOST,
    }
}

// starts a slave on a random port and advertises it on the local network
pub(crate) async fn spawn_slave(
    topology: Topology,
    instance: &str,
    reg_daemon: &ServiceDaemon,
) -> JoinHandle<()> {
    let listener = TcpListener::bind("0.0.0.0:0").await.expect("bind slave listener");
    let port = listener.local_addr().expect("local addr").port();
    let ip = local_ip();
    let ip_str = ip.to_string();

    let info = ServiceInfo::new(
        "_sovd._tcp.local.",
        instance,
        &format!("{instance}.local."),
        ip_str.as_str(),
        port,
        None,
    )
    .expect("ServiceInfo");
    reg_daemon.register(info).expect("mDNS register");

    tracing::info!("slave '{}' on {}:{}", instance, ip, port);

    let server = Server::builder()
        .listener(listener)
        .base_uri(format!("http://{ip_str}:{port}/sovd").as_str())
        .expect("valid base uri")
        .topology(topology)
        .build()
        .expect("build slave");

    let handle = tokio::spawn(async move {
        server.serve().await.expect("slave error");
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    handle
}

// ECU with a handful of live data items, an engine control app, and ECU faults
pub(crate) async fn ecu_topology() -> Topology {
    let provider = DataProviderBuilder::new()
        .read_data("voltage", "Battery Voltage", &DataCategory::CurrentData, Constant::new(12.6_f64).expect("constant"))
        .read_data("temperature", "Engine Temperature", &DataCategory::CurrentData, Constant::new(85.0_f64).expect("constant"))
        .read_data("sw.version", "Software Version", &DataCategory::IdentData, Constant::new("2.1.0").expect("constant"))
        .build()
        .expect("ecu provider");

    let mut ecu_mgr = MockSovdFaultManager::new();
    ecu_mgr.add(
        "ecu",
        SovdFault {
            code: "LOW_BATTERY_VOLTAGE".into(),
            display_code: "LOW_BATTERY_VOLTAGE".into(),
            scope: "ecu".into(),
            fault_name: "Low Battery Voltage".into(),
            fault_translation_id: String::new(),
            severity: 4, // Error
            status: dtc_status(true, true, true, false, false, false, true, false),
        },
        HashMap::from([("voltage_v".into(), "11.2".into())]),
    );
    ecu_mgr.add(
        "ecu",
        SovdFault {
            code: "ENGINE_OVERTEMP".into(),
            display_code: "ENGINE_OVERTEMP".into(),
            scope: "ecu".into(),
            fault_name: "Engine Over-Temperature".into(),
            fault_translation_id: String::new(),
            severity: 5, // Fatal
            status: dtc_status(false, false, true, true, false, false, false, false),
        },
        HashMap::new(),
    );
    ecu_mgr.add(
        "ecu",
        SovdFault {
            code: "SENSOR_FAULT".into(),
            display_code: "SENSOR_FAULT".into(),
            scope: "ecu".into(),
            fault_name: "Engine Sensor Fault".into(),
            fault_translation_id: String::new(),
            severity: 3, // Warn
            status: dtc_status(true, false, true, false, true, true, false, false),
        },
        HashMap::from([("sensor_id".into(), "temp_1".into())]),
    );
    let ecu_dfm = Arc::new(Mutex::new(ecu_mgr));

    let ecu = Component::new("ecu", "Engine Control Unit")
        .with_data_provider(provider)
        .with_fault_provider(DfmFaultProvider::new(ecu_dfm, "ecu"));

    let app_provider = DataProviderBuilder::new()
        .read_data("app.version", "App Version", &DataCategory::IdentData, Constant::new("1.0.0").expect("constant"))
        .read_data("app.status", "App Status", &DataCategory::CurrentData, Constant::new("running").expect("constant"))
        .build()
        .expect("engine_control provider");

    let engine_control =
        App::new("engine_control", "Engine Control App", "ecu").with_data_provider(app_provider);

    let topology = Topology::new();
    {
        let mut t = topology.write().await;
        t.add_component(ecu);
        t.add_app(engine_control);
    }
    topology
}

// body controller with door/light state and a body app
pub(crate) async fn body_topology() -> Topology {
    let provider = DataProviderBuilder::new()
        .read_data("door.status", "Door Status", &DataCategory::CurrentData, Constant::new("closed").expect("constant"))
        .read_data("sw.version", "Software Version", &DataCategory::IdentData, Constant::new("1.0.0").expect("constant"))
        .build()
        .expect("body provider");

    let body =
        Component::new("body_control", "Body Control Unit").with_data_provider(provider);

    let app_provider = DataProviderBuilder::new()
        .read_data("light.status", "Light Status", &DataCategory::CurrentData, Constant::new("off").expect("constant"))
        .build()
        .expect("body_app provider");

    let body_app =
        App::new("body_app", "Body Control App", "body_control").with_data_provider(app_provider);

    let topology = Topology::new();
    {
        let mut t = topology.write().await;
        t.add_component(body);
        t.add_app(body_app);
    }
    topology
}
