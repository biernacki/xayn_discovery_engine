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

import 'dart:typed_data' show Float32List;

import 'package:test/test.dart';
import 'package:xayn_discovery_engine/src/domain/models/document.dart'
    show UserReaction;
import 'package:xayn_discovery_engine/src/domain/models/news_resource.dart'
    show NewsResource;
import 'package:xayn_discovery_engine/src/domain/models/unique_id.dart'
    show DocumentId, StackId;
import 'package:xayn_discovery_engine/src/ffi/load_lib.dart' show ffi;
import 'package:xayn_discovery_engine/src/ffi/types/document/document.dart'
    show DocumentFfi;
import 'package:xayn_discovery_engine/src/ffi/types/document/document_vec.dart'
    show DocumentSliceFfi;

void main() {
  test('reading and writing a list of documents', () {
    final documents = <DocumentFfi>[
      DocumentFfi(
        id: DocumentId(),
        stackId: StackId(),
        smbertEmbedding: Float32List.fromList([.9, .1]),
        resource: NewsResource(
          title: 'fun',
          snippet: 'fun is fun',
          url: Uri.parse('https://www.foobar.example/dodo'),
          sourceUrl: Uri.parse('yyy://www.example'),
          thumbnail: null,
          datePublished: DateTime.now(),
          rank: 12,
          score: 32.625,
          country: 'Germany',
          language: 'German',
          topic: 'FunFun',
        ),
      ),
      DocumentFfi(
        id: DocumentId(),
        stackId: StackId(),
        smbertEmbedding: Float32List.fromList([9, 1]),
        resource: NewsResource(
          title: 'bun',
          snippet: 'foo bar',
          url: Uri.parse('https://www.barfoot.example/dodo'),
          sourceUrl: Uri.parse('yyy://fuu.example'),
          thumbnail: Uri.parse('https://dodo.example/'),
          datePublished: DateTime.now(),
          rank: 12,
          score: 2.125,
          country: 'Germany',
          language: 'German',
          topic: 'FunFun',
        ),
      ),
    ];
    final len = documents.length;
    final ptr = documents.createSlice();
    final res = DocumentSliceFfi.readSlice(ptr, len);
    ffi.drop_document_slice(ptr, len);
    expect(res, equals(documents));
  });

  test('conversion to documents works', () {
    final ffiDocuments = <DocumentFfi>[
      DocumentFfi(
        id: DocumentId(),
        stackId: StackId(),
        smbertEmbedding: Float32List.fromList([.9, .1]),
        resource: NewsResource(
          title: 'fun',
          snippet: 'fun is fun',
          url: Uri.parse('https://www.foobar.example/dodo'),
          sourceUrl: Uri.parse('yyy://www.example'),
          thumbnail: null,
          datePublished: DateTime.now(),
          rank: 12,
          score: 32.625,
          country: 'Germany',
          language: 'German',
          topic: 'FunFun',
        ),
      ),
      DocumentFfi(
        id: DocumentId(),
        stackId: StackId(),
        smbertEmbedding: Float32List.fromList([9, 1]),
        resource: NewsResource(
          title: 'bun',
          snippet: 'foo bar',
          url: Uri.parse('https://www.barfoot.example/dodo'),
          sourceUrl: Uri.parse('yyy://fuu.example'),
          thumbnail: Uri.parse('https://dodo.example/'),
          datePublished: DateTime.now(),
          rank: 12,
          score: 2.125,
          country: 'Germany',
          language: 'German',
          topic: 'FunFun',
        ),
      ),
    ];
    final documents = ffiDocuments.toDocumentList();

    expect(documents[0].documentId, equals(ffiDocuments[0].id));
    expect(documents[0].stackId, equals(ffiDocuments[0].stackId));
    expect(documents[0].resource, equals(ffiDocuments[0].resource));
    expect(documents[0].batchIndex, equals(0));
    expect(documents[0].userReaction, equals(UserReaction.neutral));
    expect(documents[0].isActive, isTrue);

    expect(documents[1].documentId, equals(ffiDocuments[1].id));
    expect(documents[1].stackId, equals(ffiDocuments[1].stackId));
    expect(documents[1].resource, equals(ffiDocuments[1].resource));
    expect(documents[1].batchIndex, equals(1));
    expect(documents[1].userReaction, equals(UserReaction.neutral));
    expect(documents[1].isActive, isTrue);

    expect(documents.length, equals(2));
  });
}
