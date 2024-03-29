use docbot_crd::DeploymentHook;
use k8s_openapi::api::batch::v1::{Job, JobSpec};
use k8s_openapi::api::core::v1::PodTemplate;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::Resource;

pub fn generate_from_template(
    hook: &DeploymentHook,
    template: PodTemplate,
) -> Result<Job, Box<dyn std::error::Error>> {
    let mut job = Job::default();
    job.metadata = ObjectMeta::default();
    if let Some(ref mut annotations) = job.metadata.annotations {
        annotations.remove("kubectl.kubernetes.io/last-applied-configuration");
    }
    job.metadata.labels = template.metadata.labels.clone();
    job.metadata.namespace = template.metadata.namespace.clone();
    job.metadata.generate_name = Some(format!(
        "docbot-hook-{}-",
        &hook.metadata.name.as_ref().expect("name is missing")
    ));

    // Set owner reference so job is a child of the hook resource that spawned it
    if let Some(owner_ref) = hook.controller_owner_ref(&()) {
        job.metadata.owner_references = Some(vec![owner_ref]);
    }

    let mut job_spec = JobSpec::default();

    // Set the job ttl after it finishes.
    job_spec.ttl_seconds_after_finished = hook.spec.template.ttl_seconds_after_finished;

    if let Some(pod_template_spec) = template.template {
        if let Some(ref metadata) = pod_template_spec.metadata {
            job.metadata.annotations = metadata.annotations.clone();
        }

        job_spec.template = pod_template_spec.clone();
        if let Some(ref mut spec) = job_spec.template.spec {
            // Reset this value of Always was specified. This is the default value for
            // PodTemplates used by Pods, but it is invalid for Jobs.
            if spec.restart_policy == Some("Always".to_string()) {
                spec.restart_policy = None;
            }

            spec.restart_policy = Some(
                spec.restart_policy
                    .clone()
                    .unwrap_or_else(|| "Never".to_string()),
            )
        }
        // On Error reduce the back-off limit to 1 to stop k8s from re-trying the errored out jobs
        job_spec.backoff_limit = Some(1)
    }
    job.spec = Some(job_spec);
    Ok(job)
}

#[cfg(test)]
mod test {
    use super::*;

    fn example_pod_template() -> PodTemplate {
        let contents = r#"
---
apiVersion: v1
kind: PodTemplate
metadata:
  name: nginx-template
  namespace: docbot-test
  labels:
    app: nginx
template:
  metadata:
    labels:
      app: nginx
  spec:
    containers:
    - name: nginx
      image: nginx:1.14.2
      command:
        - sh
        - "-c"
        - "echo \"Running migrations...\"\necho \"Stopping istio...\"\ncurl -sf -XPOST http://127.0.0.1:15020/quitquitquit\necho \"Done\"\n"
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
  uid: 1234
spec:
  selector:
    labels:
      apps.mx.com/deploymenthook: finished
  template:
    name: nginx-template
"#;

        serde_yaml::from_str(contents).unwrap()
    }

    #[test]
    fn generating_job_from_deployment_and_hook() {
        let template = example_pod_template();
        let hook = example_deployment_hook();
        let job = generate_from_template(&hook, template).unwrap();

        let expected_contents = r#"
---
apiVersion: batch/v1
kind: Job
metadata:
  generateName: docbot-hook-run-app-migrations-
  labels:
    app: nginx
  namespace: docbot-test
  ownerReferences:
  - apiVersion: "apps.mx.com/v1"
    controller: true
    kind: "DeploymentHook"
    name: "run-app-migrations"
    uid: "1234"
spec:
  ttlSecondsAfterFinished: 259200
  backoffLimit: 1
  template:
    metadata:
      labels:
        app: nginx
    spec:
      restartPolicy: Never
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
