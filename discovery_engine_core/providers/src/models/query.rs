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

use crate::utils::filter::{Filter, Market};

/// Page rank limiting strategy.
pub enum RankLimit {
    LimitedByMarket,
    Unlimited,
}

/// Parameters determining which news to search.
///
/// Note that, depending on the provider we're fetching from, some of these parameters
/// may not be supported.
pub struct SearchQuery<'a> {
    /// Market of news.
    pub market: &'a Market,

    /// How many articles to return (per page).
    pub page_size: usize,

    /// The number of the page which should be returned.
    /// Paging starts with `1`.
    pub page: usize,

    /// Page rank limiting strategy.
    pub rank_limit: RankLimit,

    /// Exclude given sources.
    pub excluded_sources: &'a [String],

    /// News filter.
    pub filter: &'a Filter,

    /// Maximum age of news items we want to include in the results
    pub max_age_days: Option<usize>,
}

/// Parameters determining which headlines to fetch.
///
/// Note that, depending on the provider we're fetching from, some of these parameters
/// may not be supported.
pub struct HeadlinesQuery<'a> {
    /// Market of news.
    pub market: &'a Market,

    /// How many articles to return (per page).
    pub page_size: usize,

    /// The number of the page which should be returned.
    /// Paging starts with `1`.
    pub page: usize,

    /// Page rank limiting strategy.
    pub rank_limit: RankLimit,

    /// Exclude given sources.
    pub excluded_sources: &'a [String],

    /// Trusted sources.
    pub trusted_sources: &'a [String],

    /// Headlines topic.
    pub topic: Option<&'a str>,

    /// Maximum age of news items we want to include in the results.
    pub max_age_days: Option<usize>,
}

/// Parameters determining which headlines from trusted sources to fetch.
///
/// Fields not supported by the used provider will be ignored.
pub struct TrustedHeadlinesQuery<'a> {
    /// Market of news.
    pub market: Option<&'a Market>,

    /// How many articles to return (per page).
    pub page_size: usize,

    /// The number of the page which should be returned.
    /// Paging starts with `1`.
    pub page: usize,

    /// Page rank limiting strategy.
    pub rank_limit: RankLimit,

    /// Exclude given sources.
    pub excluded_sources: &'a [String],

    /// Prefer trusted sources.
    pub trusted_sources: &'a [String],

    /// Maximum age of news items we want to include in the results.
    pub max_age_days: Option<usize>,
}

/// Parameters determining similar news to search.
pub struct SimilarSearchQuery<'a> {
    /// Query text.
    pub like: &'a str,

    /// Market of news.
    pub market: &'a Market,

    /// How many articles to return (per page).
    pub page_size: usize,

    /// The number of the page which should be returned.
    /// Paging starts with `1`.
    pub page: usize,

    /// Page rank limiting strategy.
    pub rank_limit: RankLimit,

    /// Exclude given sources.
    pub excluded_sources: &'a [String],

    /// Maximum age of news items we want to include in the results
    pub max_age_days: Option<usize>,
}
