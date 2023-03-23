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

use std::{num::NonZeroU64, time::Duration};

use super::helpers::{block_index_handle_impl::TestBlockIndexHandle, new_pub_key_destination};

use chainstate::chainstate_interface::ChainstateInterface;
use chainstate_storage::{BlockchainStorageRead, Transactional};
use chainstate_test_framework::{
    anyonecanspend_address, empty_witness, TestFramework, TransactionBuilder,
};
use chainstate_types::vrf_tools::construct_transcript;
use common::{
    chain::{
        block::{consensus_data::PoSData, timestamp::BlockTimestamp, ConsensusData},
        config::Builder as ConfigBuilder,
        signature::inputsig::InputWitness,
        stakelock::StakePoolData,
        ConsensusUpgrade, NetUpgrades, OutPoint, OutPointSourceId, PoSChainConfig, PoolId,
        RequiredConsensus, TxInput, TxOutput, UpgradeVersion,
    },
    primitives::{Amount, BlockHeight, Idable},
    Uint256,
};
use crypto::{random::CryptoRng, vrf::VRFPublicKey};
use crypto::{
    random::Rng,
    vrf::{VRFKeyKind, VRFPrivateKey},
};
use rstest::rstest;
use test_utils::random::{make_seedable_rng, Seed};

const TARGET_BLOCK_TIME: Duration = Duration::from_secs(2 * 60);

// Create a chain genesis <- block_1
// block_1 has tx with StakePool output
fn setup_test_chain_with_staked_pool(
    rng: &mut (impl Rng + CryptoRng),
    vrf_pk: VRFPublicKey,
) -> (TestFramework, PoolId) {
    let pos_config = PoSChainConfig::new(true, Uint256::MAX, TARGET_BLOCK_TIME, 0.into());
    let upgrades = vec![
        (
            BlockHeight::new(0),
            UpgradeVersion::ConsensusUpgrade(ConsensusUpgrade::IgnoreConsensus),
        ),
        (
            BlockHeight::new(2),
            UpgradeVersion::ConsensusUpgrade(ConsensusUpgrade::PoS {
                initial_difficulty: Uint256::MAX.into(),
                config: pos_config,
            }),
        ),
    ];
    let net_upgrades = NetUpgrades::initialize(upgrades).expect("valid net-upgrades");
    let chain_config = ConfigBuilder::test_chain()
        .net_upgrades(net_upgrades)
        .epoch_length(NonZeroU64::new(2).unwrap())
        .sealed_epoch_distance_from_tip(0)
        .build();
    let mut tf = TestFramework::builder(rng).with_chain_config(chain_config).build();

    let genesis_id = tf.genesis().get_id();
    let pool_id = pos_accounting::make_pool_id(&OutPoint::new(
        OutPointSourceId::BlockReward(genesis_id.into()),
        0,
    ));

    let stake_pool_data = StakePoolData::new(
        Amount::from_atoms(1),
        anyonecanspend_address(),
        vrf_pk,
        new_pub_key_destination(rng),
        0,
        Amount::ZERO,
    );

    let tx = TransactionBuilder::new()
        .add_input(
            TxInput::new(OutPointSourceId::BlockReward(genesis_id.into()), 0),
            empty_witness(rng),
        )
        .add_output(TxOutput::StakePool(Box::new(stake_pool_data)))
        .build();
    tf.make_block_builder().add_transaction(tx).build_and_process().unwrap();

    (tf, pool_id)
}

#[rstest]
#[trace]
#[case(Seed::from_entropy())]
fn stable_difficulty(#[case] seed: Seed) {
    let mut rng = make_seedable_rng(seed);
    let (vrf_sk, vrf_pk) = VRFPrivateKey::new_from_rng(&mut rng, VRFKeyKind::Schnorrkel);
    let (mut tf, pool_id) = setup_test_chain_with_staked_pool(&mut rng, vrf_pk.clone());

    for i in 0..10 {
        let new_block_time = BlockTimestamp::from_duration_since_epoch(tf.current_time());
        let new_block_height = tf.best_block_index().block_height().next_height();
        let new_target = {
            let pos_status = match tf
                .chainstate
                .get_chain_config()
                .net_upgrade()
                .consensus_status(new_block_height)
            {
                RequiredConsensus::PoS(status) => status,
                RequiredConsensus::PoW(_)
                | RequiredConsensus::DSA
                | RequiredConsensus::IgnoreConsensus => panic!("Invalid consensus"),
            };

            let tmp_block = tf.make_block_builder().build();

            let db_tx = tf.storage.transaction_ro().unwrap();
            let block_index_handle =
                TestBlockIndexHandle::new(db_tx, tf.chainstate.get_chain_config().as_ref());

            consensus::calculate_target_required(
                tf.chainstate.get_chain_config().as_ref(),
                &pos_status,
                tmp_block.header(),
                &block_index_handle,
            )
            .unwrap()
        };

        let current_epoch_index =
            tf.chainstate.get_chain_config().epoch_index_from_height(&new_block_height);
        let sealed_epoch_index =
            tf.chainstate.get_chain_config().sealed_epoch_index(&new_block_height).unwrap();
        let sealed_epoch_randomness = tf
            .storage
            .transaction_ro()
            .unwrap()
            .get_epoch_data(sealed_epoch_index)
            .unwrap()
            .map(|d| d.randomness().value())
            .unwrap_or(tf.chainstate.get_chain_config().initial_randomness());

        let transcript = construct_transcript(
            current_epoch_index,
            &sealed_epoch_randomness,
            new_block_time,
        );
        let vrf_data = vrf_sk.produce_vrf_data(transcript.into());
        let best_block_outputs = tf.outputs_from_genblock(tf.best_block_id());
        let pos_data = PoSData::new(
            vec![TxInput::new(best_block_outputs.keys().next().unwrap().clone(), 0)],
            vec![InputWitness::NoSignature(None)],
            pool_id,
            vrf_data,
            new_target,
        );

        let reward_output = TxOutput::ProduceBlockFromStake(
            Amount::from_atoms(1),
            anyonecanspend_address(),
            pool_id,
        );
        tf.make_block_builder()
            .with_consensus_data(ConsensusData::PoS(Box::new(pos_data.clone())))
            .with_timestamp(new_block_time)
            .with_reward(vec![reward_output])
            .build_and_process()
            .unwrap();

        tf.progress_time_seconds_since_epoch(TARGET_BLOCK_TIME.as_secs());
    }
}
