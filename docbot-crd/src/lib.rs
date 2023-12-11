use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::PodTemplate;
use k8s_openapi::api::core::v1::PodTemplateSpec;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::CustomResource;
use kube::{client::Client, Api};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tracing::{debug, info};

/// The default job ttl is 72 hours.
fn default_job_ttl_seconds_after_finished() -> Option<i32> {
    Some(259200)
}

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
    pub selector: DeploymentSelector,
    pub template: InternalPodTemplate,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentSelector {
    pub labels: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InternalPodTemplate {
    #[serde(default = "default_job_ttl_seconds_after_finished")]
    pub ttl_seconds_after_finished: Option<i32>,
    pub name: Option<String>,
    pub spec: Option<PodTemplateSpec>,
}

impl DeploymentHook {
    pub async fn get_pod_template(
        &self,
        client: Client,
    ) -> Result<PodTemplate, Box<dyn std::error::Error>> {
        // Check to see if the template was embedded in the struct.
        if let Some(ref template) = self.spec.template.spec {
            // HACK: Mock a PodTemplate for now to keep things simple.
            return Ok(PodTemplate {
                metadata: ObjectMeta {
                    namespace: self.metadata.namespace.clone(),
                    ..ObjectMeta::default()
                },
                template: Some(template.clone()),
            });
        }
        // Otherwise use the name to look it up via the k8s api.
        let pod_template_api: Api<PodTemplate> = Api::namespaced(
            client,
            &self
                .metadata
                .namespace
                .clone()
                .unwrap_or_else(|| "default".to_string()),
        );

        if let Some(ref name) = self.spec.template.name {
            let specific_pod_template = pod_template_api.get(&name).await?;
            let dep_time = self
                .metadata
                .creation_timestamp
                .clone()
                .expect("No timestamp in deployment");
            let pod_time = specific_pod_template
                .metadata
                .creation_timestamp
                .clone()
                .expect("No timestamp in podTemplate");
            info!(
                "Deployment hook time: {:?} vs podTemplate time: {:?}",
                dep_time, pod_time
            );
            // Print containers and their images
            if let Some(template) = &specific_pod_template.template {
                if let Some(pod_spec) = &template.spec {
                    for container in &pod_spec.containers {
                        info!("Container Image for template {} in namespace {:?} from k8s api {} : {:?}",
                        name,
                        self.metadata.namespace,
                        container.name,
                        container.image);
                    }
                }
            } else {
                info!("No PodTemplate spec found for '{}'", name);
            }
            return Ok(specific_pod_template);
        }

        Err(format!(
            "Could not find a way to return a pod template for deployment hook {:?}",
            self.metadata.name
        )
        .into())
    }

    pub fn does_match_deployment(&self, deployment: &Deployment) -> bool {
        if let Some(ref labels) = deployment.metadata.labels {
            self.spec
                .selector
                .labels
                .iter()
                .all(|hook_label| labels.get_key_value(hook_label.0) == Some(hook_label))
        } else {
            false
        }
    }
}
