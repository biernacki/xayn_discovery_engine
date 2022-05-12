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

//! The Bert pipelines compute embeddings of sequences.
//!
//! Sequences are anything string-like and can also be single words or snippets. The embeddings are
//! f32-arrays and their shape depends on the pooling strategy.
//!
//! See the example in this crate for usage details.

#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(
    clippy::future_not_send,
    clippy::pedantic,
    noop_method_call,
    rust_2018_idioms,
    unsafe_code,
    unused_qualifications
)]
#![warn(missing_docs, unreachable_pub)]
#![allow(
    clippy::items_after_statements,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate
)]

mod config;
mod model;
mod pipeline;
mod pooler;
mod tokenizer;

pub use crate::{
    config::{Config, ConfigError},
    model::kinds,
    pipeline::{Pipeline, PipelineError},
    pooler::{AveragePooler, Embedding1, Embedding2, FirstPooler, NonePooler},
};

/// A sentence (embedding) multilingual Bert pipeline.
pub type SMBert = Pipeline<kinds::SMBert, AveragePooler>;

/// A sentence (embedding) multilingual Bert configuration.
pub type SMBertConfig<'a, P> = Config<'a, kinds::SMBert, P>;

/// A question answering (embedding) multilingual Bert pipeline.
pub type QAMBert = Pipeline<kinds::QAMBert, AveragePooler>;

/// A question answering (embedding) multilingual Bert configuration.
pub type QAMBertConfig<'a, P> = Config<'a, kinds::QAMBert, P>;

#[cfg(doc)]
pub use crate::{
    model::{BertModel, ModelError},
    pooler::{Embedding, PoolerError},
    tokenizer::TokenizerError,
};