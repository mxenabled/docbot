use crate::crd::DeploymentHook;
use k8s_openapi::api::apps::v1::Deployment;
use kube::{api::ListParams, client::Client, Api};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Default, Debug, Clone)]
pub struct DeploymentHookCache {
    cache: Arc<Mutex<BTreeMap<(String, String), DeploymentHook>>>,
}

impl DeploymentHookCache {
    pub async fn refresh(&self, client: &Client) -> Result<(), Box<dyn std::error::Error>> {
        let api: Api<DeploymentHook> = Api::all(client.clone());
        let hooks: BTreeMap<(String, String), DeploymentHook> = api
            .list(&ListParams::default())
            .await?
            .items
            .iter()
            .map(|hook| {
                (
                    (
                        hook.metadata.namespace.clone().expect("must have a name"),
                        hook.metadata.name.clone().expect("must have a namespace"),
                    ),
                    hook.clone(),
                )
            })
            .collect();

        let mut cache = self.cache.lock().unwrap();
        *cache = hooks;
        Ok(())
    }

    pub fn find_by_matching_deployment(&self, deployment: &Deployment) -> Vec<DeploymentHook> {
        let cache = self.cache.lock().unwrap();
        cache
            .values()
            .filter(|hook| hook.does_match_deployment(deployment))
            .cloned()
            .collect()
    }
}
