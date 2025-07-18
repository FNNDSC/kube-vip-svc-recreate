# kube-vip DHCP LoadBalancer Recreation Workaround

We have experienced a spontaneous failure mode with [kube-vip](https://kube-vip.io) version 0.8.9 where the hostname of a `LoadBalancer`-type service using kube-vip's DHCP feature becomes unresolvable. This can be worked around by forcing kube-vip to renew the DHCP lease. The easiest way to do this is by deleting then recreating the service.

This repository implements a simple workaround which deletes then recreates `LoadBalancer`-type services managed by kube-vip as-needed. As pseudocode:

```python
for service in kubernetes.list_services(all_namespaces=True):
    if (
        serivce.type == 'LoadBalancer')
        && 'kube-vip.io/loadbalancerHostname' in serivce.annotations
        && dns_resolve(service.annotations['kube-vip.io/loadbalancerHostname']) is None
    ):
        k8s.delete(service)
        k8s.create(service)
```

Use this as a [CronJob](https://kubernetes.io/docs/concepts/workloads/controllers/cron-jobs/):

```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: kube-vip-svc-recreate
  namespace: kube-system
  annotations:
    kubernetes.io/description: Automatically recreate flakey kube-vip services
spec:
  schedule: '*/10 * * * *'
  successfulJobsHistoryLimit: 6
  failedJobsHistoryLimit: 10
  concurrencyPolicy: Forbid
  jobTemplate:
    spec:
      template:
        spec:
          containers:
          - name: kube-vip-svc-recreate
            image: ghcr.io/fnndsc/kube-vip-svc-recreate:e9c0cd2    # supports Kubernetes v1.30-1.33
            # image: ghcr.io/fnndsc/kube-vip-svc-recreate:2e056ad  # supports Kubernetes v1.28-1.32
            resources:
              requests:
                cpu: 50m
                memory: 64MiB
              limits:
                cpu: 50m
                memory: 128MiB
```
