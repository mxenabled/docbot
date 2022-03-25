DocBot
======

![docbot](docbot.jpg)

Welcome to the real world.

## Overview

Responsible for generating jobs when a deployment is updated given some `PodTemplate`.

## Building

```
cd pod-controller && cargo build
```

or you can run locally by setting `export KUBECOFIG=~/.kube/teleport` followed by

```
cd pod-controller && cargo run
```

## Custom Resources

## DeploymentHook

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
