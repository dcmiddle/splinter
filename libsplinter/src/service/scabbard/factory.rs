// Copyright 2019 Cargill Incorporated
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

use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;
use std::path::Path;

use serde_json;

use crate::service::{FactoryCreateError, Service, ServiceFactory};

use super::{Scabbard, SERVICE_TYPE};

const DEFAULT_DB_DIR: &str = "/var/lib/splinter";
const DEFAULT_DB_SIZE: usize = 1028 * 1028 * 1028;

pub struct ScabbardFactory {
    service_types: Vec<String>,
    db_dir: String,
    db_size: usize,
}

impl ScabbardFactory {
    pub fn new(db_dir: Option<String>, db_size: Option<usize>) -> Self {
        ScabbardFactory {
            service_types: vec![SERVICE_TYPE.into()],
            db_dir: db_dir.unwrap_or_else(|| DEFAULT_DB_DIR.into()),
            db_size: db_size.unwrap_or(DEFAULT_DB_SIZE),
        }
    }
}

impl ServiceFactory for ScabbardFactory {
    fn available_service_types(&self) -> &[String] {
        self.service_types.as_slice()
    }

    /// `args` should be the list of public keys that are allowed to create and modify sabre
    /// contracts, formatted as a serialized JSON array of strings.
    fn create(
        &self,
        service_id: String,
        _service_type: &str,
        peer_services: Vec<String>,
        args: HashMap<String, String>,
    ) -> Result<Box<dyn Service>, FactoryCreateError> {
        let initial_peers = HashSet::from_iter(peer_services.into_iter());
        let db_dir = Path::new(&self.db_dir);
        let admin_keys_str = args.get("admin_keys").ok_or_else(|| {
            FactoryCreateError::InvalidArguments("admin_keys argument not specified".into())
        })?;
        let admin_keys = serde_json::from_str(admin_keys_str).map_err(|err| {
            FactoryCreateError::InvalidArguments(format!(
                "failed to parse admin_keys list: {}",
                err,
            ))
        })?;

        let service = Scabbard::new(service_id, initial_peers, &db_dir, self.db_size, admin_keys)
            .map_err(|err| FactoryCreateError::CreationFailed(Box::new(err)))?;

        Ok(Box::new(service))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scabbard_factory() {
        let admin_keys: Vec<String> = vec![];
        let mut args = HashMap::new();
        args.insert(
            "admin_keys".into(),
            serde_json::to_string(&admin_keys).expect("failed to serialize admin_keys"),
        );

        let factory = ScabbardFactory::new(Some("/tmp".into()), Some(1024 * 1024));
        let service = factory
            .create(
                "0".into(),
                "",
                vec!["1".into(), "2".into(), "3".into()],
                args,
            )
            .expect("failed to create service");

        assert_eq!(service.service_id(), "0");
    }
}
