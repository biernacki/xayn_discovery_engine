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

import 'dart:typed_data' show Uint8List;

import 'package:equatable/equatable.dart' show EquatableMixin;
import 'package:xayn_discovery_engine/src/domain/assets/data_provider.dart'
    show SetupData;
import 'package:xayn_discovery_engine/src/domain/models/active_data.dart'
    show DocumentWithActiveData;
import 'package:xayn_discovery_engine/src/domain/models/configuration.dart'
    show Configuration;
import 'package:xayn_discovery_engine/src/domain/models/feed_market.dart'
    show FeedMarket, FeedMarkets;
import 'package:xayn_discovery_engine/src/domain/models/history.dart'
    show HistoricDocument;
import 'package:xayn_discovery_engine/src/domain/models/source.dart'
    show Source;
import 'package:xayn_discovery_engine/src/domain/models/time_spent.dart'
    show TimeSpent;
import 'package:xayn_discovery_engine/src/domain/models/trending_topic.dart'
    show TrendingTopic;
import 'package:xayn_discovery_engine/src/domain/models/user_reacted.dart'
    show UserReacted;

/// Interface to Discovery Engine core.
abstract class Engine {
  /// Serializes the state of the [Engine] state.
  Future<Uint8List> serialize();

  /// Changes the currently supported markets.
  Future<void> setMarkets(
    List<HistoricDocument> history,
    FeedMarkets markets,
  );

  /// Changes the currently excluded sources.
  Future<void> setExcludedSources(
    List<HistoricDocument> history,
    Set<Source> sources,
  );

  /// Changes the trusted sources.
  Future<void> setTrustedSources(
    List<HistoricDocument> history,
    Set<Source> sources,
  );

  /// Retrieves at most [maxDocuments] feed documents.
  Future<List<DocumentWithActiveData>> getFeedDocuments(
    List<HistoricDocument> history,
    int maxDocuments,
  );

  /// Process the feedback about the user spending some time on a document.
  Future<void> timeSpent(TimeSpent timeSpent);

  /// Process the user's reaction to a document.
  ///
  /// The history is only required for positive reactions.
  Future<void> userReacted(
    List<HistoricDocument>? history,
    UserReacted userReacted,
  );

  /// Perform an active search by query.
  Future<List<DocumentWithActiveData>> searchByQuery(
    String query,
    int page,
    int pageSize,
  );

  /// Perform an active search by topic.
  Future<List<DocumentWithActiveData>> searchByTopic(
    String topic,
    int page,
    int pageSize,
  );

  /// Performs a deep search by term and market.
  Future<List<DocumentWithActiveData>> deepSearch(
    String term,
    FeedMarket market,
  );

  /// Returns the currently trending topics.
  Future<List<TrendingTopic>> getTrendingTopics();

  /// Disposes the engine.
  Future<void> dispose();

  /// Resets the AI state of the engine.
  Future<void> resetAi();
}

/// Passed to constructors/initializers of `Engine` implementing classes.
class EngineInitializer with EquatableMixin {
  /// The general configuration of the discovery engine.
  final Configuration config;

  /// The data used to bootstrap it.
  final SetupData setupData;

  /// The state to restore.
  final Uint8List? engineState;

  /// The history to use for filtering initial results.
  final List<HistoricDocument> history;

  /// An opaque encoded configuration for the AI in the ranker.
  final String? aiConfig;

  /// A set of favourite sources.
  final Set<Source> trustedSources;

  /// A set of excluded sources.
  final Set<Source> excludedSources;

  EngineInitializer({
    required this.config,
    required this.setupData,
    required this.engineState,
    required this.history,
    required this.aiConfig,
    required this.trustedSources,
    required this.excludedSources,
  });

  @override
  List<Object?> get props => [
        config,
        setupData,
        engineState,
        history,
        aiConfig,
        trustedSources,
        excludedSources,
      ];
}
