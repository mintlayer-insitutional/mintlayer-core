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

use super::storage::{
    DnsServerStorage, DnsServerStorageRead, DnsServerStorageWrite, DnsServerTransactionRo,
    DnsServerTransactionRw, DnsServerTransactional,
};
use serialization::{encoded::Encoded, DecodeAll, Encode};

type ValueId = u32;

storage::decl_schema! {
    /// Database schema for peer db storage
    pub Schema {
        /// Storage for individual values
        pub DBValue: Map<ValueId, Vec<u8>>,

        /// Table for all reachable addresses
        pub DBAddresses: Map<String, ()>,
    }
}

const VALUE_ID_VERSION: ValueId = 1;

pub struct DnsServerStoreTxRo<'st, B: storage::Backend>(storage::TransactionRo<'st, B, Schema>);

pub struct DnsServerStoreTxRw<'st, B: storage::Backend>(storage::TransactionRw<'st, B, Schema>);

impl<'tx, B: storage::Backend + 'tx> DnsServerTransactional<'tx> for DnsServerStorageImpl<B> {
    type TransactionRo = DnsServerStoreTxRo<'tx, B>;
    type TransactionRw = DnsServerStoreTxRw<'tx, B>;

    fn transaction_ro<'st: 'tx>(&'st self) -> Result<Self::TransactionRo, storage::Error> {
        self.0.transaction_ro().map(DnsServerStoreTxRo)
    }

    fn transaction_rw<'st: 'tx>(&'st self) -> Result<Self::TransactionRw, storage::Error> {
        self.0.transaction_rw(None).map(DnsServerStoreTxRw)
    }
}

impl<B: storage::Backend + 'static> DnsServerStorage for DnsServerStorageImpl<B> {}

pub struct DnsServerStorageImpl<T: storage::Backend>(storage::Storage<T, Schema>);

impl<B: storage::Backend> DnsServerStorageImpl<B> {
    pub fn new(storage: B) -> Result<Self, storage::Error> {
        let store = storage::Storage::<_, Schema>::new(storage)?;
        Ok(Self(store))
    }
}

impl<'st, B: storage::Backend> DnsServerStorageWrite for DnsServerStoreTxRw<'st, B> {
    fn set_version(&mut self, version: u32) -> Result<(), storage::Error> {
        self.0.get_mut::<DBValue, _>().put(VALUE_ID_VERSION, version.encode())
    }

    fn add_address(&mut self, address: &str) -> Result<(), storage::Error> {
        self.0.get_mut::<DBAddresses, _>().put(address, ())
    }

    fn del_address(&mut self, address: &str) -> Result<(), storage::Error> {
        self.0.get_mut::<DBAddresses, _>().del(address)
    }
}

impl<'st, B: storage::Backend> DnsServerTransactionRw for DnsServerStoreTxRw<'st, B> {
    fn abort(self) {
        self.0.abort()
    }

    fn commit(self) -> Result<(), storage::Error> {
        self.0.commit()
    }
}

impl<'st, B: storage::Backend> DnsServerStorageRead for DnsServerStoreTxRo<'st, B> {
    fn get_version(&self) -> Result<Option<u32>, storage::Error> {
        let map = self.0.get::<DBValue, _>();
        let vec_opt = map.get(VALUE_ID_VERSION)?.as_ref().map(Encoded::decode);
        Ok(vec_opt.map(|vec| {
            u32::decode_all(&mut vec.as_ref()).expect("db values to be encoded correctly")
        }))
    }

    fn get_addresses(&self) -> Result<Vec<String>, storage::Error> {
        let map = self.0.get::<DBAddresses, _>();
        let iter = map.prefix_iter_decoded(&())?.map(|(addr, ())| addr);
        Ok(iter.collect::<Vec<_>>())
    }
}

impl<'st, B: storage::Backend> DnsServerTransactionRo for DnsServerStoreTxRo<'st, B> {
    fn close(self) {
        self.0.close()
    }
}
