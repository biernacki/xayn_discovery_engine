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

//! FFI and logic bindings to `discovery_engine_core`.

#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(
    clippy::future_not_send,
    clippy::pedantic,
    noop_method_call,
    rust_2018_idioms,
    unsafe_code,
    unused_qualifications
)]
#![warn(unreachable_pub, rustdoc::missing_crate_level_docs)]
#![allow(
    clippy::items_after_statements,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate
)]

pub mod async_bindings;
mod tracing;
pub mod types;

use std::path::Path;

use itertools::Itertools;
use xayn_discovery_engine_core::Engine;

#[async_bindgen::api(
    use uuid::Uuid;

    use xayn_discovery_engine_core::{
        document::{TimeSpent, UserReacted},
        InitConfig,
    };
    use xayn_discovery_engine_providers::Market;

    use crate::types::{
        document::Document,
        engine::{InitializationResult, SharedEngine},
        search::Search,
        trending_topic::TrendingTopic,
    };
)]
impl XaynDiscoveryEngineAsyncFfi {
    /// Initializes the engine.
    pub async fn initialize(config: Box<InitConfig>) -> Box<Result<InitializationResult, String>> {
        tracing::init_tracing(config.log_file.as_deref().map(Path::new));

        Box::new(
            Engine::from_config(*config)
                .await
                .map(|(engine, init_db_hint)| InitializationResult::new(engine, init_db_hint))
                .map_err(|error| error.to_string()),
        )
    }

    /// Configures the running engine.
    pub async fn configure(engine: &SharedEngine, de_config: Box<String>) {
        engine.as_ref().lock().await.configure(&de_config);
    }

