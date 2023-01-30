// Copyright 2022 Xayn AG
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, version 3.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

mod routes;

use actix_web::web::ServiceConfig;
use derive_more::AsRef;
use serde::{Deserialize, Serialize};

use crate::{
    embedding::{self, Embedder},
    logging,
    server::{self, Application, NetConfig},
    storage::{self, Storage},
};

pub struct Ingestion;

impl Application for Ingestion {
    type Config = Config;
    type AppStateExtension = AppStateExtension;

    fn configure_service(config: &mut ServiceConfig) {
        routes::configure_service(config);
    }

    fn create_app_state_extension(
        config: &Self::Config,
    ) -> Result<Self::AppStateExtension, server::SetupError> {
        Ok(AppStateExtension {
            embedder: Embedder::load(&config.embedding)?,
        })
    }
}

type AppState = server::AppState<
    <Ingestion as Application>::Config,
    <Ingestion as Application>::AppStateExtension,
    Storage,
>;

#[derive(AsRef, Debug, Default, Deserialize, Serialize)]
pub struct Config {
    #[as_ref]
    #[serde(default)]
    pub(crate) logging: logging::Config,

    #[as_ref]
    #[serde(default)]
    pub(crate) net: NetConfig,

    #[as_ref]
    #[serde(default)]
    pub(crate) storage: storage::Config,

    #[as_ref]
    #[serde(default)]
    pub(crate) ingestion: IngestionConfig,

    #[as_ref]
    #[serde(default)]
    pub(crate) embedding: embedding::Config,
}

server::impl_config! { Config }

#[derive(AsRef, Debug, Deserialize, Serialize)]
pub struct IngestionConfig {
    #[as_ref]
    #[serde(default = "default_max_document_batch_size")]
    pub(crate) max_document_batch_size: usize,
}

impl Default for IngestionConfig {
    fn default() -> Self {
        Self {
            max_document_batch_size: default_max_document_batch_size(),
        }
    }
}

const fn default_max_document_batch_size() -> usize {
    100
}

#[derive(AsRef)]
pub struct AppStateExtension {
    #[as_ref]
    pub(crate) embedder: Embedder,
}
