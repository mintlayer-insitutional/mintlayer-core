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

//! Common code for wallet UI applications

pub mod cookie;
pub mod mnemonic;
mod sync;

use std::{net::SocketAddr, sync::Arc};

use common::{
    chain::{Block, ChainConfig, GenBlock},
    primitives::{BlockHeight, Id},
};
pub use node_comm::node_traits::{ConnectedPeer, NodeInterface, PeerId};
use node_comm::{handles_client::WalletHandlesClient, make_rpc_client, rpc_client::NodeRpcClient};
use tokio::sync::Mutex;
use wallet::DefaultWallet;

#[derive(thiserror::Error, Debug)]
pub enum ControllerError {
    #[error("RPC error: {0}")]
    RpcError(String),
}

pub struct Controller<T> {
    rpc_client: T,
    _wallet: Arc<Mutex<DefaultWallet>>,
    sync_handle: tokio::task::JoinHandle<()>,
}

pub type RpcController = Controller<NodeRpcClient>;
pub type HandlesController = Controller<WalletHandlesClient>;

impl<T: NodeInterface + Clone + Send + Sync + 'static> Controller<T> {
    pub fn new(chain_config: Arc<ChainConfig>, rpc_client: T, wallet: DefaultWallet) -> Self {
        let wallet = Arc::new(Mutex::new(wallet));

        let sync_handle = tokio::spawn(sync::run_sync(
            chain_config,
            rpc_client.clone(),
            Arc::clone(&wallet),
        ));

        Self {
            rpc_client,
            _wallet: wallet,
            sync_handle,
        }
    }

    pub async fn get_best_block_id(&self) -> Result<Id<GenBlock>, ControllerError> {
        self.rpc_client
            .get_best_block_id()
            .await
            .map_err(|e| ControllerError::RpcError(e.to_string()))
    }

    pub async fn get_block(&self, block_id: Id<Block>) -> Result<Option<Block>, ControllerError> {
        self.rpc_client
            .get_block(block_id)
            .await
            .map_err(|e| ControllerError::RpcError(e.to_string()))
    }

    pub async fn get_best_block_height(&self) -> Result<BlockHeight, ControllerError> {
        self.rpc_client
            .get_best_block_height()
            .await
            .map_err(|e| ControllerError::RpcError(e.to_string()))
    }

    pub async fn get_block_id_at_height(
        &self,
        height: BlockHeight,
    ) -> Result<Option<Id<GenBlock>>, ControllerError> {
        self.rpc_client
            .get_block_id_at_height(height)
            .await
            .map_err(|e| ControllerError::RpcError(e.to_string()))
    }

    pub async fn submit_block(&self, block_hex: String) -> Result<(), ControllerError> {
        self.rpc_client
            .submit_block(block_hex)
            .await
            .map_err(|e| ControllerError::RpcError(e.to_string()))
    }

    pub async fn submit_transaction(&self, transaction_hex: String) -> Result<(), ControllerError> {
        self.rpc_client
            .submit_transaction(transaction_hex)
            .await
            .map_err(|e| ControllerError::RpcError(e.to_string()))
    }

    pub async fn node_shutdown(&self) -> Result<(), ControllerError> {
        self.rpc_client
            .node_shutdown()
            .await
            .map_err(|e| ControllerError::RpcError(e.to_string()))
    }

    pub async fn node_version(&self) -> Result<String, ControllerError> {
        self.rpc_client
            .node_version()
            .await
            .map_err(|e| ControllerError::RpcError(e.to_string()))
    }

    pub async fn p2p_connect(&self, address: String) -> Result<(), ControllerError> {
        self.rpc_client
            .p2p_connect(address)
            .await
            .map_err(|e| ControllerError::RpcError(e.to_string()))
    }

    pub async fn p2p_disconnect(&self, peer_id: PeerId) -> Result<(), ControllerError> {
        self.rpc_client
            .p2p_disconnect(peer_id)
            .await
            .map_err(|e| ControllerError::RpcError(e.to_string()))
    }

    pub async fn p2p_get_peer_count(&self) -> Result<usize, ControllerError> {
        self.rpc_client
            .p2p_get_peer_count()
            .await
            .map_err(|e| ControllerError::RpcError(e.to_string()))
    }

    pub async fn p2p_get_connected_peers(&self) -> Result<Vec<ConnectedPeer>, ControllerError> {
        self.rpc_client
            .p2p_get_connected_peers()
            .await
            .map_err(|e| ControllerError::RpcError(e.to_string()))
    }

    pub async fn p2p_add_reserved_node(&self, address: String) -> Result<(), ControllerError> {
        self.rpc_client
            .p2p_add_reserved_node(address)
            .await
            .map_err(|e| ControllerError::RpcError(e.to_string()))
    }

    pub async fn p2p_remove_reserved_node(&self, address: String) -> Result<(), ControllerError> {
        self.rpc_client
            .p2p_remove_reserved_node(address)
            .await
            .map_err(|e| ControllerError::RpcError(e.to_string()))
    }
}

impl<T> Drop for Controller<T> {
    fn drop(&mut self) {
        // TODO: Wait for the sync task to stop
        self.sync_handle.abort();
    }
}

pub async fn make_rpc_controller(
    chain_config: Arc<ChainConfig>,
    remote_socket_address: SocketAddr,
    username_password: Option<(&str, &str)>,
    wallet: DefaultWallet,
) -> Result<RpcController, ControllerError> {
    let rpc_client = make_rpc_client(remote_socket_address, username_password)
        .await
        .map_err(|e| ControllerError::RpcError(e.to_string()))?;
    Ok(Controller::new(chain_config, rpc_client, wallet))
}
