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

//! A test suite for p2p backends.

#![allow(clippy::unwrap_used)]

#[macro_use]
mod utils;
mod ban;
mod block_announcement;
mod connect;
mod events;

use std::fmt::Debug;

use libtest_mimic::{Arguments, Trial};

use p2p::{
    net::{ConnectivityService, MessagingService, NetworkingService, SyncingEventReceiver},
    testing_utils::{RandomAddressMaker, TestTransportMaker},
};

/// Runs all tests.
pub fn run<T, N, A>()
where
    T: TestTransportMaker<Transport = N::Transport, Address = N::Address>,
    N: NetworkingService + Debug + 'static,
    N::ConnectivityHandle: ConnectivityService<N> + Debug,
    N::MessagingHandle: MessagingService + Debug,
    N::SyncingEventReceiver: SyncingEventReceiver + Debug,
    A: RandomAddressMaker<Address = N::Address>,
{
    rlimit::increase_nofile_limit(10 * 1024).unwrap();
    logging::init_logging::<&str>(None);
    let args = Arguments::from_args();
    libtest_mimic::run(&args, tests::<T, N, A>()).exit();
}

/// Collects all backend agnostic tests.
fn tests<T, N, A>() -> Vec<Trial>
where
    T: TestTransportMaker<Transport = N::Transport, Address = N::Address>,
    N: NetworkingService + Debug + 'static,
    N::ConnectivityHandle: ConnectivityService<N> + Debug,
    N::MessagingHandle: MessagingService + Debug,
    N::SyncingEventReceiver: SyncingEventReceiver + Debug,
    A: RandomAddressMaker<Address = N::Address>,
{
    std::iter::empty()
        .chain(connect::tests::<T, N, A>())
        .chain(block_announcement::tests::<T, N, A>())
        .chain(ban::tests::<T, N, A>())
        .chain(events::tests::<T, N, A>())
        .collect()
}
