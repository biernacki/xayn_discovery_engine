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

mod article;
mod deduplication;
pub mod semantic;
mod source;

pub(crate) use self::{
    article::{ArticleFilter, CommonFilter, MalformedFilter, SourcesFilter},
    deduplication::DuplicateFilter,
    semantic::{
        filter_semantically,
        filter_too_similar,
        max_cosine_similarity,
        Criterion,
        SemanticFilterConfig,
    },
    source::source_weight,
};