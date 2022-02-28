use crate::crd::DeploymentHook;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::batch::v1::{Job, JobSpec};
use k8s_openapi::api::core::v1::PodTemplateSpec;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

fn merge_deployment_hook_into_pod_template_spec(
    hook: &DeploymentHook,
    pod_template_spec: &PodTemplateSpec,
) -> PodTemplateSpec {
    let mut pts = pod_template_spec.clone();
    if let Some(ref mut spec) = pts.spec {
        for target_container in hook.spec.containers.iter() {
            for pod_container in spec.containers.iter_mut() {
                // Skip if this is not a target container
                if pod_container.name != target_container.name {
                    continue;
                }

                // Replace the commands of the target with the new hook commands.
                pod_container.command = Some(target_container.command.clone());
            }
        }
    }

    pts
}

pub fn generate_from_deployment(
    hook: DeploymentHook,
    deployment: Deployment,
) -> Result<Job, Box<dyn std::error::Error>> {
    let mut job = Job::default();

    // Copy metadata over to the new job.
    job.metadata = ObjectMeta::default();
    job.metadata.annotations = deployment.metadata.annotations.clone();
    if let Some(ref mut annotations) = job.metadata.annotations {
        annotations.remove("deployment.kubernetes.io/revision");
        annotations.remove("kubectl.kubernetes.io/last-applied-configuration");
    }
    job.metadata.labels = deployment.metadata.labels.clone();
    job.metadata.namespace = deployment.metadata.namespace.clone();

    // Reset the name and use generateName to ensure the job can always run.
    job.metadata.name = None;
    job.metadata.generate_name = Some(format!(
        "docbot-hook-{}-",
        &hook.metadata.name.as_ref().expect("name is missing")
    ));

    // The objective is to pluck the pod tempalte spec from a deployment, merge in changes
    // from the deployment hook, and spawn a job with the new job template spec.
    let mut job_spec = JobSpec::default();
    if let Some(deployment_spec) = deployment.spec {
        let pod_template_spec = deployment_spec.template.clone();
        job_spec.template = merge_deployment_hook_into_pod_template_spec(&hook, &pod_template_spec);
    }
    job.spec = Some(job_spec);

    Ok(job)
}

#[cfg(test)]
mod test {
    use super::*;

    fn example_deployment() -> Deployment {
        let contents = r#"
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: nginx-deployment
  namespace: docbot-test
  labels:
    app: nginx
    apps.mx.com/deploymenthook: finished
spec:
  replicas: 1
  selector:
    matchLabels:
      app: nginx
  template:
    metadata:
      labels:
        app: nginx
    spec:
      containers:
      - name: nginx
        image: nginx:1.14.2
        envFrom:
          - configMapRef:
              name: config-nginx-test
        ports:
        - containerPort: 80
"#;

        serde_yaml::from_str(contents).unwrap()
    }

    fn example_deployment_hook() -> DeploymentHook {
        let contents = r#"
---
apiVersion: apps.mx.com/v1
kind: DeploymentHook
metadata:
  name: run-app-migrations
  namespace: docbot-test
spec:
  debounceSeconds: 30
  selector:
    labels:
      apps.mx.com/deploymenthook: finished
  containers:
    - name: nginx
      command:
        - "sh"
        - "-c"
        - |
          echo "Running migrations..."
          echo "Stopping istio..."
          curl -sf -XPOST http://127.0.0.1:15020/quitquitquit
          echo "Done"
"#;

        serde_yaml::from_str(contents).unwrap()
    }

    #[test]
    fn generating_job_from_deployment_and_hook() {
        let deployment = example_deployment();
        let hook = example_deployment_hook();
        let job = generate_from_deployment(hook, deployment).unwrap();

        let expected_contents = r#"
---
apiVersion: batch/v1
kind: Job
metadata:
  generateName: docbot-hook-run-app-migrations-
  labels:
    app: nginx
    apps.mx.com/deploymenthook: finished
  namespace: docbot-test
spec:
  template:
    metadata:
      labels:
        app: nginx
    spec:
      containers:
        - command:
            - sh
            - "-c"
            - "echo \"Running migrations...\"\necho \"Stopping istio...\"\ncurl -sf -XPOST http://127.0.0.1:15020/quitquitquit\necho \"Done\"\n"
          envFrom:
            - configMapRef:
                name: config-nginx-test
          image: "nginx:1.14.2"
          name: nginx
          ports:
            - containerPort: 80
"#;
        let expected_job: Job = serde_yaml::from_str(expected_contents).unwrap();

        assert_eq!(expected_job, job);
    }
}
