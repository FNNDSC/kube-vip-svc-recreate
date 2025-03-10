use crate::settings::Settings;
use hickory_resolver::config::{NameServerConfigGroup, ResolverConfig, ResolverOpts};
use hickory_resolver::error::{ResolveError, ResolveErrorKind};
use hickory_resolver::lookup::Ipv4Lookup;
use hickory_resolver::{AsyncResolver, Name, TokioAsyncResolver};
use std::time::Duration;
use tokio::time::Instant;

pub(crate) fn bubble_ips(
    result: Result<Ipv4Lookup, ResolveError>,
) -> Result<Option<String>, ResolveError> {
    match result {
        Ok(lookup) => {
            let ip = lookup
                .iter()
                .map(|a| format!("{:?}", a.0))
                .collect::<Vec<_>>()
                .join(",");
            Ok(Some(ip))
        }
        Err(e) => {
            if matches!(e.kind(), ResolveErrorKind::NoRecordsFound { .. }) {
                Ok(None)
            } else {
                Err(e)
            }
        }
    }
}

pub(crate) async fn poll_until_exists(
    resolver: &TokioAsyncResolver,
    host: &str,
    timeout: Duration,
    interval: Duration,
) -> Result<Option<String>, ResolveError> {
    poll_for(timeout, interval, async || {
        bubble_ips(resolver.ipv4_lookup(host).await)
    })
    .await
}

async fn poll_for<T, E, F: AsyncFn() -> Result<Option<T>, E>>(
    timeout: Duration,
    interval: Duration,
    f: F,
) -> Result<Option<T>, E> {
    let start = Instant::now();
    loop {
        let elapsed = Instant::now() - start;
        if elapsed >= timeout {
            return Ok(None);
        }
        tokio::time::sleep(interval).await;
        match f().await {
            Ok(o) => {
                if o.is_some() {
                    return Ok(o);
                }
            }
            Err(e) => {
                return Err(e);
            }
        }
    }
}

pub(crate) fn get_resolver(settings: &Settings) -> Result<TokioAsyncResolver, ResolveError> {
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
