// Decodes the fixtures in ../../fixtures — real serde output from the Rust
// types — with the generated Dart classes.
//
// The schemas prove the shape agrees; these prove the *encoding* does. A
// generated class can compile against a correct schema and still fail here:
// `#[serde(skip_serializing_if)]` omits keys the schema lists, defaults arrive
// as absence rather than as a value, and an enum's wire spelling is nothing the
// class declaration can check for itself.
//
// Regenerate the fixtures with:
//   cargo run --locked --example generate_fixtures

import 'dart:convert';
import 'dart:io';

import 'package:test/test.dart';
import 'package:zed_interfaces/zed_interfaces.dart';

Map<String, dynamic> fixture(String name) =>
    jsonDecode(File('../../fixtures/$name.json').readAsStringSync()) as Map<String, dynamic>;

Map<String, dynamic> caseOf(String name, String key) =>
    fixture(name)[key] as Map<String, dynamic>;

/// Every key the server sent must survive a decode/encode round trip with an
/// equal value, at every depth. The reverse does not hold — Dart writes an
/// explicit null where serde omits the key entirely — so this is a one-way
/// containment check by design, and it has to recurse: a nested `AuditEntry`
/// gains those explicit nulls too, which a shallow `equals` would reject.
void expectContains(Object? original, Object? encoded, [String path = r'$']) {
  if (original is Map) {
    expect(encoded, isA<Map<String, dynamic>>(), reason: '$path should still be an object');
    final actual = encoded! as Map<String, dynamic>;
    for (final entry in original.entries) {
      expect(actual.containsKey(entry.key), isTrue, reason: '$path.${entry.key} was dropped');
      expectContains(entry.value, actual[entry.key], '$path.${entry.key}');
    }
    return;
  }
  if (original is List) {
    expect(encoded, isA<List<dynamic>>(), reason: '$path should still be a list');
    final actual = encoded! as List<dynamic>;
    expect(actual.length, original.length, reason: '$path changed length');
    for (var i = 0; i < original.length; i++) {
      expectContains(original[i], actual[i], '$path[$i]');
    }
    return;
  }
  expect(encoded, equals(original), reason: '$path changed value');
}

void expectRoundTrip(Map<String, dynamic> original, Map<String, dynamic> encoded) =>
    expectContains(original, encoded);

