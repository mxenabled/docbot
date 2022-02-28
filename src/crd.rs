use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Struct corresponding to the Specification (`spec`) part of the `DeploymentHook` resource,
/// directly reflects context of the `deploymenthooks.apps.mx.com.yaml` file to be found in
/// this repository.
/// The `DeploymentHook` struct will be generated by the `CustomResource` derive macro.
#[derive(CustomResource, Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[kube(
    group = "apps.mx.com",
    version = "v1",
    kind = "DeploymentHook",
    plural = "deploymenthooks",
    derive = "PartialEq",
    namespaced
)]
pub struct DeploymentHookSpec {
    pub containers: Vec<TargetContainer>,
    pub debounce_seconds: u64,
    pub selector: DeploymentSelector,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
pub struct DeploymentSelector {
    pub labels: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
pub struct TargetContainer {
    pub name: String,
    pub command: Vec<String>,
}
