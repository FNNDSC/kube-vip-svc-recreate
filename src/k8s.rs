use k8s_openapi::api::core::v1::Service;
use kube::api::ListParams;
use kube::{Api, ResourceExt, Result};

pub async fn get_vip_services(
    api: &Api<Service>,
    annotation: &str,
) -> Result<impl Iterator<Item = Service>> {
    let list = api.list(&ListParams::default()).await?;
    let vip_services = list
        .into_iter()
        .filter(|svc| svc.annotations().contains_key(annotation));
    Ok(vip_services)
}

pub async fn recreate_service(api: &Api<Service>, service: Service) -> Result<Service> {
    tracing::info!("Fake recreating svc/{}", service.name_any());
    Ok(service)
}
