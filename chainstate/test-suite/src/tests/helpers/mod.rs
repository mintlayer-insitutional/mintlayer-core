// Copyright (c) 2021-2022 RBB S.r.l
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

use chainstate_test_framework::{anyonecanspend_address, TestFramework, TransactionBuilder};
use common::{
    chain::{
        block::timestamp::BlockTimestamp, signature::inputsig::InputWitness,
        timelock::OutputTimeLock, tokens::OutputValue, OutputPurpose, TxInput, TxOutput,
    },
    primitives::{Amount, BlockDistance},
};

pub mod in_memory_storage_wrapper;

/// Adds a block with the locked output and returns input corresponding to this output.
pub fn add_block_with_locked_output(
    tf: &mut TestFramework,
    output_time_lock: OutputTimeLock,
    timestamp: BlockTimestamp,
) -> (InputWitness, TxInput) {
    // Find the last block.
    let current_height = tf.best_block_index().block_height();
    let prev_block_info = tf.block_info(current_height.into());

    tf.make_block_builder()
        .add_transaction(
            TransactionBuilder::new()
                .add_input(
                    TxInput::new(prev_block_info.txns[0].0.clone(), 0),
                    InputWitness::NoSignature(None),
                )
                .add_anyone_can_spend_output(10000)
                .add_output(TxOutput::new(
                    OutputValue::Coin(Amount::from_atoms(100000)),
                    OutputPurpose::LockThenTransfer(anyonecanspend_address(), output_time_lock),
                ))
                .build(),
        )
        .with_timestamp(timestamp)
        .build_and_process()
        .unwrap();

    let new_height = (current_height + BlockDistance::new(1)).unwrap();
    assert_eq!(tf.best_block_index().block_height(), new_height);

    let block_info = tf.block_info(new_height.into());
    (
        InputWitness::NoSignature(None),
        TxInput::new(block_info.txns[0].0.clone(), 1),
    )
}
