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

//! Center of interests module.

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

pub mod config;
pub(crate) mod context;
mod document;
pub mod embedding;
pub mod id;
pub mod point;
pub mod stats;
pub mod system;
pub mod utils;

#[cfg(doc)]
pub use crate::embedding::COSINE_SIMILARITY_RANGE;
pub use crate::{
    config::{Config as CoiConfig, Error as CoiConfigError},
    context::Error as CoiContextError,
    document::Document,
    embedding::{
        cosine_similarity,
        pairwise_cosine_similarity,
        Embedding,
        MalformedBytesEmbedding,
    },
    id::CoiId,
    point::{CoiPoint, NegativeCoi, PositiveCoi, UserInterests},
    stats::{compute_coi_relevances, CoiStats},
    system::System as CoiSystem,
    utils::{nan_safe_f32_cmp, nan_safe_f32_cmp_desc, system_time_now},
};