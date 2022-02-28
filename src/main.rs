use crd::DeploymentHook;
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::apps::v1::{Deployment, DeploymentStatus};
use k8s_openapi::api::batch::v1::{Job, JobSpec};
use k8s_openapi::api::core::v1::PodTemplateSpec;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::{api::ListParams, client::Client, core::WatchEvent, Api};
use kube_runtime::controller::{Context, ReconcilerAction};

mod crd;
mod job;

/// Context injected with each `reconcile` and `on_error` method invocation.
struct ContextData {
    /// Kubernetes client to make Kubernetes API requests with. Required for K8S resource management.
    client: Client,
}

impl ContextData {
    /// Constructs a new instance of ContextData.
    ///
    /// # Arguments:
    /// - `client`: A Kubernetes client to make Kubernetes REST API requests with. Resources
    /// will be created and deleted with this client.
    pub fn new(client: Client) -> Self {
        ContextData { client }
    }
}

async fn watch_for_new_deployments(
    client: Client,
    hooks: Vec<DeploymentHook>,
) -> Result<(), Box<dyn std::error::Error>> {
    let deployment_api: Api<Deployment> = Api::all(client.clone());
    let params = ListParams::default().labels("apps.mx.com/deploymenthook");

    // let resource_version = deployment_api
    //     .list(&params)
    //     .await?
    //     .metadata
    //     .resource_version
    //     .expect("invalid call");

    // let mut stream = deployment_api
    //     .watch(&params, &resource_version)
    //     .await?
    //     .boxed();

    let deployments = deployment_api.list(&params).await?.items;

    for deployment in deployments.iter() {
        if let Some(ref labels) = deployment.metadata.labels {
            let matching_hooks: Vec<DeploymentHook> =
                hooks
                    .iter()
                    .filter(|hook| {
                        hook.spec.selector.labels.iter().all(|hook_label| {
                            labels.get_key_value(hook_label.0) == Some(hook_label)
                        })
                    })
                    .cloned()
                    .collect();

            for dh in matching_hooks.iter() {
                println!(
                    "MATCHED: DeployHook {:?}, Labels: {:?}",
                    dh.metadata.name, dh.spec.selector.labels,
                );

                let some_job = job::generate_from_deployment(dh.clone(), deployment.clone())?;
                println!(
                    "------- JOB:\n{}",
                    serde_yaml::to_string(&some_job).unwrap()
                );

                continue;
            }

            println!("DEPLOYMENT: {labels:?}");
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client: Client = Client::try_default()
        .await
        .expect("Expected a valid KUBECONFIG environment variable.");

    // watch_for_new_deployments(client.clone()).await;

    let dh_api: Api<DeploymentHook> = Api::all(client.clone());
    let context: Context<ContextData> = Context::new(ContextData::new(client.clone()));

    let deployment_hooks = dh_api.list(&ListParams::default()).await?.items;

    // println!("HOOKS: {:?}", &deployment_hooks);

    watch_for_new_deployments(client.clone(), deployment_hooks).await?;

    //    // TODO: Is there a way to get a list of all deployment hooks across all namespaces, begin
    //    // the reconiliation of those, and then start consuming new bits? We will need to create a
    //    // watch on all the deployments we care about.
    //
    //    // The controller comes from the `kube_runtime` crate and manages the reconciliation process.
    //    Controller::new(crd_api.clone(), ListParams::default())
    //        .run(reconcile, on_error, context)
    //        .for_each(|reconciliation_result| async move {
    //            match reconciliation_result {
    //                Ok(echo_resource) => {
    //                    println!("Reconciliation successful. Resource: {:?}", echo_resource);
    //                }
    //                Err(reconciliation_err) => {
    //                    eprintln!("Reconciliation error: {:?}", reconciliation_err)
    //                }
    //            }
    //        })
    //        .await;
    //
    Ok(())
}

// async fn reconcile(
//     deployment_hook: DeploymentHook,
//     context: Context<ContextData>,
// ) -> Result<ReconcilerAction, Error> {
//     let client: Client = context.get_ref().client.clone();
//
//     // Get the deployment hook namesace.
//     let namespace: String = match deployment_hook.namespace() {
//         None => {
//             // If there is no namespace to deploy to defined, reconciliation ends with an error immediately.
//             return Err(Error::UserInputError(
//                 "Expected DeploymentHook resource to be namespaced. Can't deploy to an unknown namespace."
//                     .to_owned(),
//             ));
//         }
//         // If namespace is known, proceed. In a more advanced version of the operator, perhaps
//         // the namespace could be checked for existence first.
//         Some(namespace) => namespace,
//     };
//
//     // TODO: Get the deployment object.
//
//     // Determine if a deployment was successful, if so, create a job patterned after the pod spec
//     // as defined according to the deployment.
//
//     // Performs action as decided by the `determine_action` function.
//     return match determine_action(client.clone(), &deployment_hook).await? {
//         Action::Create => {
//             // Creates a deployment with `n` Echo service pods, but applies a finalizer first.
//             // Finalizer is applied first, as the operator might be shut down and restarted
//             // at any time, leaving subresources in intermediate state. This prevents leaks on
//             // the `Echo` resource deletion.
//             let name = echo.name(); // Name of the Echo resource is used to name the subresources as well.
//
//             // Apply the finalizer first. If that fails, the `?` operator invokes automatic conversion
//             // of `kube::Error` to the `Error` defined in this crate.
//             finalizer::add(client.clone(), &name, &namespace).await?;
//             // Invoke creation of a Kubernetes built-in resource named deployment with `n` echo service pods.
//             echo::deploy(client, &echo.name(), echo.spec.replicas, &namespace).await?;
//             Ok(ReconcilerAction {
//                 // Finalizer is added, deployment is deployed, re-check in 10 seconds.
//                 requeue_after: Some(Duration::from_secs(10)),
//             })
//         }
//         Action::Delete => {
//             // Deletes any subresources related to this `Echo` resources. If and only if all subresources
//             // are deleted, the finalizer is removed and Kubernetes is free to remove the `Echo` resource.
//
//             //First, delete the deployment. If there is any error deleting the deployment, it is
//             // automatically converted into `Error` defined in this crate and the reconciliation is ended
//             // with that error.
//             // Note: A more advanced implementation would for the Deployment's existence.
//             echo::delete(client.clone(), &echo.name(), &namespace).await?;
//
//             // Once the deployment is successfully removed, remove the finalizer to make it possible
//             // for Kubernetes to delete the `Echo` resource.
//             finalizer::delete(client, &echo.name(), &namespace).await?;
//             Ok(ReconcilerAction {
//                 requeue_after: None, // Makes no sense to delete after a successful delete, as the resource is gone
//             })
//         }
//         Action::NoOp => Ok(ReconcilerAction {
//             // The resource is already in desired state, do nothing and re-check after 10 seconds
//             requeue_after: Some(Duration::from_secs(10)),
//         }),
//     };
// }
//
// fn on_error(error: &Error, _context: Context<ContextData>) -> ReconcilerAction {
//     eprintln!("Reconciliation error:\n{:?}", error);
//     ReconcilerAction {
//         requeue_after: Some(Duration::from_secs(5)),
//     }
// }