    /// Sets the markets.
    pub async fn set_markets(
        engine: &SharedEngine,
        markets: Box<Vec<Market>>,
    ) -> Box<Result<(), String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .set_markets(*markets)
                .await
                .map_err(|error| error.to_string()),
        )
    }

    /// Gets the next batch of feed documents.
    pub async fn feed_next_batch(engine: &SharedEngine) -> Box<Result<Vec<Document>, String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .feed_next_batch()
                .await
                .map(|documents| documents.into_iter().map_into().collect())
                .map_err(|error| error.to_string()),
        )
    }

    /// Restores the documents which have been fed, i.e. the current feed.
    pub async fn restore_feed(engine: &SharedEngine) -> Box<Result<Vec<Document>, String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .restore_feed()
                .await
                .map(|documents| documents.into_iter().map_into().collect())
                .map_err(|error| error.to_string()),
        )
    }

    /// Deletes the feed documents.
    pub async fn delete_feed_documents(
        engine: &SharedEngine,
        ids: Box<Vec<Uuid>>,
    ) -> Box<Result<(), String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .delete_feed_documents(&ids.into_iter().map_into().collect_vec())
                .await
                .map_err(|error| error.to_string()),
        )
    }

    /// Processes the user's time on a document.
    pub async fn time_spent(
        engine: &SharedEngine,
        time_spent: Box<TimeSpent>,
    ) -> Box<Result<(), String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .time_spent(*time_spent)
                .await
                .map_err(|error| error.to_string()),
        )
    }

    /// Processes the user's reaction to a document.
    pub async fn user_reacted(
        engine: &SharedEngine,
        reacted: Box<UserReacted>,
    ) -> Box<Result<Document, String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .user_reacted(*reacted)
                .await
                .map(Into::into)
                .map_err(|error| error.to_string()),
        )
    }

    /// Perform an active search by query.
    pub async fn search_by_query(
        engine: &SharedEngine,
        query: Box<String>,
        page: u32,
    ) -> Box<Result<Vec<Document>, String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .search_by_query(query.as_ref(), page)
                .await
                .map(|documents| documents.into_iter().map_into().collect())
                .map_err(|error| error.to_string()),
        )
    }

    /// Perform an active search by topic.
    pub async fn search_by_topic(
        engine: &SharedEngine,
        topic: Box<String>,
        page: u32,
    ) -> Box<Result<Vec<Document>, String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .search_by_topic(topic.as_ref(), page)
                .await
                .map(|documents| documents.into_iter().map_into().collect())
                .map_err(|error| error.to_string()),
        )
    }

    /// Performs an active search by document id (aka deep search).
    ///
    /// The documents are sorted in descending order wrt their cosine similarity towards the
    /// original search term embedding.
    pub async fn search_by_id(
        engine: &SharedEngine,
        id: Box<Uuid>,
    ) -> Box<Result<Vec<Document>, String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .search_by_id((*id).into())
                .await
                .map(|documents| documents.into_iter().map_into().collect())
                .map_err(|error| error.to_string()),
        )
    }

    /// Gets the next batch of the current active search.
    pub async fn search_next_batch(engine: &SharedEngine) -> Box<Result<Vec<Document>, String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .search_next_batch()
                .await
                .map(|documents| documents.into_iter().map_into().collect())
                .map_err(|error| error.to_string()),
        )
    }

    /// Restores the documents which have been searched, i.e. the current active search.
    pub async fn restore_search(engine: &SharedEngine) -> Box<Result<Vec<Document>, String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .restore_search()
                .await
                .map(|documents| documents.into_iter().map_into().collect())
                .map_err(|error| error.to_string()),
        )
    }

    /// Gets the current active search mode and term.
    pub async fn searched_by(engine: &SharedEngine) -> Box<Result<Search, String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .searched_by()
                .await
                .map(Into::into)
                .map_err(|error| error.to_string()),
        )
    }

    /// Closes the current active search.
    pub async fn close_search(engine: &SharedEngine) -> Box<Result<(), String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .close_search()
                .await
                .map_err(|error| error.to_string()),
        )
    }

    /// Returns the current trending topics.
    pub async fn trending_topics(engine: &SharedEngine) -> Box<Result<Vec<TrendingTopic>, String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .trending_topics()
                .await
                .map(|trending_topics| trending_topics.into_iter().map_into().collect())
                .map_err(|error| error.to_string()),
        )
    }

    /// Sets new trusted and excluded sources.
    pub async fn set_sources(
        engine: &SharedEngine,
        trusted: Box<Vec<String>>,
        excluded: Box<Vec<String>>,
    ) -> Box<Result<(), String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .set_sources(*trusted, *excluded)
                .await
                .map_err(|error| error.to_string()),
        )
    }

    /// Returns the trusted sources.
    pub async fn trusted_sources(engine: &SharedEngine) -> Box<Result<Vec<String>, String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .trusted_sources()
                .await
                .map_err(|error| error.to_string()),
        )
    }

    /// Returns the excluded sources.
    pub async fn excluded_sources(engine: &SharedEngine) -> Box<Result<Vec<String>, String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .excluded_sources()
                .await
                .map_err(|error| error.to_string()),
        )
    }

    /// Adds a trusted source.
    pub async fn add_trusted_source(
        engine: &SharedEngine,
        trusted: Box<String>,
    ) -> Box<Result<(), String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .add_trusted_source(*trusted)
                .await
                .map_err(|error| error.to_string()),
        )
    }

    /// Removes a trusted source.
    pub async fn remove_trusted_source(
        engine: &SharedEngine,
        trusted: Box<String>,
    ) -> Box<Result<(), String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .remove_trusted_source(*trusted)
                .await
                .map_err(|error| error.to_string()),
        )
    }

    /// Adds an excluded source.
    pub async fn add_excluded_source(
        engine: &SharedEngine,
        excluded: Box<String>,
    ) -> Box<Result<(), String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .add_excluded_source(*excluded)
                .await
                .map_err(|error| error.to_string()),
        )
    }

    /// Removes an excluded source.
    pub async fn remove_excluded_source(
        engine: &SharedEngine,
        excluded: Box<String>,
    ) -> Box<Result<(), String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .remove_excluded_source(*excluded)
                .await
                .map_err(|error| error.to_string()),
        )
    }

    /// Disposes the engine.
    pub async fn dispose(engine: Box<SharedEngine>) {
        drop(engine.as_ref().as_ref().lock().await);
    }

    /// Reset the AI state of this engine.
    pub async fn reset_ai(engine: &SharedEngine) -> Box<Result<(), String>> {
        Box::new(
            engine
                .as_ref()
                .lock()
                .await
                .reset_ai()
                .await
                .map_err(|error| error.to_string()),
        )
    }
}
