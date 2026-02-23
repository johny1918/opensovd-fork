// Copyright (c) 2025 Contributors to the Eclipse Foundation
//
// See the NOTICE file(s) distributed with this work for additional
// information regarding copyright ownership.
//
// This program and the accompanying materials are made available under the
// terms of the Apache License Version 2.0 which is available at
// <https://www.apache.org/licenses/LICENSE-2.0>
//
// SPDX-License-Identifier: Apache-2.0

//! Standalone in-memory mock of the Diagnostic Fault Manager.
//!
//! No external dependencies — copy this crate into any repo and use it
//! to test SOVD fault endpoints without IPC or persistent storage.
//!
//! ```rust
//! use mock_dfm::MockSovdFaultManager;
//!
//! let mgr = MockSovdFaultManager::new();
//!
//! let faults = mgr.get_all_faults("hvac").unwrap();
//! let (fault, env) = mgr.get_fault("hvac", "ENGINE_OVERTEMP").unwrap();
//!
//! println!("{} severity={} confirmed={}", fault.fault_name, fault.severity, fault.status["confirmedDTC"]);
//! ```

use std::collections::HashMap;

// ─── Types (mirror dfm_lib's public surface) ─────────────────────────────────

/// Mirrors `dfm_lib::sovd_fault_manager::SovdFault`.
#[derive(Debug, Clone, PartialEq)]
pub struct SovdFault {
    pub code: String,
    pub display_code: String,
    pub scope: String,
    pub fault_name: String,
    pub fault_translation_id: String,
    /// Fault severity as a `u32` matching `FaultSeverity` ordinal values:
    /// Trace=0, Debug=1, Info=2, Warn=3, Error=4, Fatal=5.
    pub severity: u32,
    /// DTC status flags encoded as `"0"` / `"1"` strings.
    /// Keys: `testFailed`, `testFailedThisOperationCycle`, `testFailedSinceLastClear`,
    /// `testNotCompletedThisOperationCycle`, `testNotCompletedSinceLastClear`,
    /// `pendingDTC`, `confirmedDTC`, `warningIndicatorRequested`.
    pub status: HashMap<String, String>,
}

/// Mirrors `dfm_lib::sovd_fault_manager::SovdEnvData`.
pub type SovdEnvData = HashMap<String, String>;

/// Mirrors `dfm_lib::sovd_fault_manager::Error`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    BadArgument,
    Generic,
}

// ─── MockSovdFaultManager ────────────────────────────────────────────────────

/// Pre-populated mock that mirrors `SovdFaultManager`'s public API.
///
/// Contains HVAC (`"hvac"`) and IVI (`"ivi"`) paths out of the box.
/// Call [`add`](MockSovdFaultManager::add) to inject additional faults.
pub struct MockSovdFaultManager {
    /// path → fault_code → (fault, env_data)
    data: HashMap<String, HashMap<String, (SovdFault, SovdEnvData)>>,
}

impl Default for MockSovdFaultManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MockSovdFaultManager {
    /// Creates the manager pre-loaded with sample HVAC and IVI faults.
    pub fn new() -> Self {
        let mut mgr = Self { data: HashMap::new() };
        mgr.load_samples();
        mgr
    }

    /// Returns all faults for `path`, or `Error::BadArgument` if the path is unknown.
    pub fn get_all_faults(&self, path: &str) -> Result<Vec<SovdFault>, Error> {
        let map = self.data.get(path).ok_or(Error::BadArgument)?;
        Ok(map.values().map(|(f, _)| f.clone()).collect())
    }

    /// Returns a single fault and its env data, or an error if not found.
    pub fn get_fault(&self, path: &str, fault_code: &str) -> Result<(SovdFault, SovdEnvData), Error> {
        let map = self.data.get(path).ok_or(Error::BadArgument)?;
        let (fault, env) = map.get(fault_code).ok_or(Error::Generic)?;
        Ok((fault.clone(), env.clone()))
    }

    /// Inject a custom fault under `path`. `fault.code` is used as the lookup key.
    pub fn add(&mut self, path: &str, fault: SovdFault, env: SovdEnvData) {
        self.data
            .entry(path.to_string())
            .or_default()
            .insert(fault.code.clone(), (fault, env));
    }

