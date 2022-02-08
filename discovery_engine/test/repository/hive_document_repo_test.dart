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

import 'package:hive/hive.dart' show Hive;
import 'package:test/test.dart';
import 'package:xayn_discovery_engine/src/domain/models/document.dart'
    show Document, UserReaction;
import 'package:xayn_discovery_engine/src/domain/models/unique_id.dart'
    show StackId;
import 'package:xayn_discovery_engine/src/infrastructure/box_name.dart'
    show documentBox;
import 'package:xayn_discovery_engine/src/infrastructure/repository/hive_document_repo.dart'
    show HiveDocumentRepository;

import '../discovery_engine/utils/utils.dart';
import '../logging.dart' show setupLogging;

Future<void> main() async {
  setupLogging();

  final box = await Hive.openBox<Document>(documentBox, bytes: Uint8List(0));
  final repo = HiveDocumentRepository();

  group('DocumentRepository', () {
    final stackId = StackId();
    final doc1 = Document(
      stackId: stackId,
      batchIndex: 0,
      resource: mockNewsResource,
    );
    final doc2 = Document(
      stackId: stackId,
      batchIndex: 1,
      resource: mockNewsResource,
    );

    tearDown(() async {
      await box.clear();

      // reset test docs
      doc1.isActive = true;
      doc1.userReaction = UserReaction.neutral;
      doc2.isActive = true;
      doc2.userReaction = UserReaction.neutral;
    });

    group('empty box', () {
      test('add new', () async {
        expect(box, isEmpty);
        await repo.update(doc1);
        expect(box, hasLength(1));
        expect(box.values.first, equals(doc1));
      });

      test('fetch all from none', () async {
        expect(await repo.fetchAll(), isEmpty);
      });

      test('fetch absent', () async {
        expect(await repo.fetchById(doc1.documentId), isNull);
      });

      test('fetch by absent ids', () async {
        final docs = await repo.fetchByIds({doc1.documentId, doc2.documentId});
        expect(docs, isEmpty);
      });

      test('add new multiple', () async {
        expect(box, isEmpty);
        await repo.updateMany([doc1, doc2]);
        expect(box, hasLength(2));
        expect(box.values, containsAll(<Document>[doc1, doc2]));
      });
    });

    group('nonempty box', () {
      setUp(() async {
        await repo.update(doc1);
      });

      test('update existing', () async {
        expect(doc1.isActive, isTrue);

        await repo.update(doc1..isActive = false);

        expect(box, hasLength(1));
        expect(box.values.first.isActive, isFalse);
      });

      test('add new', () async {
        await repo.update(doc2);
        expect(box, hasLength(2));
        expect(box.values, containsAll(<Document>[doc1, doc2]));
      });

      test('fetch present then absent', () async {
        final doc1Fetched = await repo.fetchById(doc1.documentId);
        expect(doc1Fetched, equals(doc1));

        final doc2Fetched = await repo.fetchById(doc2.documentId);
        expect(doc2Fetched, isNull);
      });

      test('fetch all', () async {
        var all = await repo.fetchAll();
        expect(all, hasLength(1));
        expect(all.first, equals(doc1));

        await repo.update(doc2);
        all = await repo.fetchAll();
        expect(all, hasLength(2));
        expect(all, containsAll(<Document>[doc1, doc2]));
      });

      test('fetch present and absent', () async {
        final docs = await repo.fetchByIds({doc1.documentId, doc2.documentId});
        expect(docs, equals([doc1]));
      });

      test('update existing and add new', () async {
        expect(box.values.first.isActive, isTrue);

        doc1.isActive = false;
        await repo.updateMany([doc1, doc2]);

        expect(box, hasLength(2));
        expect(box.values, containsAll(<Document>[doc1, doc2]));
      });
    });
  });
}
