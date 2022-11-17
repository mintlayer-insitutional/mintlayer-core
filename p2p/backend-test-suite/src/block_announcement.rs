// Copyright (c) 2022 RBB S.r.l
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

use std::{fmt::Debug, sync::Arc};

use tokio::{pin, select, time::Duration};

use common::{
    chain::{
        block::{consensus_data::ConsensusData, timestamp::BlockTimestamp, Block, BlockReward},
        transaction::signed_transaction::SignedTransaction,
        transaction::Transaction,
    },
    primitives::{Id, H256},
};
use serialization::Encode;

use p2p::{
    error::{P2pError, PublishError},
    message::Announcement,
    net::{
        types::{PubSubTopic, SyncingEvent},
        ConnectivityService, NetworkingService, SyncingMessagingService,
    },
};
use p2p_test_utils::{connect_services, MakeTestAddress};

tests![
    block_announcement,
    block_announcement_no_subscription,
    block_announcement_too_big_message,
];

async fn block_announcement<A, S>()
where
    A: MakeTestAddress<Address = S::Address>,
    S: NetworkingService + Debug,
    S::SyncingMessagingHandle: SyncingMessagingService<S>,
    S::ConnectivityHandle: ConnectivityService<S>,
{
    let config = Arc::new(common::chain::config::create_mainnet());
    let (mut conn1, mut sync1) =
        S::start(A::make_address(), Arc::clone(&config), Default::default())
            .await
            .unwrap();
    let (mut conn2, mut sync2) =
        S::start(A::make_address(), Arc::clone(&config), Default::default())
            .await
            .unwrap();

    connect_services::<S>(&mut conn1, &mut conn2).await;

    sync1.subscribe(&[PubSubTopic::Blocks]).await.unwrap();
    sync2.subscribe(&[PubSubTopic::Blocks]).await.unwrap();

    // Spam the message until until we have a peer.
    loop {
        let res = sync1
            .make_announcement(Announcement::Block(
                Block::new(
                    vec![],
                    Id::new(H256([0x01; 32])),
                    BlockTimestamp::from_int_seconds(1337u64),
                    ConsensusData::None,
                    BlockReward::new(Vec::new()),
                )
                .unwrap(),
            ))
            .await;

        match res {
            Ok(()) => break,
            Err(e) => assert_eq!(e, P2pError::PublishError(PublishError::InsufficientPeers)),
        }
    }

    // Poll an event from the network for server2.
    let block = match sync2.poll_next().await.unwrap() {
        SyncingEvent::Announcement {
            peer_id: _,
            message_id: _,
            announcement: Announcement::Block(block),
        } => block,
        _ => panic!("Unexpected event"),
    };
    assert_eq!(block.timestamp().as_int_seconds(), 1337u64);
    sync2
        .make_announcement(Announcement::Block(
            Block::new(
                vec![],
                Id::new(H256([0x02; 32])),
                BlockTimestamp::from_int_seconds(1338u64),
                ConsensusData::None,
                BlockReward::new(Vec::new()),
            )
            .unwrap(),
        ))
        .await
        .unwrap();

    let block = match sync1.poll_next().await.unwrap() {
        SyncingEvent::Announcement {
            peer_id: _,
            message_id: _,
            announcement: Announcement::Block(block),
        } => block,
        _ => panic!("Unexpected event"),
    };
    assert_eq!(block.timestamp(), BlockTimestamp::from_int_seconds(1338u64));
}

async fn block_announcement_no_subscription<A, S>()
where
    A: MakeTestAddress<Address = S::Address>,
    S: NetworkingService + Debug,
    S::SyncingMessagingHandle: SyncingMessagingService<S>,
    S::ConnectivityHandle: ConnectivityService<S>,
{
    let config = Arc::new(common::chain::config::create_mainnet());
    let (mut conn1, mut sync1) =
        S::start(A::make_address(), Arc::clone(&config), Default::default())
            .await
            .unwrap();
    let (mut conn2, _sync2) = S::start(A::make_address(), Arc::clone(&config), Default::default())
        .await
        .unwrap();

    connect_services::<S>(&mut conn1, &mut conn2).await;

    let timeout = tokio::time::sleep(Duration::from_secs(1));
    pin!(timeout);
    loop {
        select! {
            res = sync1.make_announcement(Announcement::Block(
                Block::new(
                    vec![],
                    Id::new(H256([0x01; 32])),
                    BlockTimestamp::from_int_seconds(1337u64),
                    ConsensusData::None,
                    BlockReward::new(Vec::new()),
                )
                .unwrap(),
            )) => {
                assert_eq!(Err(P2pError::PublishError(PublishError::InsufficientPeers)), res);
            }
            _ = &mut timeout => break,
        }
    }
}

async fn block_announcement_too_big_message<A, S>()
where
    A: MakeTestAddress<Address = S::Address>,
    S: NetworkingService + Debug,
    S::SyncingMessagingHandle: SyncingMessagingService<S>,
    S::ConnectivityHandle: ConnectivityService<S>,
{
    let config = Arc::new(common::chain::config::create_mainnet());
    let (mut conn1, mut sync1) =
        S::start(A::make_address(), Arc::clone(&config), Default::default())
            .await
            .unwrap();

    let (mut conn2, mut sync2) =
        S::start(A::make_address(), Arc::clone(&config), Default::default())
            .await
            .unwrap();

    connect_services::<S>(&mut conn1, &mut conn2).await;

    sync1.subscribe(&[PubSubTopic::Blocks]).await.unwrap();
    sync2.subscribe(&[PubSubTopic::Blocks]).await.unwrap();

    let txs = (0..200_000)
        .map(|_| {
            SignedTransaction::new(Transaction::new(0, vec![], vec![], 0).unwrap(), vec![])
                .expect("invalid witness count")
        })
        .collect::<Vec<_>>();
    let message = Announcement::Block(
        Block::new(
            txs,
            Id::new(H256([0x04; 32])),
            BlockTimestamp::from_int_seconds(1337u64),
            ConsensusData::None,
            BlockReward::new(Vec::new()),
        )
        .unwrap(),
    );
    let encoded_size = message.encode().len();
    // TODO: move this to a spec.rs so it's accessible everywhere
    const MAXIMUM_SIZE: usize = 2 * 1024 * 1024;

    assert_eq!(
        sync1.make_announcement(message).await,
        Err(P2pError::PublishError(PublishError::MessageTooLarge(
            Some(encoded_size),
            Some(MAXIMUM_SIZE)
        )))
    );
}