use crate::dns::get_resolver;
use crate::k8s::{get_vip_services, recreate_service};
use crate::settings::Settings;
use figment::providers::Env;
use hickory_resolver::TokioAsyncResolver;
use hickory_resolver::error::ResolveErrorKind;
use k8s_openapi::api::core::v1::Service;
use kube::ResourceExt;

mod dns;
mod k8s;
mod settings;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    init_tracing_subscriber().unwrap();
    let settings = get_env_settings().unwrap();
    let k8s = kube::Client::try_default().await.unwrap();
    let resolver = get_resolver(&settings).unwrap();
    recreate_disconnected_vip_services(settings, k8s, resolver).await
}

async fn recreate_disconnected_vip_services(
    settings: Settings,
    k8s: kube::Client,
    resolver: TokioAsyncResolver,
) {
    let api = kube::Api::all(k8s);
    let vip_services = get_vip_services(&api, &settings.vip_annotation)
        .await
        .unwrap();
    for service in vip_services {
        check_and_repair(service, &settings, &api, &resolver).await
    }
}

async fn check_and_repair(
    service: Service,
    settings: &Settings,
    api: &kube::Api<Service>,
    resolver: &TokioAsyncResolver,
) {
    let host = service
        .metadata
        .annotations
        .as_ref()
        .and_then(|a| a.get(&settings.vip_annotation))
        .unwrap();
    match resolver.ipv4_lookup(host).await {
        Ok(lookup) => {
            let ip = lookup
                .iter()
                .map(|a| format!("{:?}", a.0))
                .collect::<Vec<_>>()
                .join(",");
            tracing::info!(
                namespace = service.namespace(),
                name = service.name_any(),
                hostname = host,
                ipv4_address = ip,
                "DHCP is healthy"
            )
        }
        Err(e) => {
            if matches!(e.kind(), ResolveErrorKind::NoRecordsFound { .. }) {
                tracing::warn!(
                    namespace = service.namespace(),
                    name = service.name_any(),
                    hostname = host,
                    "Unresolved host, will try to fix."
                );
                let _service = recreate_service(&api, service).await.unwrap();
                // and then we poll DNS until the service reappears...
            } else {
                panic!("Unhandled error: {:?}", e)
            }
        }
    }
}

fn init_tracing_subscriber() -> Result<(), tracing::dispatcher::SetGlobalDefaultError> {
    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish(),
    )
}

fn get_env_settings() -> figment::Result<Settings> {
    figment::Figment::new().merge(Env::raw()).extract()
}
