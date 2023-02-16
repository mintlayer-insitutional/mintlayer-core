// Copyright (c) 2023 RBB S.r.l
// opensource@mintlayer.org
// SPDX-License-Identifier: MIT
// Licensed under the MIT License;
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://github.com/mintlayer/mintlayer-core/blob/master/LICENSE
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Mintlayer network crawler
//!
//! To keep things simple, the server will try to keep connections open to all reachable nodes.
//! When a new outbound connection is made, a new DNS record is added (but only for nodes on default ports).
//! When the connection is closed, the DNS record is removed.
//! When a connection fails, the server increases the backoff time between connection attempts.
//! If the number of failed connection attempts exceeds the limit, the address is removed from the list.
//! Once-reachable and newer-reachable addresses have different connection failure limits
//! (equivalent to about 1 month and about 1 hour, respectively).

pub mod storage;
pub mod storage_impl;

use std::{
    collections::{btree_map::Entry, BTreeMap},
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use common::chain::ChainConfig;
use crypto::random::{make_pseudo_rng, seq::IteratorRandom};
use logging::log;
use p2p::{
    error::P2pError,
    message::{AnnounceAddrRequest, PeerManagerMessage, PingRequest, PingResponse},
    net::{
        default_backend::transport::TransportAddress,
        types::{ConnectivityEvent, PeerInfo, SyncingEvent},
        ConnectivityService, NetworkingService, SyncingMessagingService,
    },
    peer_manager::global_ip::IsGlobalIp,
    types::peer_address::PeerAddress,
};
use tokio::sync::mpsc;

use crate::{
    crawler::storage::{DnsServerStorageWrite, DnsServerTransactionRw},
    dns_server::ServerCommands,
    error::DnsServerError,
};

use self::storage::{DnsServerStorage, DnsServerStorageRead, DnsServerTransactionRo};

/// How often the server performs maintenance (tries to connect to new nodes)
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

/// How many outbound connection attempts can be made per heartbeat
const MAX_CONNECTS_PER_HEARTBEAT: usize = 25;

/// When the server drops the unreachable node address. Used for negative caching.
const PURGE_UNREACHABLE_TIME: Duration = Duration::from_secs(3600);

/// When the server drops the unreachable node address that was once reachable. This should take about a month.
/// Such a long time is useful if the server itself has prolonged connectivity problems.
const PURGE_REACHABLE_FAIL_COUNT: u32 = 35;

const STORAGE_VERSION: u32 = 1;

/// Connection state of a potential node address (outbound only)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AddressState {
    Disconnected,
    Connecting,
    Connected,
}

/// Additional state of a potential node address
struct AddressData {
    /// Connection state
    state: AddressState,

    /// Last time when the connection state was updated
    state_updated_at: tokio::time::Instant,

    /// The number of failed connection attempts.
    /// New connection attempts are made after a progressive backoff time.
    /// Resets to 0 when an outbound connection to the address is successful.
    fail_count: u32,

    /// Whether the address was reachable at least once.
    /// Addresses that were once reachable are stored in the DB.
    was_reachable: bool,

    /// Whether the address was added from the command line
    user_added: bool,
}

#[derive(Clone)]
pub struct CrawlerConfig {
    pub add_node: Vec<String>,

    pub p2p_port: u16,
}

pub struct Crawler<N: NetworkingService, S> {
    /// Crawler config
    config: CrawlerConfig,

    /// Chain config
    chain_config: Arc<ChainConfig>,

    /// Backend's ConnectivityHandle
    conn: N::ConnectivityHandle,

    /// Backend's SyncingMessagingHandle
    sync: N::SyncingMessagingHandle,

    /// Map of all known addresses (including currently unreachable)
    addresses: BTreeMap<N::Address, AddressData>,

    /// Map of all currently connected outbound peers
    outbound_peers: BTreeMap<N::PeerId, N::Address>,

    /// Storage implementation
    storage: S,

    /// Channel used to manage the DNS server
    command_tx: mpsc::UnboundedSender<ServerCommands>,
}

