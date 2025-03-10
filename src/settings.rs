use serde::Deserialize;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

#[derive(Deserialize)]
pub struct Settings {
    /// DNS server
    pub dns_server: Option<DnsAddr>,

    /// Timeout for DNS lookups
    #[serde(with = "humantime_serde", default)]
    pub dns_timeout: Option<Duration>,

    /// Network domain name for DNS lookups
    #[serde(default)]
    pub dns_domain: Option<String>,

    /// Annotation of kube-vip DHCP LoadBalancer-type services
    #[serde(default = "loadbalancer_hostname")]
    pub vip_annotation: String,

    /// How long to check for the startup of recreated services
    #[serde(with = "humantime_serde", default = "default_check_timeout")]
    pub check_timeout: Duration,

    /// Interval between DNS poll for startup of recreated services
    #[serde(with = "humantime_serde", default = "default_check_interval")]
    pub check_interval: Duration,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum DnsAddr {
    Ip(IpAddr),
    Socket(SocketAddr),
}

fn default_check_interval() -> Duration {
    Duration::from_secs(5)
}

fn default_check_timeout() -> Duration {
    Duration::from_secs(120)
}

fn loadbalancer_hostname() -> String {
    "kube-vip.io/loadbalancerHostname".to_string()
}

impl DnsAddr {
    pub fn ip(&self) -> IpAddr {
        match &self {
            Self::Ip(x) => *x,
            Self::Socket(x) => x.ip(),
        }
    }

    pub fn port(&self) -> u16 {
        match &self {
            DnsAddr::Ip(_) => 53,
            DnsAddr::Socket(x) => x.port(),
        }
    }
}
