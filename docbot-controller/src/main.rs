use crate::cache::{CacheOp, DeploymentHookCache, DeploymentPodTemplateHashCache};
use docbot_crd::DeploymentHook;
use tracing::{debug, info, Level};

use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::batch::v1::Job;
use k8s_openapi::api::core::v1::PodTemplate;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::{
    api::{ListParams, PostParams},
    client::Client,
    core::WatchEvent,
    Api,
};
use utils::DeploymentExt;

mod cache;
mod job;
mod utils;

// Helper to print namspace/name in a nice way since we do that a lot.
trait ResourceFormatter {
    fn formatted_name(&self) -> String;
}

impl ResourceFormatter for ObjectMeta {
    fn formatted_name(&self) -> String {
        let name = self
            .name
            .clone()
            .unwrap_or_else(|| "__unknown__".to_string());
        let namespace = self
            .namespace
            .clone()
            .unwrap_or_else(|| "default".to_string());

        format!("{namespace}/{name}")
    }
}

fn display_container(podtemplate: &PodTemplate, event: &str) {
    let namespace = &podtemplate
        .metadata
        .namespace
        .clone()
        .expect("all object will have namespace");

    if let Some(template) = &podtemplate.template {
        if let Some(pod_spec) = &template.spec {
            for container in &pod_spec.containers {
                info!("Debug watcher: For event {} - Container Image for template in namespace {:?} from k8s api {} : {:?}", event, namespace, container.name, container.image);
            }
        }
    } else {
        info!(
            "Debug watcher: No PodTemplate spec found for '{}'",
            namespace
        );
    }
}

async fn create_job_for_deployment_hook(
    client: Client,
    hook: &DeploymentHook,
) -> Result<(), Box<dyn std::error::Error>> {
    let generated_job =
        job::generate_from_template(hook, hook.get_pod_template(client.clone()).await?)?;

    let job_api: Api<Job> = Api::namespaced(
        client.clone(),
        &generated_job.metadata.namespace.as_ref().unwrap(),
    );

    job_api
        .create(&PostParams::default(), &generated_job)
        .await?;

    Ok(())
}

async fn watch_for_podtemplate(client: Client) -> Result<(), Box<dyn std::error::Error>> {
    let pod_template_api: Api<PodTemplate> = Api::all(client);

    let lp = ListParams::default();
    let pod_template_stream = pod_template_api.watch(&lp, "0").await?;
    // await on try_next suggested to use a pin
    tokio::pin!(pod_template_stream);
    // Process watch events
    while let Some(pod_template_event) = pod_template_stream.try_next().await? {
        match pod_template_event {
            WatchEvent::Added(pod_template) => {
                display_container(&pod_template, "ADDED");
            }
            WatchEvent::Modified(pod_template) => {
                display_container(&pod_template, "MODIFIED");
            }
            WatchEvent::Error(error) => {
                debug!("Error: {:?}", error);
            }
            _ => {}
        }
    }
    Ok(())
}

async fn watch_for_new_deployments(
    client: Client,
    cache: DeploymentHookCache,
    template_cache: DeploymentPodTemplateHashCache,
) -> Result<(), Box<dyn std::error::Error>> {
    let deployment_api: Api<Deployment> = Api::all(client.clone());
    let params = ListParams::default().labels("apps.mx.com/deploymenthook");

    let resource_version = deployment_api
        .list(&params)
        .await?
        .metadata
        .resource_version
        .expect("invalid call");

    info!(
        "Current Deployment API ResourceVersion: {}, Subscribing...",
        &resource_version
    );

    let mut stream = deployment_api
        .watch(&params, &resource_version)
        .await?
        .boxed();

    while let Some(event) = stream.try_next().await? {
        match event {
            WatchEvent::Added(deployment) | WatchEvent::Modified(deployment) => {
                // If the deployment hasn't finished, we should skip.
                if !deployment.did_successfully_deploy() {
                    continue;
                }

                // With a successfully deployed deployment, check to see if we've seen
                // this pod template before. If we have, then it is likely a pod of an
                // existing deployment was restarted, or scaled up or down.
                if let CacheOp::Unchanged = template_cache.update_cache(&deployment) {
                    info!(
                        "Skipping deployment {} because pod template was not modified",
                        deployment.metadata.formatted_name()
                    );
                    continue;
                }

                for hook in cache.find_by_matching_deployment(&deployment).iter() {
                    info!(
                        "Creating a job for hook {} generated by deployment {}",
                        hook.metadata.formatted_name(),
                        deployment.metadata.formatted_name()
                    );

                    create_job_for_deployment_hook(client.clone(), hook).await?;
                }
            }
            _ => { /* ignore */ }
        }
    }

    Ok(())
}

async fn watch_for_deployment_hook_changes(
    client: Client,
    cache: DeploymentHookCache,
) -> Result<(), Box<dyn std::error::Error>> {
    let hooks_api: Api<DeploymentHook> = Api::all(client.clone());
    let params = ListParams::default();

    let resource_version = hooks_api
        .list(&params)
        .await?
        .metadata
        .resource_version
        .expect("invalid call");

    info!(
        "Current Deployment Hook API ResrouceVersion: {}, Subscribing...",
        &resource_version
    );

    let mut stream = hooks_api.watch(&params, &resource_version).await?.boxed();

    while let Some(_event) = stream.try_next().await? {
        info!("Refreshing deployment hook cache.");
        cache.refresh(&client).await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // construct a subscriber that prints formatted traces to stdout
    let subscriber = tracing_subscriber::fmt()
        // Use a more compact, abbreviated log format
        .pretty()
        // Display the thread ID an event was recorded on
        .with_thread_ids(true)
        // Don't display the event's target (module path)
        .with_target(false)
        .with_max_level(Level::DEBUG)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        // Build the subscriber
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;
    debug!("Setting up k8s config");

    let client: Client = Client::try_default()
        .await
        .expect("Expected a valid KUBECONFIG environment variable.");

    // Prime the deployhook cache
    let cache = cache::DeploymentHookCache::default();
    cache.refresh(&client).await?;

    // Prime the deployment cache
    let template_cache = cache::DeploymentPodTemplateHashCache::default();
    template_cache.refresh(&client).await?;

    // Refresh the cache every minute
    tokio::spawn({
        let cache = cache.clone();
        let client = client.clone();

        async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;

                info!("Refreshing deployment hook cache.");
                if let Err(err) = cache.refresh(&client).await {
                    info!("Failed to refresh the deployment hooks cache: {:?}", err);
                }
            }
        }
    });

    tokio::spawn({
        let cache = cache.clone();
        let client = client.clone();

        async move {
            // Watch for deployment hook changes
            loop {
                if let Err(err) =
                    watch_for_deployment_hook_changes(client.clone(), cache.clone()).await
                {
                    info!("Error while watching deployment hook changes: {err:?}");
                }

                info!("DeploymentHook watcher finished or expired, restarting...");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    });

    tokio::spawn({
        let cache = cache.clone();
        let client = client.clone();

        async move {
            // Watch for deployment changes
            loop {
                if let Err(err) =
                    watch_for_new_deployments(client.clone(), cache.clone(), template_cache.clone())
                        .await
                {
                    info!("Error while watching deployment hook changes: {err:?}");
                }

                info!("Deployment watcher finished or expired, restarting...");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    });

    tokio::spawn({
        let client = client.clone();

        async move {
            // Watch for deployment changes
            loop {
                if let Err(err) = watch_for_podtemplate(client.clone()).await {
                    info!("Error while watching deployment hook changes: {err:?}");
                }

                info!("Deployment watcher finished or expired, restarting...");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    });

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await
    }
}
