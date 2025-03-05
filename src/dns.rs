use crate::settings::Settings;
use hickory_resolver::config::{NameServerConfigGroup, ResolverConfig, ResolverOpts};
use hickory_resolver::error::ResolveError;
use hickory_resolver::{AsyncResolver, Name, TokioAsyncResolver};

pub fn get_resolver(settings: &Settings) -> Result<TokioAsyncResolver, ResolveError> {
    if let Some(dns_server) = &settings.dns_server {
        let domain = settings
            .dns_domain
            .clone()
            .map(|d| Name::from_utf8(d).unwrap());
        let ips = [dns_server.ip()];
        let nameservers = NameServerConfigGroup::from_ips_clear(&ips, dns_server.port(), true);
        let config = ResolverConfig::from_parts(domain, vec![], nameservers);
        let resolver = AsyncResolver::tokio(config, to_opts(settings));
        Ok(resolver)
    } else {
        if settings.dns_timeout.is_some() {
            tracing::warn!("Ignoring DNS_TIMEOUT because DNS_SERVER is unset");
        }
        if settings.dns_domain.is_some() {
            tracing::warn!("Ignoring DNS_DOMAIN because DNS_SERVER is unset");
        }
        AsyncResolver::tokio_from_system_conf()
    }
}

fn to_opts(settings: &Settings) -> ResolverOpts {
    let mut opts = ResolverOpts::default();
    if let Some(timeout) = settings.dns_timeout {
        opts.timeout = timeout;
    }
    opts
}
