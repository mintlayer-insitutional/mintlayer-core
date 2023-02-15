// Copyright (c) 2021 RBB S.r.l
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

use thiserror::Error;

use chainstate_types::vrf_tools::ProofOfStakeVRFError;
use common::{
    chain::{block::timestamp::BlockTimestamp, Block, GenBlock, PoolId},
    primitives::{Compact, Id},
};

#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum ConsensusPoSError {
    #[error("Property query error: `{0}`")]
    PropertyQueryError(#[from] chainstate_types::PropertyQueryError),
    #[error("Stake kernel hash failed to meet the target requirement")]
    StakeKernelHashTooHigh,
    #[error(
        "Stake block timestamp cannot be smaller than the kernel's (kernel: {0} < stake: {1})"
    )]
    TimestampViolation(BlockTimestamp, BlockTimestamp),
    #[error("Kernel inputs are empty")]
    NoKernel,
    #[error("Only one kernel allowed")]
    MultipleKernels,
    #[error("Could not find the transaction pointed to by an outpoint")]
    OutpointTransactionNotFound,
    #[error("Outpoint access error. Possibly invalid")]
    InIndexOutpointAccessError,
    #[error("Output already spent")]
    KernelOutputAlreadySpent,
    #[error("Kernel block index not found with block id: {0}")]
    KernelBlockIndexNotFound(Id<GenBlock>),
    #[error("Kernel output index out of range: {0}")]
    KernelOutputIndexOutOfRange(u32),
    #[error("Kernel input transaction not found")]
    KernelTransactionNotFound,
    #[error("Kernel header output load error")]
    KernelHeaderOutputDoesNotExist(Id<GenBlock>),
    #[error("Kernel header index out of range. Block id: {0} and index {1}")]
    KernelHeaderOutputIndexOutOfRange(Id<GenBlock>, u32),
    #[error("Bits to target conversion failed {0:?}")]
    BitsToTargetConversionFailed(Compact),
    #[error("Could not find the previous block index of block: {0}")]
    PrevBlockIndexNotFound(Id<Block>),
    #[error("The kernel is not an ancestor of the current header of id {0}. This is a double-spend attempt at best")]
    KernelAncestryCheckFailed(Id<Block>),
    #[error("Attempted to use a non-locked stake as stake kernel in block {0}")]
    InvalidOutputPurposeInStakeKernel(Id<Block>),
    #[error("Failed to verify VRF data with error: {0}")]
    VRFDataVerificationFailed(ProofOfStakeVRFError),
    #[error("Epoch data not found for index: {0}")]
    EpochDataNotFound(u64),
    #[error("Balance for pool {0} not found")]
    PoolBalanceNotFound(PoolId),
    #[error("PoS accounting error: `{0}`")]
    PoSAccountingError(#[from] pos_accounting::Error),
}
