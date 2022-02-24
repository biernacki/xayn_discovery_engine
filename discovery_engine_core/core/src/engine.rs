// Copyright 2021 Xayn AG
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

use std::{collections::HashMap, sync::Arc};

use displaydoc::Display;
use figment::{
    providers::{Format, Json, Serialized},
    Figment,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;

use xayn_ai::{
    ranker::{AveragePooler, Builder, CoiSystemConfig},
    KpeConfig,
    SMBertConfig,
};
use xayn_discovery_engine_providers::Market;

use crate::{
    document::{self, document_from_article, Document, TimeSpent, UserReacted},
    mab::{self, BetaSampler, SelectionIter},
    ranker::Ranker,
    stack::{
        self,
        BoxedOps,
        BreakingNews,
        Data as StackData,
        Id as StackId,
        PersonalizedNews,
        Stack,
    },
};

/// Discovery engine errors.
#[derive(Error, Debug, Display)]
pub enum Error {
    /// Failed to serialize internal state of the engine: {0}.
    Serialization(#[source] GenericError),

    /// Failed to deserialize internal state to create the engine: {0}.
    Deserialization(#[source] bincode::Error),

    /// No operations on stack were provided.
    NoStackOps,

    /// Invalid stack: {0}.
    InvalidStack(#[source] stack::Error),

    /// Invalid stack id: {0}.
    InvalidStackId(StackId),

    /// An operation on a stack failed: {0}.
    StackOpFailed(#[source] stack::Error),

    /// Error while selecting the documents to return: {0}.
    Selection(#[from] mab::Error),

    /// Error while using the ranker: {0}
    Ranker(#[from] GenericError),

    /// Error while creating document: {0}.
    Document(#[source] document::Error),

    /// List of errors/warnings. {0:?}
    Errors(Vec<Error>),
}

/// Configuration settings to initialize Discovery Engine with a [`xayn_ai::ranker::Ranker`].
pub struct InitConfig {
    /// Key for accessing the API.
    pub api_key: String,
    /// API base url.
    pub api_base_url: String,
    /// List of markets to use.
    pub markets: Vec<Market>,
    /// S-mBert vocabulary path.
    pub smbert_vocab: String,
    /// S-mBert model path.
    pub smbert_model: String,
    /// KPE vocabulary path.
    pub kpe_vocab: String,
    /// KPE model path.
    pub kpe_model: String,
    /// KPE CNN path.
    pub kpe_cnn: String,
    /// KPR classifier path.
    pub kpe_classifier: String,
}

/// Discovery Engine endpoint settings.
pub struct EndpointConfig {
    /// Key for accessing API.
    pub(crate) api_key: String,
    /// Base URL for API.
    pub(crate) api_base_url: String,
    /// Write-exclusive access to markets list.
    pub(crate) markets: Arc<RwLock<Vec<Market>>>,
}

impl From<InitConfig> for EndpointConfig {
    fn from(config: InitConfig) -> Self {
        Self {
            api_key: config.api_key,
            api_base_url: config.api_base_url,
            markets: Arc::new(RwLock::new(config.markets)),
        }
    }
}

/// Temporary config to allow for configurations within the core without a mirroring outside impl.
struct CoreConfig {
    /// The number of selected top key phrases while updating the stacks.
    select_top: usize,
    /// The number of top documents per stack to keep while filtering the stacks.
    keep_top: usize,
    /// The lower bound of documents per stack at which new items are requested.
    request_new: usize,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            select_top: 3,
            keep_top: 20,
            request_new: 3,
        }
    }
}

/// Discovery Engine.
pub struct Engine<R> {
    config: EndpointConfig,
    core_config: CoreConfig,
    stacks: RwLock<HashMap<StackId, Stack>>,
    ranker: R,
}

impl<R> Engine<R>
where
    R: Ranker + Send + Sync,
{
    /// Creates a new `Engine`.
    async fn new(
        config: EndpointConfig,
        ranker: R,
        stack_ops: Vec<BoxedOps>,
    ) -> Result<Self, Error> {
        let stack_data = |_| StackData::default();

        Self::from_stack_data(config, ranker, stack_data, stack_ops).await
    }

    /// Creates a new `Engine` from serialized state and stack operations.
    ///
    /// The `Engine` only keeps in its state data related to the current [`BoxedOps`].
    /// Data related to missing operations will be dropped.
    async fn from_state(
        state: &StackState,
        config: EndpointConfig,
        ranker: R,
        stack_ops: Vec<BoxedOps>,
    ) -> Result<Self, Error> {
        if stack_ops.is_empty() {
            return Err(Error::NoStackOps);
        }

        let mut stack_data = bincode::deserialize::<HashMap<StackId, _>>(&state.0)
            .map_err(Error::Deserialization)?;
        let stack_data = |id| stack_data.remove(&id).unwrap_or_default();

        Self::from_stack_data(config, ranker, stack_data, stack_ops).await
    }

    async fn from_stack_data(
        config: EndpointConfig,
        ranker: R,
        mut stack_data: impl FnMut(StackId) -> StackData + Send,
        stack_ops: Vec<BoxedOps>,
    ) -> Result<Self, Error> {
        let stacks = stack_ops
            .into_iter()
            .map(|mut ops| {
                let id = ops.id();
                let data = stack_data(id);
                ops.configure(&config);
                Stack::new(data, ops).map(|stack| (id, stack))
            })
            .collect::<Result<_, _>>()
            .map(RwLock::new)
            .map_err(Error::InvalidStack)?;
        let core_config = CoreConfig::default();

        let mut engine = Self {
            config,
            core_config,
            stacks,
            ranker,
        };

        // we don't want to fail initialization if there are network problems
        drop(engine.update_stacks(usize::MAX).await);

        Ok(engine)
    }

    /// Serializes the state of the `Engine` and `Ranker` state.
    pub async fn serialize(&self) -> Result<Vec<u8>, Error> {
        let stacks = self.stacks.read().await;
        let stacks_data = stacks
            .iter()
            .map(|(id, stack)| (id, &stack.data))
            .collect::<HashMap<_, _>>();

        let engine = bincode::serialize(&stacks_data)
            .map(StackState)
            .map_err(|err| Error::Serialization(err.into()))?;

        let ranker = self
            .ranker
            .serialize()
            .map(RankerState)
            .map_err(Error::Serialization)?;

        let state_data = State { engine, ranker };

        bincode::serialize(&state_data).map_err(|err| Error::Serialization(err.into()))
    }

    /// Updates the markets configuration.
    ///
    /// Also resets and updates all stacks.
    pub async fn set_markets(&mut self, markets: Vec<Market>) -> Result<(), Error> {
        *self.config.markets.write().await = markets;

        for stack in self.stacks.write().await.values_mut() {
            stack.data = StackData::default();
        }
        self.update_stacks(self.core_config.request_new).await
    }

    /// Returns at most `max_documents` [`Document`]s for the feed.
    pub async fn get_feed_documents(
        &mut self,
        max_documents: usize,
    ) -> Result<Vec<Document>, Error> {
        let documents = SelectionIter::new(BetaSampler, self.stacks.write().await.values_mut())
            .select(max_documents)?;
        self.update_stacks(self.core_config.request_new).await?;

        Ok(documents)
    }

    /// Process the feedback about the user spending some time on a document.
    pub async fn time_spent(&mut self, time_spent: &TimeSpent) -> Result<(), Error> {
        self.ranker.log_document_view_time(time_spent)?;

        rank_stacks(self.stacks.write().await.values_mut(), &mut self.ranker)
    }

    /// Process the feedback about the user reacting to a document.
    pub async fn user_reacted(&mut self, reacted: &UserReacted) -> Result<(), Error> {
        let mut stacks = self.stacks.write().await;
        stacks
            .get_mut(&reacted.stack_id)
            .ok_or(Error::InvalidStackId(reacted.stack_id))?
            .update_relevance(reacted.reaction);

        self.ranker.log_user_reaction(reacted)?;

        rank_stacks(stacks.values_mut(), &mut self.ranker)
    }

    /// Updates the stacks with data related to the top key phrases of the current data.
    ///
    /// Requires a threshold below which new items will be requested for a stack.
    async fn update_stacks(&mut self, request_new: usize) -> Result<(), Error> {
        let key_phrases = &self
            .ranker
            .select_top_key_phrases(self.core_config.select_top);

        let mut errors = Vec::new();
        for stack in self.stacks.write().await.values_mut() {
            if stack.len() <= request_new {
                let articles = stack
                    .new_items(key_phrases)
                    .await
                    .and_then(|articles| stack.filter_articles(articles));

                match articles.map_err(Error::StackOpFailed).and_then(|articles| {
                    let id = stack.id();
                    articles
                        .into_iter()
                        .map(|article| {
                            let title = article.title.as_str();
                            let embedding =
                                self.ranker.compute_smbert(title).map_err(Error::Ranker)?;
                            document_from_article(article, id, embedding).map_err(Error::Document)
                        })
                        .collect::<Result<Vec<_>, _>>()
                }) {
                    Ok(documents) => {
                        if let Err(error) = stack.update(&documents, &mut self.ranker) {
                            errors.push(Error::StackOpFailed(error));
                        } else {
                            stack.data.retain_top(self.core_config.keep_top);
                        }
                    }
                    Err(error) => errors.push(error),
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(Error::Errors(errors))
        }
    }
}

/// The ranker could rank the documents in a different order so we update the stacks with it.
fn rank_stacks<'a>(
    stacks: impl Iterator<Item = &'a mut Stack>,
    ranker: &mut impl Ranker,
) -> Result<(), Error> {
    let errors = stacks.fold(Vec::new(), |mut errors, stack| {
        if let Err(error) = stack.rank(ranker) {
            errors.push(Error::StackOpFailed(error));
        }

        errors
    });

    if errors.is_empty() {
        Ok(())
    } else {
        Err(Error::Errors(errors))
    }
}

/// A discovery engine with [`xayn_ai::ranker::Ranker`] as a ranker.
pub type XaynAiEngine = Engine<xayn_ai::ranker::Ranker>;

impl XaynAiEngine {
    /// Creates a discovery engine with [`xayn_ai::ranker::Ranker`] as a ranker.
    pub async fn from_config(config: InitConfig, state: Option<&[u8]>) -> Result<Self, Error> {
        // TODO: TY-2449
        let ai_config = ai_config_from_json("{}");

        let smbert_config = SMBertConfig::from_files(&config.smbert_vocab, &config.smbert_model)
            .map_err(|err| Error::Ranker(err.into()))?
            .with_token_size(
                ai_config
                    .extract_inner("smbert.token_size")
                    .map_err(|err| Error::Ranker(err.into()))?,
            )
            .map_err(|err| Error::Ranker(err.into()))?
            .with_accents(false)
            .with_lowercase(true)
            .with_pooling(AveragePooler);

        let kpe_config = KpeConfig::from_files(
            &config.kpe_vocab,
            &config.kpe_model,
            &config.kpe_cnn,
            &config.kpe_classifier,
        )
        .map_err(|err| Error::Ranker(err.into()))?
        .with_token_size(
            ai_config
                .extract_inner("kpe.token_size")
                .map_err(|err| Error::Ranker(err.into()))?,
        )
        .map_err(|err| Error::Ranker(err.into()))?
        .with_accents(false)
        .with_lowercase(false);

        let coi_system_config = ai_config
            .extract()
            .map_err(|err| Error::Ranker(err.into()))?;

        let builder =
            Builder::from(smbert_config, kpe_config).with_coi_system_config(coi_system_config);

        let stack_ops = vec![
            Box::new(BreakingNews::default()) as BoxedOps,
            Box::new(PersonalizedNews::default()) as BoxedOps,
        ];

        if let Some(state) = state {
            let state: State = bincode::deserialize(state).map_err(Error::Deserialization)?;
            let ranker = builder
                .with_serialized_state(&state.ranker.0)
                .map_err(|err| Error::Ranker(err.into()))?
                .build()
                .map_err(|err| Error::Ranker(err.into()))?;
            Self::from_state(&state.engine, config.into(), ranker, stack_ops).await
        } else {
            let ranker = builder.build().map_err(|err| Error::Ranker(err.into()))?;
            Self::new(config.into(), ranker, stack_ops).await
        }
    }
}

fn ai_config_from_json(json: &str) -> Figment {
    Figment::new()
        .merge(Serialized::defaults(CoiSystemConfig::default()))
        .merge(Serialized::default("kpe.token_size", 150))
        .merge(Serialized::default("smbert.token_size", 52))
        .merge(Json::string(json))
}

/// A wrapper around a dynamic error type, similar to `anyhow::Error`,
/// but without the need to declare `anyhow` as a dependency.
pub(crate) type GenericError = Box<dyn std::error::Error + Sync + Send + 'static>;

#[derive(Serialize, Deserialize)]
struct StackState(Vec<u8>);

#[derive(Serialize, Deserialize)]
struct RankerState(Vec<u8>);

#[derive(Serialize, Deserialize)]
struct State {
    /// The serialized engine state.
    engine: StackState,
    /// The serialized ranker state.
    ranker: RankerState,
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;

    #[test]
    fn test_ai_config_from_json_default() -> Result<(), Box<dyn Error>> {
        let ai_config = ai_config_from_json("{}");
        assert_eq!(ai_config.extract_inner::<usize>("kpe.token_size")?, 150);
        assert_eq!(ai_config.extract_inner::<usize>("smbert.token_size")?, 52);
        assert_eq!(
            ai_config.extract::<CoiSystemConfig>()?,
            CoiSystemConfig::default(),
        );
        Ok(())
    }

    #[test]
    fn test_ai_config_from_json_modified() -> Result<(), Box<dyn Error>> {
        let ai_config = ai_config_from_json(
            r#"{
                "coi": {
                    "threshold": 0.42
                },
                "kpe": {
                    "penalty": [0.99, 0.66, 0.33]
                },
                "smbert": {
                    "token_size": 42,
                    "foo": "bar"
                },
                "baz": 0
            }"#,
        );
        assert_eq!(ai_config.extract_inner::<usize>("kpe.token_size")?, 150);
        assert_eq!(ai_config.extract_inner::<usize>("smbert.token_size")?, 42);
        assert_eq!(
            ai_config.extract::<CoiSystemConfig>()?,
            CoiSystemConfig::default()
                .with_threshold(0.42)?
                .with_penalty(&[0.99, 0.66, 0.33])?,
        );
        Ok(())
    }
}