impl<N: NetworkingService + 'static, S: DnsServerStorage> Crawler<N, S>
where
    N::SyncingMessagingHandle: SyncingMessagingService<N>,
    N::ConnectivityHandle: ConnectivityService<N>,
    DnsServerError: From<<<N as NetworkingService>::Address as FromStr>::Err>,
{
    pub fn new(
        config: CrawlerConfig,
        chain_config: Arc<ChainConfig>,
        conn: N::ConnectivityHandle,
        sync: N::SyncingMessagingHandle,
        storage: S,
        command_tx: mpsc::UnboundedSender<ServerCommands>,
    ) -> Result<Self, DnsServerError> {
        // Addresses that are stored in the DB as reachable
        let loaded_addresses: Vec<N::Address> = Self::load_storage(&storage)?;

        // Addresses listed as reachable from the command line
        let added_addresses: Vec<N::Address> = config
            .add_node
            .iter()
            .map(|addr| addr.parse())
            .collect::<Result<Vec<N::Address>, _>>()?;

        // Added addresses must come after loaded addresses to not lose the `user_added` flag!
        let addresses = loaded_addresses
            .into_iter()
            .map(|addr| {
                (
                    addr,
                    AddressData {
                        state: AddressState::Disconnected,
                        state_updated_at: tokio::time::Instant::now(),
                        fail_count: 0,
                        was_reachable: true,
                        user_added: false,
                    },
                )
            })
            .chain(added_addresses.into_iter().map(|addr| {
                (
                    addr,
                    AddressData {
                        state: AddressState::Disconnected,
                        state_updated_at: tokio::time::Instant::now(),
                        fail_count: 0,
                        was_reachable: false,
                        user_added: true,
                    },
                )
            }))
            .collect();

        Ok(Self {
            config,
            chain_config,
            conn,
            sync,
            addresses,
            outbound_peers: BTreeMap::new(),
            storage,
            command_tx,
        })
    }

    fn load_storage(storage: &S) -> Result<Vec<N::Address>, DnsServerError> {
        let tx = storage.transaction_ro()?;
        let version = tx.get_version()?;
        tx.close();

        match version {
            None => Self::init_storage(storage),
            Some(STORAGE_VERSION) => Self::load_storage_v1(storage),
            Some(_version) => Err(DnsServerError::Other("Unexpected storage version")),
        }
    }

    fn init_storage(storage: &S) -> Result<Vec<N::Address>, DnsServerError> {
        let mut tx = storage.transaction_rw()?;
        tx.set_version(STORAGE_VERSION)?;
        tx.commit()?;
        Ok(Vec::new())
    }

    fn load_storage_v1(storage: &S) -> Result<Vec<N::Address>, DnsServerError> {
        let tx = storage.transaction_ro()?;
        let addresses =
            tx.get_addresses()?.iter().filter_map(|address| address.parse().ok()).collect();
        Ok(addresses)
    }

    fn handle_conn_request(&mut self, peer_id: N::PeerId, request: PeerManagerMessage) {
        match request {
            PeerManagerMessage::AddrListRequest(_) => {
                // Ignored
            }
            PeerManagerMessage::AnnounceAddrRequest(AnnounceAddrRequest { address }) => {
                // TODO: Rate limit `AnnounceAddrRequest` requests from a specific peer to prevent DoS attack,
                // when too many invalid addresses are announced, preventing the server from discovering new addresses.
                // For example, Bitcoin Core allows 0.1 address/sec.
                if let Some(address) = TransportAddress::from_peer_address(&address, false) {
                    Self::new_address(&mut self.addresses, address);
                }
            }
            PeerManagerMessage::PingRequest(PingRequest { nonce }) => {
                let _ = self.conn.send_message(
                    peer_id,
                    PeerManagerMessage::PingResponse(PingResponse { nonce }),
                );
            }
            PeerManagerMessage::AddrListResponse(_) => {}
            PeerManagerMessage::AnnounceAddrResponse(_) => {}
            PeerManagerMessage::PingResponse(_) => {}
        }
    }

    fn handle_outbound_accepted(
        &mut self,
        address: N::Address,
        peer_info: PeerInfo<N::PeerId>,
        _receiver_address: Option<PeerAddress>,
    ) {
        let is_compatible = peer_info.is_compatible(&self.chain_config);

        if is_compatible {
            log::info!("successfully connected to {}", address.to_string());

            self.create_outbound_peer(peer_info.peer_id, address);
        } else {
            log::info!("incompatible peer detected at {}", address.to_string());

            self.disconnect_invalid_peer(peer_info.peer_id, address);
        }
    }

    fn handle_inbound_accepted(
        &mut self,
        _address: N::Address,
        _peer_info: PeerInfo<N::PeerId>,
        _receiver_address: Option<PeerAddress>,
    ) {
        unreachable!("unexpected inbound connection");
    }

    fn handle_connection_error(&mut self, address: N::Address, error: P2pError) {
        log::debug!("connection to {} failed: {}", address.to_string(), error);
        let address_data = self
            .addresses
            .get_mut(&address)
            .expect("address must be known (handle_connection_error)");
        Self::change_address_state(
            &self.config,
            &address,
            address_data,
            AddressState::Disconnected,
            &mut self.storage,
            &self.command_tx,
        );
    }

    fn handle_connection_closed(&mut self, peer_id: N::PeerId) {
        log::debug!("connection from peer {} closed", peer_id);
        if self.outbound_peers.get(&peer_id).is_some() {
            self.remove_outbound_peer(peer_id);
        }
    }

    fn handle_misbehaved(&mut self, _peer_id: N::PeerId, _error: P2pError) {
        // Ignore all misbehave reports

        // TODO: Should we ban peers when they send unexpected messages?
    }

    fn handle_conn_event(&mut self, event: ConnectivityEvent<N>) {
        match event {
            ConnectivityEvent::Message { peer, message } => {
                self.handle_conn_request(peer, message);
            }
            ConnectivityEvent::OutboundAccepted {
                address,
                peer_info,
                receiver_address,
            } => {
                self.handle_outbound_accepted(address, peer_info, receiver_address);
            }
            ConnectivityEvent::InboundAccepted {
                address,
                peer_info,
                receiver_address,
            } => {
                self.handle_inbound_accepted(address, peer_info, receiver_address);
            }
            ConnectivityEvent::ConnectionError { address, error } => {
                self.handle_connection_error(address, error);
            }
            ConnectivityEvent::ConnectionClosed { peer_id } => {
                self.handle_connection_closed(peer_id);
            }
            ConnectivityEvent::Misbehaved { peer_id, error } => {
                self.handle_misbehaved(peer_id, error);
            }
        }
    }

    fn handle_sync_event(&mut self, _event: SyncingEvent<N>) {
        // Ignore all sync events
    }

    fn new_address(addresses: &mut BTreeMap<N::Address, AddressData>, address: N::Address) {
        if let Entry::Vacant(vacant) = addresses.entry(address.clone()) {
            log::debug!("new address {} added", address.to_string());
            vacant.insert(AddressData {
                state: AddressState::Disconnected,
                state_updated_at: tokio::time::Instant::now(),
                fail_count: 0,
                was_reachable: false,
                user_added: false,
            });
        }
    }

    fn get_dns_ip(address: &N::Address, p2p_port: u16) -> Option<IpAddr> {
        // Only add nodes listening on the default port to DNS
        match address.as_peer_address() {
            PeerAddress::Ip4(addr)
                if Ipv4Addr::from(addr.ip).is_global_unicast_ip() && addr.port == p2p_port =>
            {
                Some(Ipv4Addr::from(addr.ip).into())
            }
            PeerAddress::Ip6(addr)
                if Ipv6Addr::from(addr.ip).is_global_unicast_ip() && addr.port == p2p_port =>
            {
                Some(Ipv6Addr::from(addr.ip).into())
            }
            _ => None,
        }
    }

    /// Update address state.
    ///
    /// The only place where the address state can be updated.
    fn change_address_state(
        config: &CrawlerConfig,
        address: &N::Address,
        address_data: &mut AddressData,
        new_state: AddressState,
        storage: &mut S,
        command_tx: &mpsc::UnboundedSender<ServerCommands>,
    ) {
        if address_data.state == new_state {
            return;
        }

        log::debug!(
            "change address {} state to {:?}",
            address.to_string(),
            new_state
        );

        let old_state = address_data.state;
        address_data.state = new_state;
        address_data.state_updated_at = tokio::time::Instant::now();

        let dns_ip = Self::get_dns_ip(address, config.p2p_port);

        match old_state {
            AddressState::Disconnected | AddressState::Connecting => {
                // Do nothing
            }
            AddressState::Connected => {
                // Reachable node has disconnected, update DNS
                if let Some(ip) = dns_ip {
                    command_tx.send(ServerCommands::DelAddress(ip)).expect("sending must succeed");
                }
            }
        }

        match new_state {
            AddressState::Connecting => {
                // Do nothing
            }
            AddressState::Connected => {
                if let Some(ip) = dns_ip {
                    command_tx.send(ServerCommands::AddAddress(ip)).expect("sending must succeed");
                }

                address_data.fail_count = 0;

                if !address_data.was_reachable {
                    address_data.was_reachable = true;

                    // New reachable address discovered
                    storage::update_db(storage, |tx| tx.add_address(&address.to_string()))
                        .expect("update_db must succeed (add_address)");
                }
            }
            AddressState::Disconnected => {
                address_data.fail_count += 1;
            }
        }
    }

    /// Returns true when it is time to attempt a new outbound connection
    fn connect_now(now: tokio::time::Instant, address_data: &AddressData) -> bool {
        let age = now.duration_since(address_data.state_updated_at);

        match address_data.state {
            AddressState::Connected | AddressState::Connecting => false,
            AddressState::Disconnected if address_data.user_added => {
                // Try to connect to the user added nodes more often
                match address_data.fail_count {
                    0 => true,
                    1 => age > Duration::from_secs(60),
                    2 => age > Duration::from_secs(360),
                    _ => age > Duration::from_secs(3600),
                }
            }
            AddressState::Disconnected if address_data.was_reachable => {
                match address_data.fail_count {
                    0 => true,
                    1 => age > Duration::from_secs(60),
                    2 => age > Duration::from_secs(360),
                    3 => age > Duration::from_secs(3600),
                    4 => age > Duration::from_secs(3 * 3600),
                    5 => age > Duration::from_secs(6 * 3600),
                    6 => age > Duration::from_secs(12 * 3600),
                    _ => age > Duration::from_secs(24 * 3600),
                }
            }
            AddressState::Disconnected => {
                // The address was never reachable, try to connect just once
                address_data.fail_count == 0
            }
        }
    }

    /// Returns true if the address should be kept in memory
    fn retain_address(
        now: tokio::time::Instant,
        address: &N::Address,
        address_data: &mut AddressData,
        storage: &mut S,
    ) -> bool {
        if address_data.user_added || address_data.state != AddressState::Disconnected {
            return true;
        }

        if address_data.was_reachable && address_data.fail_count >= PURGE_REACHABLE_FAIL_COUNT {
            log::debug!("purge old (once reachable) address {}", address.to_string());

            storage::update_db(storage, |tx| tx.del_address(&address.to_string()))
                .expect("update_db must succeed (retain_address)");

            return false;
        }

        if !address_data.was_reachable
            && address_data.fail_count > 0
            && now.duration_since(address_data.state_updated_at) >= PURGE_UNREACHABLE_TIME
        {
            log::debug!("purge old (unreachable) address {}", address.to_string());

            return false;
        }

        true
    }

    /// Create new outbound peer
    fn create_outbound_peer(&mut self, peer_id: N::PeerId, address: N::Address) {
        log::debug!(
            "outbound peer created, peer_id: {}, address: {}",
            peer_id,
            address.to_string()
        );

        let address_data =
            self.addresses.get_mut(&address).expect("address must be known (create_peer)");

        self.outbound_peers.insert(peer_id, address.clone());

        Self::change_address_state(
            &self.config,
            &address,
            address_data,
            AddressState::Connected,
            &mut self.storage,
            &self.command_tx,
        );
    }

    /// Remove existing outbound peer
    fn remove_outbound_peer(&mut self, peer_id: N::PeerId) {
        log::debug!("outbound peer removed, peer_id: {}", peer_id);

        let address = self
            .outbound_peers
            .remove(&peer_id)
            .expect("peer must be known (remove_outbound_peer)");

        let address_data = self
            .addresses
            .get_mut(&address)
            .expect("address must be known (remove_outbound_peer)");

        Self::change_address_state(
            &self.config,
            &address,
            address_data,
            AddressState::Disconnected,
            &mut self.storage,
            &self.command_tx,
        );
    }

    /// Invalid peer connected and should be disconnected
    fn disconnect_invalid_peer(&mut self, peer_id: N::PeerId, address: N::Address) {
        log::debug!("disconnect invalid peer, peer_id: {}", peer_id);

        self.conn.disconnect(peer_id).expect("disconnect must succeed");

        let address_data = self
            .addresses
            .get_mut(&address)
            .expect("address must be known (disconnect_invalid_peer)");

        Self::change_address_state(
            &self.config,
            &address,
            address_data,
            AddressState::Disconnected,
            &mut self.storage,
            &self.command_tx,
        );
    }

    /// Peer and address list maintenance.
    ///
    /// Select random addresses to connect to, delete old addresses from memory and DB.
    fn heartbeat(&mut self) {
        let now = tokio::time::Instant::now();
        let connecting_addresses = self
            .addresses
            .iter_mut()
            .filter(|(_address, address_data)| Self::connect_now(now, address_data))
            .choose_multiple(&mut make_pseudo_rng(), MAX_CONNECTS_PER_HEARTBEAT);

        for (address, address_data) in connecting_addresses {
            Self::change_address_state(
                &self.config,
                address,
                address_data,
                AddressState::Connecting,
                &mut self.storage,
                &self.command_tx,
            );

            let res = self.conn.connect(address.clone());
            if let Err(e) = res {
                log::debug!("connection to {} failed: {}", address.to_string(), e);
                Self::change_address_state(
                    &self.config,
                    address,
                    address_data,
                    AddressState::Disconnected,
                    &mut self.storage,
                    &self.command_tx,
                );
            }
        }

        let now = tokio::time::Instant::now();
        self.addresses.retain(|address, address_data| {
            Self::retain_address(now, address, address_data, &mut self.storage)
        });
    }

    pub async fn run(&mut self) -> Result<void::Void, DnsServerError> {
        let mut heartbeat_timer = tokio::time::interval(HEARTBEAT_INTERVAL);

        loop {
            tokio::select! {
                event_res = self.conn.poll_next() => {
                    self.handle_conn_event(event_res?);
                },
                event_res = self.sync.poll_next() => {
                    self.handle_sync_event(event_res?);
                },
                _ = heartbeat_timer.tick() => {
                    self.heartbeat();
                },
            }
        }
    }
}

#[cfg(test)]
mod tests;