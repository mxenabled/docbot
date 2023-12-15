use futures::TryStreamExt;
use k8s_openapi::api::core::v1::PodTemplate;
use kube::{
    api::{ListParams, WatchEvent},
    client::Client,
    Api,
};
use lru::LruCache;
use tokio::sync::broadcast::Sender;

use futures::future;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

#[derive(Clone)]
pub struct PodTemplateService {
    cache: Arc<Mutex<LruCache<(String, String), PodTemplate>>>,
    client: Client,
    changes_channel: Sender<(String, String)>,
}

impl PodTemplateService {
    pub fn new(client: Client) -> Self {
        let cache = Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(1024).unwrap())));
        let (changes_channel, _rx) = broadcast::channel(100);

        Self {
            cache,
            client,
            changes_channel,
        }
    }

    pub async fn wait_for_deployment_hook_pod_template_changes(
        &self,
        hook: &DeploymentHook,
        timeout: std::time::Duration,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // If the pod template, is in-lined, we don't need to wait for anything.
        if hook.has_embedded_pod_template() {
            return Ok(());
        }

        // Obtain the pod template (namespace, name) tuple...
        let subscription_key = (
            hook.metadata.namespace.unwrap_or_default(|| "default"),
            hook.get_pod_template_name().unwrap_or_default(|| "default"),
        );

        // Subscribe to the change, await for one, or bail out if the duration expires.
        let mut receiver = self.changes_channel.subscribe();
        future::select(
            async {
                while let Ok(pod_template_name_namespace_pair) = receiver.recv().await {
                    if pod_template_name_namespace_pair == subscription_key {
                        return ();
                    }
                }
            },
            tokio::time::sleep(timeout),
        )
        .await;

        Ok(())
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
        info!("Cache miss: calling the api");
        // Otherwise we should pull directly from the API.
        let pod_template_api: Api<PodTemplate> = Api::namespaced(self.client.clone(), namespace);
        let pod_template = pod_template_api.get(name).await?;

        // Now fill the cache so we can avoid an API call next time.
        locked.push(cache_key, pod_template.clone());

        Ok(Some(pod_template))
    }

    pub async fn push(&self, pod_template: PodTemplate) {
        let namespace = pod_template
            .metadata
            .namespace
            .clone()
            .unwrap_or_else(|| "default".to_string());

        let mut locked = self.cache.lock().await;

        if let Some(name) = pod_template.metadata.name.clone() {
            let cache_key = (namespace.to_string(), name.to_string());

            locked.push(cache_key, pod_template);
        } else {
            warn!("Could not find a name for pod_template in namepsace: {namespace}")
        }
    }

    pub async fn watch_for_changes(&self) -> Result<(), Box<dyn std::error::Error>> {
        let pod_template_api: Api<PodTemplate> = Api::all(self.client.clone());
        let changes_channel = self.changes_channel.clone();

        loop {
            let lp = ListParams::default();
            let pod_template_stream = pod_template_api.watch(&lp, "0").await?;
            tokio::pin!(pod_template_stream);

            while let Some(ref pod_template_event) = pod_template_stream.try_next().await? {
                match pod_template_event {
                    WatchEvent::Added(pod_template) | WatchEvent::Modified(pod_template) => {
                        let name = pod_template
                            .metadata
                            .name
                            .clone()
                            .unwrap_or_else(|| "unknown".to_string());
                        let namespace = pod_template
                            .metadata
                            .namespace
                            .clone()
                            .unwrap_or_else(|| "default".to_string());

                        info!(
                            "Witnessed {:?} for PodTemplate: {}/{}",
                            pod_template_event, namespace, name
                        );
                        self.push(pod_template.clone()).await;

                        if Err(err) = changes_channel.send((namespace, name).clone()).await {
                            error!("Unable to publish a change to {namespace}/{name} over internal brodcast stream}");
                        }
                    }
                    _ => { /* ignore */ }
                }
            }
        }
    }
}
