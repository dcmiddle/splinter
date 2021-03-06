// Copyright 2018-2020 Cargill Incorporated
// Copyright 2018 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

pub mod admin;
mod api;
pub mod certs;
pub mod circuit;
#[cfg(feature = "database")]
pub mod database;
#[cfg(feature = "health")]
pub mod health;
pub mod keygen;
pub mod registry;

use std::collections::HashMap;
use std::ffi::CString;
use std::fs::File;
use std::io::{Error as IoError, ErrorKind, Read};
use std::path::Path;

use clap::ArgMatches;
#[cfg(feature = "splinter-cli-jwt")]
use cylinder::{jwt::JsonWebTokenBuilder, load_user_key, secp256k1::Secp256k1Context, Context};

use super::error::CliError;

const DEFAULT_SPLINTER_REST_API_URL: &str = "http://127.0.0.1:8080";
const SPLINTER_REST_API_URL_ENV: &str = "SPLINTER_REST_API_URL";

/// A CLI Command Action.
///
/// An Action is a single subcommand for CLI operations.
pub trait Action {
    /// Run a CLI Action with the given args
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError>;
}

/// A collection of Subcommands associated with a single parent command.
#[derive(Default)]
pub struct SubcommandActions<'a> {
    actions: HashMap<String, Box<dyn Action + 'a>>,
}

impl<'a> SubcommandActions<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_command<'action: 'a, A: Action + 'action>(
        mut self,
        command: &str,
        action: A,
    ) -> Self {
        self.actions.insert(command.to_string(), Box::new(action));

        self
    }
}

impl<'s> Action for SubcommandActions<'s> {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let args = arg_matches.ok_or(CliError::RequiresArgs)?;

        let (subcommand, args) = args.subcommand();

        if let Some(action) = self.actions.get_mut(subcommand) {
            action.run(args)
        } else {
            Err(CliError::InvalidSubcommand)
        }
    }
}

fn chown(path: &Path, uid: u32, gid: u32) -> Result<(), CliError> {
    let pathstr = path
        .to_str()
        .ok_or_else(|| CliError::EnvironmentError(format!("Invalid path: {:?}", path)))?;
    let cpath =
        CString::new(pathstr).map_err(|err| CliError::EnvironmentError(format!("{}", err)))?;
    let result = unsafe { libc::chown(cpath.as_ptr(), uid, gid) };
    match result {
        0 => Ok(()),
        code => Err(CliError::EnvironmentError(format!(
            "Error chowning file {}: {}",
            pathstr, code
        ))),
    }
}

/// Reads a private key from the given file name.
fn read_private_key(file_name: &str) -> Result<String, CliError> {
    let mut file = File::open(file_name).map_err(|err| {
        CliError::EnvironmentError(format!(
            "Unable to open key file '{}': {}",
            file_name,
            msg_from_io_error(err)
        ))
    })?;

    let mut buf = String::new();
    file.read_to_string(&mut buf).map_err(|err| {
        CliError::EnvironmentError(format!(
            "Unable to read key file '{}': {}",
            file_name,
            msg_from_io_error(err)
        ))
    })?;
    let key = buf.trim().to_string();

    Ok(key)
}

fn msg_from_io_error(err: IoError) -> String {
    match err.kind() {
        ErrorKind::NotFound => "File not found".into(),
        ErrorKind::PermissionDenied => "Permission denied".into(),
        ErrorKind::InvalidData => "Invalid data".into(),
        _ => "Unknown I/O error".into(),
    }
}

#[cfg(feature = "splinter-cli-jwt")]
// build a signed json web token using the private key
fn create_cylinder_jwt_auth(key_name: Option<&str>) -> Result<String, CliError> {
    let default_key_path = dirs::home_dir()
        .map(|mut p| {
            p.push(".splinter/keys");
            p
        })
        .ok_or_else(|| CliError::EnvironmentError("Home directory not found".into()))?;
    let default_path_string = {
        if let Some(path) = default_key_path.to_str() {
            path
        } else {
            return Err(CliError::ActionError(
                "Path is not valid unicode".to_string(),
            ));
        }
    };

    let private_key = load_user_key(key_name, default_path_string).map_err(|err| {
        CliError::ActionError(format!("Unable to get private key from file: {}", err))
    })?;

    let context = Secp256k1Context::new();
    let signer = context.new_signer(private_key);

    let encoded_token = JsonWebTokenBuilder::new()
        .build(&*signer)
        .map_err(|err| CliError::ActionError(format!("failed to build json web token: {}", err)))?;

    Ok(format!("Bearer Cylinder:{}", encoded_token))
}
