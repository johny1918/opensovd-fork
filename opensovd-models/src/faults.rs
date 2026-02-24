use serde::{Deserialize, Serialize};


#[derive(Debug, Serialize, Deserialize)]
pub struct FaultQueryParams {
    // Filters the available elements based on a key from status
    pub status: Option<StatusWrapper>,
    // Filter fault entries by their severity
    pub severity: Option<u32>,
    // Scope fault entries retrival
    pub scope: Option<String>,
    // Specify if schema is provided or not
    pub include_schema: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusWrapper {
    pub key: FaultStatusKeys,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultStatusKeys {
    ConfirmedDtc,
    Mask,
    PendingDtc,
    TestFailed,
    TestFailedSinceLastClear,
    TestFailedThisOperationCycle,
    TestNotCompletedSinceLastClear,
    TestNotCompletedThisOperationCycle,
    WarningIndicatorRequested,
}

pub struct FaultData {
    // Filters the available elements based on a key from status
    pub status: Option<StatusWrapper>,
    // Filter fault entries by their severity
    pub severity: Option<u32>,
    // Scope fault entries retrival
    pub scope: Option<String>,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub struct FaultResponse {
    pub items: Fault,
    pub schema: Option<schemars::Schema>,
}

 #[derive(Serialize, Deserialize, schemars::JsonSchema)]
pub struct Fault {
    ///Fault code native representation of entity.
    pub code: String,
    //  Defines the scope, where capability description defines which scopes are supported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Display representation of the fault code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_code: Option<String>,
    /// Name /description of the fault code.
    pub fault_name: String,
    /// Severity defines the impact of the fault on the system.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<u32>,
    /// Detailed status information as key value pairs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<FaultStatus>,
}

#[derive(Serialize, Deserialize, Debug, schemars::JsonSchema)]
pub struct FaultStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_failed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_failed_this_operation_cycle: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_dtc: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmed_dtc: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_not_completed_since_last_clear: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_failed_since_last_clear: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_not_completed_this_operation_cycle: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning_indicator_requested: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mask: Option<String>,
}
