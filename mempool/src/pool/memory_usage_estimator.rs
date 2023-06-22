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

use super::MempoolStore;

pub trait MemoryUsageEstimator: Send + Sync + 'static {
    fn estimate_memory_usage(&self, store: &MempoolStore) -> usize;
}

/// Estimate memory usage by asking the mempool store
pub struct StoreMemoryUsageEstimator;

impl MemoryUsageEstimator for StoreMemoryUsageEstimator {
    fn estimate_memory_usage(&self, _store: &MempoolStore) -> usize {
        // TODO: Just a temporary value to emulate the original behavior. In order for eviction to
        // work properly, we need to get transaction verifier and mempool to agree on transaction
        // dependencies.
        0
    }
}