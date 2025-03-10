use crate::dns::get_resolver;
use crate::k8s::{get_vip_services, recreate_service};
use crate::settings::Settings;
use dns::{bubble_ips, poll_until_exists};
use figment::providers::Env;
use hickory_resolver::TokioAsyncResolver;
use k8s_openapi::api::core::v1::Service;
use kube::ResourceExt;

mod constants;
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
        .unwrap()
        .to_string();
    let name = service.name_any();
    if let Some(ips) = bubble_ips(resolver.ipv4_lookup(&host).await).unwrap() {
        tracing::info!(
            namespace = service.namespace(),
            service = &name,
            hostname = host,
            ipv4_address = ips,
            "DHCP is healthy"
        );
    } else {
        tracing::warn!(
            namespace = service.namespace(),
            service = service.name_any(),
            hostname = host,
            "Unresolved host, will try to fix."
        );
        let client = api.clone().into_client();
        let service = recreate_service(client, service).await.unwrap();
        if let Some(ips) = poll_until_exists(
            resolver,
            &host,
            settings.check_timeout,
            settings.check_interval,
        )
        .await
        .unwrap()
        {
            tracing::info!(
                namespace = service.namespace(),
                service = &name,
                hostname = host,
                ipv4_address = ips,
                "Successfully restored."
            );
        } else {
            tracing::error!(
                namespace = service.namespace(),
                service = &name,
                hostname = host,
                "Hostname still unresolved after service was recreated."
            );
            panic!("Failed to restore functionality for svc/{name}");
        }
    }
}

fn init_tracing_subscriber() -> Result<(), tracing::dispatcher::SetGlobalDefaultError> {
    let filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive(tracing_subscriber::filter::LevelFilter::INFO.into());
    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_env_filter(filter)
            .finish(),
    )
}

fn get_env_settings() -> figment::Result<Settings> {
    figment::Figment::new().merge(Env::raw()).extract()
}
