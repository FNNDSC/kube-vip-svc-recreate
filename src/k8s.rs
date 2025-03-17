use either::Either;
use k8s_openapi::api::core::v1::Service;
use kube::api::{DeleteParams, ListParams, PostParams};
use kube::runtime::conditions::is_deleted;
use kube::runtime::reflector::Lookup;
use kube::runtime::wait::await_condition;
use kube::{Api, ResourceExt};

use crate::constants::ANNOTATIONS_TO_REMOVE;

pub async fn get_vip_services(
    api: &Api<Service>,
    annotation: &str,
) -> kube::Result<impl Iterator<Item = Service>> {
    let list = api.list(&ListParams::default()).await?;
    let vip_services = list
        .into_iter()
        .filter(|svc| svc.annotations().contains_key(annotation));
    Ok(vip_services)
}

pub async fn recreate_service(client: kube::Client, service: Service) -> kube::Result<Service> {
    let name = service
        .name()
        .expect("Kubernetes service does not have a name")
        .into_owned();
    let namespace = Lookup::namespace(&service)
        .expect("Kubernetes service does not have a namespace")
        .into_owned();
    let api: Api<Service> = Api::namespaced(client, &namespace);
    delete(&api, &name, &namespace).await?;
    match api.create(&PostParams::default(), &reset(service)).await {
        Ok(svc) => {
            tracing::info!(namespace = namespace, service = name, "recreated");
            Ok(svc)
        }
        Err(e) => {
            tracing::error!(
                namespace = namespace,
                service = name,
                error = format!("{:?}", &e),
                "Failed to create service.
                Action required! Service is permanently deleted and must
                be recreated manually."
            );
            Err(e)
        }
    }
}

/// Prepare a previously existing kube-vip LoadBalancer service for replacement.
fn reset(mut service: Service) -> Service {
    service.metadata.uid = None;
    service.metadata.creation_timestamp = None;
    service.metadata.resource_version = None;
    service.status = None;
    // N.B. metadata.resourceVersion is kept because it is required for replace operation.
    // https://docs.rs/kube/0.98.0/kube/api/struct.Api.html#method.replace
    if let Some(annotations) = service.metadata.annotations.as_mut() {
        for key in ANNOTATIONS_TO_REMOVE {
            annotations.remove(key);
        }
    }
    if let Some(spec) = service.spec.as_mut() {
        spec.cluster_ip = None;
        spec.cluster_ips = None;
        spec.external_ips = None;
        spec.external_name = None;
        spec.ip_families = None;
        if let Some(ports) = spec.ports.as_mut() {
            for port in ports {
                // NOTE: NodePort remains allocated for a short duration
                // after its service is deleted, and trying to recreate
                // the service without changing NodePort causes an error.
                // Clearing the value of NodePort shouldn't cause harm...
                port.node_port = None;
            }
        }
    }
    service
}

async fn delete(api: &Api<Service>, name: &str, namespace: &str) -> kube::Result<()> {
    match api.delete(name, &DeleteParams::foreground()).await? {
        Either::Left(svc) => {
            let uid = Lookup::uid(&svc).unwrap();
            let api = api.clone();
            tracing::trace!(
                service = name,
                namespace = namespace,
                uid = uid.as_ref(),
                "Pending deletion"
            );
            // SMELL: irresponsible .unwrap()
            await_condition(api, name, is_deleted(&uid)).await.unwrap();
        }
        Either::Right(status) => {
            if status.is_failure() {
                tracing::error!(
                    namespace = namespace,
                    service = name,
                    reason = status.reason,
                    "Failed to delete service"
                );
                panic!("Failed to delete svc/{name} in namepsace {namespace}");
            }
        }
    };
    tracing::info!(namespace = namespace, service = name, "Service deleted");
    Ok(())
}