    fn load_samples(&mut self) {
        // ── HVAC ─────────────────────────────────────────────────────────────
        self.add(
            "hvac",
            SovdFault {
                code: "ENGINE_OVERTEMP".into(),
                display_code: "ENGINE_OVERTEMP".into(),
                scope: "hvac".into(),
                fault_name: "Engine Over-Temperature".into(),
                fault_translation_id: String::new(),
                severity: 5, // Fatal
                status: dtc_status(true, true, true, false, false, false, true, true),
            },
            HashMap::from([("coolant_temp_c".into(), "118".into()), ("ambient_temp_c".into(), "28".into())]),
        );
        self.add(
            "hvac",
            SovdFault {
                code: "BLOWER_MOTOR_FAIL".into(),
                display_code: "BLOWER_MOTOR_FAIL".into(),
                scope: "hvac".into(),
                fault_name: "Blower Motor Failure".into(),
                fault_translation_id: String::new(),
                severity: 4, // Error
                status: dtc_status(false, false, true, false, false, false, false, false),
            },
            HashMap::new(),
        );
        self.add(
            "hvac",
            SovdFault {
                code: "REFRIGERANT_LEAK".into(),
                display_code: "REFRIGERANT_LEAK".into(),
                scope: "hvac".into(),
                fault_name: "Refrigerant Leak Detected".into(),
                fault_translation_id: String::new(),
                severity: 3, // Warn
                status: dtc_status(true, false, true, false, false, true, false, false),
            },
            HashMap::from([("pressure_bar".into(), "0.3".into())]),
        );

        // ── IVI ───────────────────────────────────────────────────────────────
        self.add(
            "ivi",
            SovdFault {
                code: "DISPLAY_TIMEOUT".into(),
                display_code: "DISPLAY_TIMEOUT".into(),
                scope: "ivi".into(),
                fault_name: "Display Timeout".into(),
                fault_translation_id: String::new(),
                severity: 3, // Warn
                status: dtc_status(true, false, true, true, false, true, false, false),
            },
            HashMap::from([("last_seen_ts".into(), "1700000000".into())]),
        );
        self.add(
            "ivi",
            SovdFault {
                code: "AUDIO_CODEC_ERR".into(),
                display_code: "AUDIO_CODEC_ERR".into(),
                scope: "ivi".into(),
                fault_name: "Audio Codec Error".into(),
                fault_translation_id: String::new(),
                severity: 4, // Error
                status: dtc_status(true, true, true, false, false, false, true, false),
            },
            HashMap::new(),
        );
        self.add(
            "ivi",
            SovdFault {
                code: "TOUCH_UNRESPONSIVE".into(),
                display_code: "TOUCH_UNRESPONSIVE".into(),
                scope: "ivi".into(),
                fault_name: "Touchscreen Unresponsive".into(),
                fault_translation_id: String::new(),
                severity: 4, // Error
                status: dtc_status(false, false, false, true, true, false, false, false),
            },
            HashMap::new(),
        );
    }
}

/// Build the standard DTC status map used by the real `SovdFaultManager`.
///
/// Argument order:
/// `test_failed`, `test_failed_this_op_cycle`, `test_failed_since_last_clear`,
/// `test_not_completed_this_op_cycle`, `test_not_completed_since_last_clear`,
/// `pending_dtc`, `confirmed_dtc`, `warning_indicator_requested`
pub fn dtc_status(
    test_failed: bool,
    test_failed_this_op_cycle: bool,
    test_failed_since_last_clear: bool,
    test_not_completed_this_op_cycle: bool,
    test_not_completed_since_last_clear: bool,
    pending_dtc: bool,
    confirmed_dtc: bool,
    warning_indicator_requested: bool,
) -> HashMap<String, String> {
    HashMap::from([
        ("testFailed".into(), (test_failed as u32).to_string()),
        ("testFailedThisOperationCycle".into(), (test_failed_this_op_cycle as u32).to_string()),
        ("testFailedSinceLastClear".into(), (test_failed_since_last_clear as u32).to_string()),
        ("testNotCompletedThisOperationCycle".into(), (test_not_completed_this_op_cycle as u32).to_string()),
        ("testNotCompletedSinceLastClear".into(), (test_not_completed_since_last_clear as u32).to_string()),
        ("pendingDTC".into(), (pending_dtc as u32).to_string()),
        ("confirmedDTC".into(), (confirmed_dtc as u32).to_string()),
        ("warningIndicatorRequested".into(), (warning_indicator_requested as u32).to_string()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_all_faults_hvac() {
        let mgr = MockSovdFaultManager::new();
        assert_eq!(mgr.get_all_faults("hvac").unwrap().len(), 3);
    }

    #[test]
    fn get_all_faults_ivi() {
        let mgr = MockSovdFaultManager::new();
        assert_eq!(mgr.get_all_faults("ivi").unwrap().len(), 3);
    }

    #[test]
    fn get_fault_engine_overtemp() {
        let mgr = MockSovdFaultManager::new();
        let (fault, env) = mgr.get_fault("hvac", "ENGINE_OVERTEMP").unwrap();
        assert_eq!(fault.fault_name, "Engine Over-Temperature");
        assert_eq!(fault.severity, 5);
        assert_eq!(fault.status["confirmedDTC"], "1");
        assert_eq!(fault.status["testFailed"], "1");
        assert_eq!(env["coolant_temp_c"], "118");
    }

    #[test]
    fn unknown_path_returns_bad_argument() {
        let mgr = MockSovdFaultManager::new();
        assert_eq!(mgr.get_all_faults("unknown"), Err(Error::BadArgument));
    }

    #[test]
    fn unknown_code_returns_generic_error() {
        let mgr = MockSovdFaultManager::new();
        assert_eq!(mgr.get_fault("hvac", "NOPE"), Err(Error::Generic));
    }

    #[test]
    fn custom_fault_injected_via_add() {
        let mut mgr = MockSovdFaultManager::new();
        mgr.add(
            "my_ecu",
            SovdFault {
                code: "SENSOR_FAIL".into(),
                display_code: "SENSOR_FAIL".into(),
                scope: "my_ecu".into(),
                fault_name: "Sensor Failure".into(),
                fault_translation_id: String::new(),
                severity: 4,
                status: dtc_status(true, false, true, false, false, false, true, false),
            },
            HashMap::from([("sensor_id".into(), "42".into())]),
        );

        let (fault, env) = mgr.get_fault("my_ecu", "SENSOR_FAIL").unwrap();
        assert_eq!(fault.severity, 4);
        assert_eq!(fault.status["confirmedDTC"], "1");
        assert_eq!(env["sensor_id"], "42");
    }
}
