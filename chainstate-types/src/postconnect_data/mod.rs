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

use std::collections::BTreeMap;

use common::primitives::H256;

use self::pool_operation::PoolOperation;

pub mod error;
pub mod pool_operation;

pub struct BlockPostconnectDataDelta {
    ops: BTreeMap<H256, PoolOperation>,
}

impl BlockPostconnectDataDelta {
    pub fn new() -> Self {
        Self {
            ops: BTreeMap::new(),
        }
    }

    pub fn incorporate_operation(
        &mut self,
        staker_id: H256,
        pool_operation: PoolOperation,
    ) -> Result<(), error::Error> {
        let entry = self.ops.remove(&staker_id);
        let new_entry = match entry {
            Some(op) => op.incorporate(pool_operation)?,
            None => pool_operation,
        };
        self.ops.insert(staker_id, new_entry);

        Ok(())
    }
}
