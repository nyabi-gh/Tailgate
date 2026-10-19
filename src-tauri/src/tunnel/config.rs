use std::net::Ipv4Addr;

use base64::{Engine, engine::general_purpose::STANDARD};

/// Everything needed to bring up one WireGuard peer inside the app.
#[derive(Clone)]
pub struct TunnelConfig {
    pub private_key: [u8; 32],
    pub address: Ipv4Addr,
    pub dns: Ipv4Addr,
    pub peer_public_key: [u8; 32],
    pub preshared_key: Option<[u8; 32]>,
    /// `host:port`; the host may be a name, resolved when the tunnel starts.
    pub endpoint: String,
    pub keepalive: Option<u16>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ConfigError {
    #[error("missing `{0}`")]
    Missing(&'static str),
    #[error("invalid `{key}`: {value}")]
    Invalid { key: &'static str, value: String },
}

impl TunnelConfig {
    /// Parses a wg-quick style `.conf`. Only the first `[Peer]` is used, and only IPv4 addresses,
    /// because the in-app stack is IPv4-only. `AllowedIPs` is ignored: all app traffic goes through.
    pub fn parse_wg_quick(text: &str) -> Result<Self, ConfigError> {
        let mut section = "";
        let mut private_key = None;
        let mut address = None;
        let mut dns = None;
        let mut peer_public_key = None;
        let mut preshared_key = None;
        let mut endpoint = None;
        let mut keepalive = None;
        let mut seen_peer = false;

        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('[') {
                section = if line.eq_ignore_ascii_case("[interface]") {
                    "interface"
                } else if line.eq_ignore_ascii_case("[peer]") {
                    if seen_peer { "ignored" } else { "peer" }
                } else {
                    "ignored"
                };
                seen_peer |= section == "peer";
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim().to_ascii_lowercase(), value.trim());

            match (section, key.as_str()) {
                ("interface", "privatekey") => private_key = Some(parse_key("PrivateKey", value)?),
                ("interface", "address") => address = Some(first_ipv4("Address", value)?),
                ("interface", "dns") => dns = Some(first_ipv4("DNS", value)?),
                ("peer", "publickey") => peer_public_key = Some(parse_key("PublicKey", value)?),
                ("peer", "presharedkey") => preshared_key = Some(parse_key("PresharedKey", value)?),
                ("peer", "endpoint") => endpoint = Some(value.to_string()),
                ("peer", "persistentkeepalive") => {
                    keepalive = match value.parse::<u16>() {
                        Ok(0) => None,
                        Ok(seconds) => Some(seconds),
                        Err(_) => return Err(invalid("PersistentKeepalive", value)),
                    }
                }
                _ => {}
            }
        }

        Ok(Self {
            private_key: private_key.ok_or(ConfigError::Missing("PrivateKey"))?,
            address: address.ok_or(ConfigError::Missing("Address"))?,
            dns: dns.ok_or(ConfigError::Missing("DNS"))?,
            peer_public_key: peer_public_key.ok_or(ConfigError::Missing("PublicKey"))?,
            preshared_key,
            endpoint: endpoint.ok_or(ConfigError::Missing("Endpoint"))?,
            keepalive,
        })
    }
}

pub fn parse_key(key: &'static str, value: &str) -> Result<[u8; 32], ConfigError> {
    STANDARD
        .decode(value)
        .ok()
        .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
        .ok_or_else(|| invalid(key, "not a base64 32-byte key"))
}

fn first_ipv4(key: &'static str, value: &str) -> Result<Ipv4Addr, ConfigError> {
    value
        .split(',')
        .map(|item| item.trim().split('/').next().unwrap_or(""))
        .find_map(|item| item.parse::<Ipv4Addr>().ok())
        .ok_or_else(|| invalid(key, value))
}

fn invalid(key: &'static str, value: &str) -> ConfigError {
    ConfigError::Invalid {
        key,
        value: value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_A: &str = "yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=";
    const KEY_B: &str = "xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=";

    fn sample() -> String {
        format!(
            "[Interface]\n\
             # device: tailgate\n\
             PrivateKey = {KEY_A}\n\
             Address = 10.66.1.2/32, fc00:bbbb:bbbb:bb01::3:102/128\n\
             DNS = 10.64.0.1\n\
             \n\
             [Peer]\n\
             PublicKey = {KEY_B}\n\
             AllowedIPs = 0.0.0.0/0, ::/0\n\
             Endpoint = 146.70.1.1:51820\n\
             PersistentKeepalive = 25\n\
             \n\
             [Peer]\n\
             PublicKey = {KEY_A}\n\
             Endpoint = 1.1.1.1:1\n"
        )
    }

    #[test]
    fn parses_mullvad_style_config() {
        let config = TunnelConfig::parse_wg_quick(&sample()).unwrap();
        assert_eq!(config.address, Ipv4Addr::new(10, 66, 1, 2));
        assert_eq!(config.dns, Ipv4Addr::new(10, 64, 0, 1));
        assert_eq!(config.endpoint, "146.70.1.1:51820");
        assert_eq!(config.keepalive, Some(25));
        assert_eq!(
            config.peer_public_key,
            parse_key("PublicKey", KEY_B).unwrap()
        );
        assert_eq!(config.preshared_key, None);
    }

    #[test]
    fn picks_ipv4_when_ipv6_comes_first() {
        let text = sample().replace(
            "Address = 10.66.1.2/32, fc00:bbbb:bbbb:bb01::3:102/128",
            "address=fc00::2/128,10.9.0.7/32",
        );
        let config = TunnelConfig::parse_wg_quick(&text).unwrap();
        assert_eq!(config.address, Ipv4Addr::new(10, 9, 0, 7));
    }

    #[test]
    fn zero_keepalive_means_off() {
        let text = sample().replace("PersistentKeepalive = 25", "PersistentKeepalive = 0");
        assert_eq!(TunnelConfig::parse_wg_quick(&text).unwrap().keepalive, None);
    }

    #[test]
    fn reports_missing_dns() {
        let text = sample().replace("DNS = 10.64.0.1\n", "");
        assert_eq!(
            TunnelConfig::parse_wg_quick(&text).err(),
            Some(ConfigError::Missing("DNS"))
        );
    }

    #[test]
    fn rejects_bad_key() {
        let text = sample().replace(KEY_A, "short");
        assert!(matches!(
            TunnelConfig::parse_wg_quick(&text),
            Err(ConfigError::Invalid {
                key: "PrivateKey",
                ..
            })
        ));
    }
}
