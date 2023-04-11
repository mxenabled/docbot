DocBot
======

![docbot](docbot.jpg)

Welcome to the real world.

## Overview

At MX we needed to run a job after a deployment successfully rolled out.
The primary use case was running migrations after a deploy.
We tried using ArgoCD hooks to run those wouldn't wait for a deployment, and we had to delete the job before we could re-run after a deploy, meaning if it was a long running migration, like creating an index, it would be terminated half way through execution.
Not good.
We looked at other operators but couldn't find anything simple that we could run to solve this problem, so we created docbot.

## Usage

Docbot watches deployments that have the following label: `apps.mx.com/deploymenthook: finished`.
When a deployment with this label finishes deploying, it will check to see if any `DeploymentHook`s in the namespace have a selector matching the labels of the deployment.
When a `DeploymentHook` matches, it will create a job using the inlined `PodSpec` or use the `PodTemplate` referenced by `.spec.template.name`.
These jobs will run until completion and not be reaped by docbot, so it's encouraged to set a TTL on these jobs.

## Building

```
cd pod-controller && cargo build
```

or you can run locally by setting `export KUBECOFIG=~/.kube/teleport` followed by

```
cd pod-controller && cargo run
```

## Custom Resources

### DeploymentHook

When an deployment matching the selector labels is updated (ex: new revision), a new job will be created using the pod template defined in the hook's `spec/template/name`.

```yaml
apiVersion: apps.mx.com/v1
kind: DeploymentHook
metadata:
  name: run-app-migrations
  namespace: docbot-test
spec:
  selector:
    labels:
      app: nginx
  template:
    name: nginx-pod-template
```

You can also inline a pod template on the CRD:

```yaml
apiVersion: apps.mx.com/v1
kind: DeploymentHook
metadata:
  name: run-app-migrations
  namespace: docbot-test
spec:
  selector:
    labels:
      app: nginx
  template:
    spec:
      metadata:
        labels:
          app: nginx
      spec:
        containers:
        - name: nginx
          image: nginx:1.14.2
          command:
            - "sh"
            - "-c"
            - |
              echo "Doing some work..."
              echo "Still working on it..."
              echo "Done!"
          envFrom:
            - configMapRef:
                name: config-nginx-test
          ports:
          - containerPort: 80
```

## License

MIT (See the LICENSE file included with this project)
