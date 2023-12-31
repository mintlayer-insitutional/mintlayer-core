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

use std::io::Write;

pub use log;

pub fn is_only_terminal_output_logging() -> bool {
    true
}

pub fn is_file_output_supported() -> bool {
    false
}

static INITIALIZE_LOGGER_ONCE_FLAG: std::sync::Once = std::sync::Once::new();

pub fn init_logging<P: AsRef<std::path::Path>>(_log_file_path: Option<P>) {
    INITIALIZE_LOGGER_ONCE_FLAG.call_once(env_logger::init);
}

/// Send log output to the specified [Write] instance, log lines are separated by '\n'
pub fn init_logging_pipe(file: impl Write + Send + 'static) {
    INITIALIZE_LOGGER_ONCE_FLAG.call_once(|| {
        env_logger::builder().target(env_logger::Target::Pipe(Box::new(file))).init()
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_twice() {
        init_logging::<&std::path::Path>(None);
        init_logging::<&std::path::Path>(None);
    }
}
