use panelink_core::{local_peer_id, OperatingSystem, Peer, PeerStatus};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io,
    net::{Ipv4Addr, SocketAddrV4, UdpSocket},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

pub const SERVICE_NAME: &str = "_panelink._udp.local";
pub const DEFAULT_PORT: u16 = 48170;
pub const DEFAULT_PEER_TTL: Duration = Duration::from_secs(30);
pub const PAIRING_TOKEN_TTL: Duration = Duration::from_secs(5 * 60);
const DISCOVERY_MAGIC: &[u8] = b"PANELINK_DISCOVERY_V1\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryConfig {
    pub service_name: &'static str,
    pub port: u16,
    pub peer_ttl: Duration,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            service_name: SERVICE_NAME,
            port: DEFAULT_PORT,
            peer_ttl: DEFAULT_PEER_TTL,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvertisementPayload {
    pub service: String,
    pub peer_id: String,
    pub peer_name: String,
    pub os: OperatingSystem,
    pub address: String,
    pub port: u16,
    pub app_version: String,
    pub transport: String,
    pub pairing_required: bool,
}

impl AdvertisementPayload {
    pub fn local(config: &DiscoveryConfig) -> Self {
        Self {
            service: config.service_name.to_string(),
            peer_id: local_peer_id(),
            peer_name: local_peer_name(),
            os: local_operating_system(),
            address: local_lan_address().unwrap_or_else(|| "0.0.0.0".into()),
            port: config.port,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            transport: "udp-lan-discovery".into(),
            pairing_required: true,
        }
    }