void main() {
  group('PackageMetadata', () {
    test('decodes a response with every optional field present', () {
      final json = caseOf('package-metadata', 'full');
      final decoded = PackageMetadata.fromJson(json);
      expect(decoded.org, 'acme');
      expect(decoded.vcs, Vcs.jj);
      expect(decoded.versionScheme, VersionScheme.calver);
      expect(decoded.tags, ['http', 'client']);
      expect(decoded.versions, ['2.0.0', '1.0.0']);
      expectRoundTrip(json, decoded.toJson());
    });

    test('decodes a response where serde omitted every optional key', () {
      // The minimal fixture has no `description`, `latest`, `tags` or
      // `version_scheme` at all — this is what `skip_serializing_if` produces,
      // and treating absence as an error would break every real response.
      final json = caseOf('package-metadata', 'minimal');
      expect(json.containsKey('version_scheme'), isFalse);
      final decoded = PackageMetadata.fromJson(json);
      expect(decoded.description, isNull);
      expect(decoded.latest, isNull);
      expect(decoded.tags, isNull);
      expect(decoded.versionScheme, isNull, reason: 'absent means "the default", not a value');
      expectRoundTrip(json, decoded.toJson());
    });
  });

  test('VersionMetadata decodes an integer size and a defaulted format', () {
    final json = caseOf('version-metadata', 'published');
    final decoded = VersionMetadata.fromJson(json);
    expect(decoded.size, 4096);
    expect(decoded.format, ArtifactFormat.tarGz);
    expect(decoded.yanked, isFalse);
    expect(decoded.vcsCommit, isNotNull);
    expectRoundTrip(json, decoded.toJson());
  });

  test('ApiError decodes the error envelope', () {
    final json = caseOf('api-error', 'not_found');
    final decoded = ApiError.fromJson(json);
    expect(decoded.code, 'not_found');
    expectRoundTrip(json, decoded.toJson());
  });

  test('list and search responses decode their shared PackageSummary', () {
    final list = caseOf('package-list-response', 'page');
    final decodedList = PackageListResponse.fromJson(list);
    expect(decodedList.total, 1);
    expect(decodedList.items.single.name, 'http-kit');
    expectRoundTrip(list, decodedList.toJson());

    final search = caseOf('search-response', 'hit');
    final decodedSearch = SearchResponse.fromJson(search);
    expect(decodedSearch.query, 'http');
    expect(decodedSearch.items.single.org, 'acme');
    expectRoundTrip(search, decodedSearch.toJson());
  });

  test('AuditLogResponse decodes a nested list with a nullable enum', () {
    final json = caseOf('audit-log-response', 'chain');
    final decoded = AuditLogResponse.fromJson(json);
    final entry = decoded.entries.single;
    expect(entry.actionKind, AuditAction.publish);
    expect(entry.prevHash, isNull, reason: 'the first entry has no predecessor');
    expect(entry.seq, 1);
    expectRoundTrip(json, decoded.toJson());
  });

  test('SyncChangeEvent decodes opaque JSON and a nested Hlc', () {
    final json = caseOf('sync-change-event', 'upsert');
    final decoded = SyncChangeEvent.fromJson(json);
    expect(decoded.op, SyncOp.upsert);
    expect(decoded.version.counter, 3);
    expect(decoded.atMs, 1754400000001);
    expect(decoded.row, {'org': 'acme'});
    expect(decoded.writeKey, isNull);
    expectRoundTrip(json, decoded.toJson());
  });

  test('enum wire spellings match what serde emits', () {
    // Each of these would be a silent decode failure in production if the
    // generated spelling drifted from the Rust variant's rename.
    final enums = fixture('enums');
    expect(SyncWriteMode.fromJson(enums['write_mode'] as String), SyncWriteMode.optimisticQueue);
    expect(SyncErrorPolicy.fromJson(enums['error_policy'] as String), SyncErrorPolicy.emitOnly);
    expect(
      SyncConflictResolution.fromJson(enums['conflict_resolution'] as String),
      SyncConflictResolution.serverWins,
    );
    expect(ArtifactFormat.fromJson(enums['artifact_format'] as String), ArtifactFormat.zip);
    expect(Vcs.fromJson(enums['vcs'] as String), Vcs.sapling);
    expect(VersionScheme.fromJson(enums['version_scheme'] as String), VersionScheme.opaque);
    expect(AuditAction.fromJson(enums['audit_action'] as String), AuditAction.orgClaim);
  });

  test('an unknown enum value fails loudly rather than decoding to a fallback', () {
    expect(() => Vcs.fromJson('perforce'), throwsFormatException);
  });

  test('the remaining request and response bodies decode', () {
    final misc = fixture('misc-responses');
    final publish = PublishResponse.fromJson(misc['publish'] as Map<String, dynamic>);
    expect(publish.version, '1.0.0');
    expectRoundTrip(misc['publish'] as Map<String, dynamic>, publish.toJson());

    final claimRequest = ClaimOrgRequest.fromJson(misc['claim_org_request'] as Map<String, dynamic>);
    expect(claimRequest.slug, 'acme');

    final claimResponse =
        ClaimOrgResponse.fromJson(misc['claim_org_response'] as Map<String, dynamic>);
    expect(claimResponse.created, isTrue);

    expect(YankRequest.fromJson(misc['yank_request'] as Map<String, dynamic>).yanked, isTrue);
    expect(YankResponse.fromJson(misc['yank_response'] as Map<String, dynamic>).name, 'http-kit');

    final integrity =
        AuditIntegrityResponse.fromJson(misc['audit_integrity'] as Map<String, dynamic>);
    expect(integrity.intact, isTrue);
    expect(integrity.entriesChecked, 1);
    expect(integrity.firstBadSeq, isNull);

    final request =
        SemanticSearchRequest.fromJson(misc['semantic_search_request'] as Map<String, dynamic>);
    expect(request.limit, 5);
    expect(request.embedding.length, 3);

    final response =
        SemanticSearchResponse.fromJson(misc['semantic_search_response'] as Map<String, dynamic>);
    expect(response.items.single.distance, closeTo(0.25, 1e-9));
  });
}
