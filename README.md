DocBot
======

![docbot](docbot.jpg)

Welcome to the real world.

## Overview

Responsible for generating jobs when a deployment is updated given some `PodTemplate`.

## Building

```
cargo build
```

or you can run locally by setting `export KUBECOFIG=~/.kube/teleport` followed by

```
cargo run
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


