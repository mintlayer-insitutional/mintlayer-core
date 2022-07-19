// Copyright (c) 2022 RBB S.r.l
// opensource@mintlayer.org
// SPDX-License-Identifier: MIT
// Licensed under the MIT License;
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://spdx.org/licenses/MIT
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! The node command line options.

use std::{ffi::OsString, fs, net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use directories::UserDirs;
use strum::VariantNames;

use common::chain::config::ChainType;

const CONFIG_NAME: &str = "config.toml";

/// Mintlayer node executable
#[derive(Parser, Debug)]
#[clap(author, version, about)]
pub struct Options {
    /// Where to write logs
    #[clap(long, value_name = "PATH")]
    pub log_path: Option<PathBuf>,

    /// The path to the data directory.
    #[clap(short, long, default_value_os_t = default_data_dir())]
    data_dir: PathBuf,

    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Create a configuration file.
    CreateConfig,
    Run(RunOptions),
}

/// Run the node.
#[derive(Args, Debug)]
pub struct RunOptions {
    /// Blockchain type.
    #[clap(long, possible_values = ChainType::VARIANTS, default_value = "mainnet")]
    pub net: ChainType,

    /// The number of maximum attempts to process a block.
    #[clap(long)]
    pub max_db_commit_attempts: Option<usize>,

    /// The maximum capacity of the orphan blocks pool in blocks.
    #[clap(long)]
    pub max_orphan_blocks: Option<usize>,

    /// Address to bind P2P to.
    #[clap(long, value_name = "ADDR")]
    pub p2p_addr: Option<String>,

    /// The p2p score threshold after which a peer is baned.
    #[clap(long)]
    pub p2p_ban_threshold: Option<u32>,

    /// The p2p timeout value in seconds.
    #[clap(long)]
    pub p2p_outbound_connection_timeout: Option<u64>,

    /// Address to bind RPC to.
    #[clap(long, value_name = "ADDR")]
    pub rpc_addr: Option<SocketAddr>,
}

impl Options {
    /// Constructs an instance by parsing the given arguments.
    ///
    /// The data directory is created as a side-effect of the invocation.
    pub fn from_args<A: Into<OsString> + Clone>(args: impl IntoIterator<Item = A>) -> Result<Self> {
        let options: Options = clap::Parser::parse_from(args);

        // We want to check earlier if the directory can be created.
        fs::create_dir_all(&options.data_dir).with_context(|| {
            format!(
                "Failed to create the '{:?}' data directory",
                options.data_dir
            )
        })?;

        Ok(options)
    }

    /// Returns a path to the config file.
    pub fn config_path(&self) -> PathBuf {
        self.data_dir.join(CONFIG_NAME)
    }
}

fn default_data_dir() -> PathBuf {
    UserDirs::new()
        // Expect here is OK because `Parser::parse_from` panics anyway in case of error.
        .expect("Unable to get home directory")
        .home_dir()
        .join(".mintlayer")
}
