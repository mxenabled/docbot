use crate::DeploymentHook;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::PodTemplate;
use kube::{api::ListParams, client::Client, Api};
use lru::LruCache;
use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct PodTemplateService {
    cache: Arc<Mutex<LruCache<(String, String), PodTemplate>>>,
    client: Client,
}

impl PodTemplateService {
    pub fn new(client: Client) -> Self {
        let cache = Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(1024).unwrap())));

        Self { cache, client }
    }

    pub async fn get(
        &self,
        name: &str,
        namespace: &str,
    ) -> Result<Option<PodTemplate>, Box<dyn std::error::Error>> {
        let mut locked = self.cache.lock().await;

        // Check the LRU cache for the pod template.
        let cache_key = (namespace.to_string(), name.to_string());
        if let Some(pod_template) = locked.get(&cache_key) {
            return Ok(Some(pod_template.clone()));
        }

        // Otherwise we should pull directly from the API.
        let pod_template_api: Api<PodTemplate> = Api::namespaced(self.client.clone(), namespace);
        let pod_template = pod_template_api.get(name).await?;

        // Now fill the cache so we can avoid an API call next time.
        locked.push(cache_key, pod_template.clone());

        Ok(Some(pod_template))
    }
}
