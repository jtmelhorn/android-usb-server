// SPDX-License-Identifier: GPL-3.0-or-later
//! Client admission: USB/IP has no authentication, so only local-network peers are accepted.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// True for loopback, RFC 1918, IPv4 link-local, IPv6 ULA (fc00::/7), IPv6 link-local
/// (fe80::/10) and IPv4-mapped forms of the IPv4 ranges. Carrier-grade NAT (100.64/10) and all
/// public addresses are rejected.
pub fn is_local_network(addr: IpAddr) -> bool {
    match addr {
        IpAddr::V4(v4) => is_local_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_local_v4(v4);
            }
            let seg0 = v6.segments()[0];
            v6 == Ipv6Addr::LOCALHOST || (seg0 & 0xfe00) == 0xfc00 || (seg0 & 0xffc0) == 0xfe80
        }
    }
}

fn is_local_v4(v4: Ipv4Addr) -> bool {
    v4.is_loopback() || v4.is_private() || v4.is_link_local()
}

/// Normalise IPv4-mapped IPv6 addresses so they compare equal to the plain IPv4 form.
pub fn canonical(addr: IpAddr) -> IpAddr {
    match addr {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(addr),
        v4 => v4,
    }
}

#[derive(Debug, Clone, Default)]
pub struct AccessPolicy {
    /// If set, only this client may connect (still subject to the local-network rule).
    pub allowed_client: Option<IpAddr>,
}

impl AccessPolicy {
    pub fn admits(&self, peer: IpAddr) -> bool {
        let peer = canonical(peer);
        if !is_local_network(peer) {
            return false;
        }
        match self.allowed_client {
            Some(a) => canonical(a) == peer,
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn local_ranges() {
        for a in [
            "10.0.0.1", "10.255.255.255", "172.16.0.1", "172.31.255.254", "192.168.15.133", "169.254.1.1",
            "127.0.0.1", "::1", "fd00::1", "fc12::1", "fe80::1", "::ffff:192.168.1.5", "::ffff:10.1.2.3",
        ] {
            assert!(is_local_network(ip(a)), "{a} should be local");
        }
    }

    #[test]
    fn public_and_edge_ranges_rejected() {
        for a in [
            "8.8.8.8", "172.15.255.255", "172.32.0.0", "192.169.0.1", "100.64.0.1", "11.0.0.1", "0.0.0.0",
            "255.255.255.255", "2001:4860:4860::8888", "fec0::1", "::ffff:8.8.8.8", "::", "ff02::1",
        ] {
            assert!(!is_local_network(ip(a)), "{a} should be rejected");
        }
    }

    #[test]
    fn allowed_client_policy() {
        let open = AccessPolicy::default();
        assert!(open.admits(ip("192.168.1.10")));
        assert!(!open.admits(ip("1.2.3.4")));

        let only = AccessPolicy { allowed_client: Some(ip("192.168.15.133")) };
        assert!(only.admits(ip("192.168.15.133")));
        assert!(only.admits(ip("::ffff:192.168.15.133")));
        assert!(!only.admits(ip("192.168.15.134")));

        // A public address configured as "allowed" is still refused.
        let public = AccessPolicy { allowed_client: Some(ip("8.8.8.8")) };
        assert!(!public.admits(ip("8.8.8.8")));
    }
}