    pub fn endpoint(&self) -> String {
        format!("{}:{}", self.address, self.port)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingToken {
    pub peer_id: String,
    pub token: String,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

impl PairingToken {
    pub fn issue(peer_id: impl Into<String>) -> Self {
        Self::issue_at(peer_id, now_unix_ms(), PAIRING_TOKEN_TTL)
    }

    pub fn issue_at(peer_id: impl Into<String>, issued_at_unix_ms: u64, ttl: Duration) -> Self {
        let ttl_ms = ttl.as_millis().min(u128::from(u64::MAX)) as u64;

        Self {
            peer_id: peer_id.into(),
            token: Uuid::new_v4().to_string(),
            issued_at_unix_ms,
            expires_at_unix_ms: issued_at_unix_ms.saturating_add(ttl_ms),
        }
    }

    pub fn is_expired_at(&self, now_unix_ms: u64) -> bool {
        now_unix_ms >= self.expires_at_unix_ms
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedPeer {
    pub peer: Peer,
    pub advertisement: AdvertisementPayload,
    pub first_seen_unix_ms: u64,
    pub last_seen_unix_ms: u64,
}

#[derive(Debug, Clone)]
pub struct PeerRegistry {
    peers: HashMap<String, CachedPeer>,
    ttl: Duration,
}

impl PeerRegistry {
    pub fn new(ttl: Duration) -> Self {
        Self {
            peers: HashMap::new(),
            ttl,
        }
    }

    pub fn upsert(&mut self, advertisement: AdvertisementPayload) -> CachedPeer {
        self.upsert_at(advertisement, now_unix_ms())
    }

    pub fn upsert_at(
        &mut self,
        advertisement: AdvertisementPayload,
        seen_at_unix_ms: u64,
    ) -> CachedPeer {
        let advertisement = normalize_advertisement(advertisement);
        let existing_first_seen = self
            .peers
            .get(&advertisement.peer_id)
            .map(|cached| cached.first_seen_unix_ms)
            .unwrap_or(seen_at_unix_ms);

        let cached = CachedPeer {
            peer: peer_from_advertisement(&advertisement, PeerStatus::Online),
            advertisement,
            first_seen_unix_ms: existing_first_seen,
            last_seen_unix_ms: seen_at_unix_ms,
        };

        self.peers.insert(cached.peer.id.clone(), cached.clone());
        cached
    }

    pub fn issue_pairing_token(&self, peer_id: &str) -> Option<PairingToken> {
        self.peers
            .contains_key(peer_id)
            .then(|| PairingToken::issue(peer_id))
    }

    pub fn list_peers(&self) -> Vec<Peer> {
        let mut peers = self
            .peers
            .values()
            .filter(|cached| cached.peer.id != local_peer_id())
            .map(|cached| cached.peer.clone())
            .collect::<Vec<_>>();

        peers.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
        peers
    }

    pub fn expire_stale(&mut self) {
        self.expire_stale_at(now_unix_ms());
    }

    pub fn expire_stale_at(&mut self, now_unix_ms: u64) {
        let ttl_ms = self.ttl.as_millis().min(u128::from(u64::MAX)) as u64;

        self.peers
            .retain(|_, cached| now_unix_ms.saturating_sub(cached.last_seen_unix_ms) <= ttl_ms);
    }
}

impl Default for PeerRegistry {
    fn default() -> Self {
        Self::new(DEFAULT_PEER_TTL)
    }
}

#[derive(Debug, Clone)]
pub struct DiscoveryService {
    config: DiscoveryConfig,
    registry: PeerRegistry,
    local_advertisement: AdvertisementPayload,
}

impl DiscoveryService {
    pub fn new(config: DiscoveryConfig) -> Self {
        let local_advertisement = AdvertisementPayload::local(&config);
        let mut registry = PeerRegistry::new(config.peer_ttl);
        registry.upsert(local_advertisement.clone());

        Self {
            config,
            registry,
            local_advertisement,
        }
    }

    pub fn advertise_local_peer(&mut self) -> AdvertisementPayload {
        self.local_advertisement = AdvertisementPayload::local(&self.config);
        self.registry.upsert(self.local_advertisement.clone());
        self.local_advertisement.clone()
    }

    pub fn ingest_peer(&mut self, advertisement: AdvertisementPayload) -> CachedPeer {
        self.registry.upsert(advertisement)
    }

    pub fn scan_lan(&mut self, timeout: Duration) -> io::Result<Vec<Peer>> {
        let local = self.advertise_local_peer();
        let packet = encode_advertisement(&local)?;
        let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, self.config.port))?;
        socket.set_broadcast(true)?;
        socket.set_read_timeout(Some(Duration::from_millis(60)))?;

        for target in discovery_targets(self.config.port) {
            let _ = socket.send_to(&packet, target);
        }

        let started_at = Instant::now();
        let mut buffer = [0_u8; 4096];

        while started_at.elapsed() < timeout {
            match socket.recv_from(&mut buffer) {
                Ok((size, from)) => {
                    if let Some(mut advertisement) = decode_advertisement(&buffer[..size]) {
                        if advertisement.peer_id == local.peer_id {
                            continue;
                        }

                        if is_unspecified_or_loopback(&advertisement.address) {
                            advertisement.address = from.ip().to_string();
                        }

                        self.ingest_peer(advertisement);
                        let _ = socket.send_to(&packet, from);
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            }
        }

        Ok(self.list_cached_peers())
    }

    pub fn list_cached_peers(&mut self) -> Vec<Peer> {
        self.registry.upsert(self.local_advertisement.clone());
        self.registry.expire_stale();
        self.registry.list_peers()
    }

    pub fn issue_pairing_token(&self, peer_id: &str) -> Option<PairingToken> {
        self.registry.issue_pairing_token(peer_id)
    }
}

impl Default for DiscoveryService {
    fn default() -> Self {
        Self::new(DiscoveryConfig::default())
    }
}

pub fn advertise_payload() -> AdvertisementPayload {
    with_discovery_service(|service| service.advertise_local_peer())
}

pub fn list_cached_peers() -> Vec<Peer> {
    with_discovery_service(|service| service.list_cached_peers())
}

pub fn scan_lan_peers(timeout: Duration) -> io::Result<Vec<Peer>> {
    with_discovery_service(|service| service.scan_lan(timeout))
}

pub fn ingest_peer_advertisement(advertisement: AdvertisementPayload) -> CachedPeer {
    with_discovery_service(|service| service.ingest_peer(advertisement))
}

pub fn issue_pairing_token(peer_id: &str) -> Option<PairingToken> {
    with_discovery_service(|service| service.issue_pairing_token(peer_id))
}

fn with_discovery_service<T>(run: impl FnOnce(&mut DiscoveryService) -> T) -> T {
    static SERVICE: OnceLock<Mutex<DiscoveryService>> = OnceLock::new();

    let mut service = SERVICE
        .get_or_init(|| Mutex::new(DiscoveryService::default()))
        .lock()
        .expect("discovery service mutex poisoned");

    run(&mut service)
}

fn peer_from_advertisement(advertisement: &AdvertisementPayload, status: PeerStatus) -> Peer {
    Peer {
        id: advertisement.peer_id.clone(),
        name: advertisement.peer_name.clone(),
        os: advertisement.os,
        address: advertisement.endpoint(),
        last_seen: "Now".into(),
        status,
        trusted: !advertisement.pairing_required,
        latency_ms: 0,
    }
}

fn encode_advertisement(advertisement: &AdvertisementPayload) -> io::Result<Vec<u8>> {
    let mut packet = DISCOVERY_MAGIC.to_vec();
    let body = serde_json::to_vec(advertisement).map_err(io::Error::other)?;
    packet.extend(body);
    Ok(packet)
}

fn decode_advertisement(packet: &[u8]) -> Option<AdvertisementPayload> {
    let body = packet.strip_prefix(DISCOVERY_MAGIC)?;
    serde_json::from_slice(body).ok()
}

fn normalize_advertisement(mut advertisement: AdvertisementPayload) -> AdvertisementPayload {
    if is_unspecified_or_loopback(&advertisement.address) {
        advertisement.address = "0.0.0.0".into();
    }

    advertisement
}

fn is_unspecified_or_loopback(address: &str) -> bool {
    address == "0.0.0.0" || address == "127.0.0.1" || address == "::1"
}

fn local_lan_address() -> Option<String> {
    UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))
        .ok()
        .and_then(|socket| {
            socket
                .connect(SocketAddrV4::new(Ipv4Addr::new(1, 1, 1, 1), 80))
                .ok()?;
            socket.local_addr().ok()
        })
        .map(|address| address.ip().to_string())
        .filter(|address| !is_unspecified_or_loopback(address))
}

fn discovery_targets(port: u16) -> Vec<SocketAddrV4> {
    let mut targets = vec![SocketAddrV4::new(Ipv4Addr::BROADCAST, port)];

    if let Some(address) = local_lan_address().and_then(|address| address.parse::<Ipv4Addr>().ok())
    {
        let octets = address.octets();
        let directed_broadcast = Ipv4Addr::new(octets[0], octets[1], octets[2], 255);

        if directed_broadcast != Ipv4Addr::BROADCAST {
            targets.push(SocketAddrV4::new(directed_broadcast, port));
        }
    }

    targets
}

fn local_operating_system() -> OperatingSystem {
    if cfg!(target_os = "macos") {
        OperatingSystem::MacOs
    } else {
        OperatingSystem::Windows
    }
}

fn local_peer_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "PaneLink device".into())
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn advertisement(peer_id: &str, name: &str) -> AdvertisementPayload {
        AdvertisementPayload {
            service: SERVICE_NAME.into(),
            peer_id: peer_id.into(),
            peer_name: name.into(),
            os: OperatingSystem::Windows,
            address: "192.168.1.20".into(),
            port: DEFAULT_PORT,
            app_version: "0.1.0".into(),
            transport: "udp-lan-discovery".into(),
            pairing_required: true,
        }
    }

    #[test]
    fn local_advertisement_contains_stable_peer_identity_and_endpoint() {
        let config = DiscoveryConfig::default();
        let first = AdvertisementPayload::local(&config);
        let second = AdvertisementPayload::local(&config);

        assert_eq!(first.peer_id, second.peer_id);
        assert_eq!(first.service, SERVICE_NAME);
        assert_eq!(first.port, DEFAULT_PORT);
        assert_ne!(first.endpoint(), format!("127.0.0.1:{}", DEFAULT_PORT));
        assert!(first.pairing_required);
    }

    #[test]
    fn registry_upserts_and_expires_peers() {
        let mut registry = PeerRegistry::new(Duration::from_millis(100));

        registry.upsert_at(advertisement("peer-a", "Desk"), 1_000);
        registry.upsert_at(advertisement("peer-a", "Desk renamed"), 1_050);

        let peers = registry.list_peers();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].name, "Desk renamed");

        registry.expire_stale_at(1_151);
        assert!(registry.list_peers().is_empty());
    }

    #[test]
    fn pairing_token_is_scoped_and_expires() {
        let token = PairingToken::issue_at("peer-a", 10_000, Duration::from_millis(500));

        assert_eq!(token.peer_id, "peer-a");
        assert!(!token.token.is_empty());
        assert!(!token.is_expired_at(10_499));
        assert!(token.is_expired_at(10_500));
    }

    #[test]
    fn discovery_packets_roundtrip() {
        let advertisement = advertisement("peer-a", "Desk");
        let packet = encode_advertisement(&advertisement).expect("packet should encode");
        let decoded = decode_advertisement(&packet).expect("packet should decode");

        assert_eq!(decoded, advertisement);
    }

    #[test]
    fn discovery_targets_include_global_broadcast() {
        let targets = discovery_targets(DEFAULT_PORT);

        assert!(targets.contains(&SocketAddrV4::new(Ipv4Addr::BROADCAST, DEFAULT_PORT)));
    }
}
