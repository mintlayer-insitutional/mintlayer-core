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

use common::{
    chain::{stakelock::StakePoolData, Destination},
    primitives::{per_thousand::PerThousand, Amount},
};
use crypto::vrf::VRFPublicKey;
use serialization::{Decode, Encode};

#[derive(Debug, Eq, PartialEq, Clone, Encode, Decode)]
pub struct PoolData {
    decommission_destination: Destination,
    pledge_amount: Amount,
    vrf_public_key: VRFPublicKey,
    margin_ratio_per_thousand: PerThousand,
    cost_per_block: Amount,
}

impl PoolData {
    pub fn new(
        decommission_destination: Destination,
        pledge_amount: Amount,
        vrf_public_key: VRFPublicKey,
        margin_ratio_per_thousand: PerThousand,
        cost_per_block: Amount,
    ) -> Self {
        Self {
            decommission_destination,
            pledge_amount,
            vrf_public_key,
            margin_ratio_per_thousand,
            cost_per_block,
        }
    }

    pub fn decommission_destination(&self) -> &Destination {
        &self.decommission_destination
    }

    pub fn pledge_amount(&self) -> Amount {
        self.pledge_amount
    }

    pub fn vrf_public_key(&self) -> &VRFPublicKey {
        &self.vrf_public_key
    }

    pub fn margin_ratio_per_thousand(&self) -> PerThousand {
        self.margin_ratio_per_thousand
    }

    pub fn cost_per_block(&self) -> Amount {
        self.cost_per_block
    }
}

impl From<StakePoolData> for PoolData {
    fn from(stake_data: StakePoolData) -> Self {
        Self {
            decommission_destination: stake_data.decommission_key().clone(),
            pledge_amount: stake_data.value(),
            vrf_public_key: stake_data.vrf_public_key().clone(),
            margin_ratio_per_thousand: stake_data.margin_ratio_per_thousand(),
            cost_per_block: stake_data.cost_per_block(),
        }
    }
}
