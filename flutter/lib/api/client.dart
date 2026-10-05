/// API client abstraction + HTTP implementation talking to the embedded
/// backend over same-origin HTTP.
library;

import 'dart:async';
import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:web_socket_channel/web_socket_channel.dart';

import 'types.dart';

/// Thrown for non-2xx responses; [message] is the server's `error` field.
class ApiException implements Exception {
  final int status;
  final String message;
  const ApiException(this.status, this.message);

  @override
  String toString() => message;
}

/// Contract used by the app; tests implement [FakeApiClient].
abstract class ApiClient {
  // auth / setup
  Future<bool> needsSetup();
  Future<void> setup(String username, String password);
  Future<String> login(String username, String password);
  Future<void> logout();
  Future<String> me();
  Future<void> changePassword(String current, String next);

  // dashboard
  Future<SystemStats> systemStats();
  Future<List<ServerDto>> listServers();

  // server lifecycle
  Future<ServerDto> getServer(String id);
  Future<ServerDto> createServer(Map<String, Object?> body);
  Future<ServerDto> updateServer(String id, Map<String, Object?> patch);
  Future<void> deleteServer(String id);
  Future<void> lifecycle(String id, String action);
  Future<void> sendCommand(String id, String command);
  Future<void> updateJar(String id);

  // properties
  Future<Map<String, String>> getProperties(String id);
  Future<void> putProperties(String id, Map<String, String> props);
  Future<String> getPropertiesRaw(String id);
  Future<void> putPropertiesRaw(String id, String body);

  // files
  Future<List<FileEntry>> listFiles(String id, String path);
  Future<String> readFile(String id, String path);
  Future<void> writeFile(String id, String path, String content);
  Future<void> mkdir(String id, String path);
  Future<void> deleteFile(String id, String path);
  Future<void> renameFile(String id, String from, String to);
  String fileDownloadUrl(String id, String path);
  Future<void> uploadFile(
      String id, String dir, String name, List<int> bytes);

  // backups
  Future<List<Backup>> listBackups(String id);
  Future<Backup> createBackup(String id, String note);
  Future<void> restoreBackup(String id, String backupId);
  Future<void> deleteBackup(String id, String backupId);
  String backupDownloadUrl(String id, String backupId);

  // players
  Future<List<PlayerEntry>> listPlayers(String id, String list);
  Future<List<PlayerEntry>> addPlayer(String id, String list, String name);
  Future<List<PlayerEntry>> removePlayer(String id, String list, String name);

  // schedules
  Future<List<Schedule>> listSchedules(String id);
  Future<Schedule> createSchedule(String id, Map<String, Object?> body);
  Future<Schedule> updateSchedule(
      String id, String sid, Map<String, Object?> body);
  Future<void> deleteSchedule(String id, String sid);

  // mods
  Future<List<ModrinthHit>> searchMods(String id, String query);
  Future<String> installMod(String id, String project);
  Future<List<InstalledFile>> listMods(String id);

  // versions
  Future<List<ServerTypeInfo>> serverTypes();
  Future<List<String>> mcVersions(String type);
  Future<List<String>> loaders(String type, String mcVersion);
  Future<int> requiredJava(String mcVersion);

  // java
  Future<List<JavaInstall>> listJava();
  Future<String> installJava(int major);
  Future<void> deleteJava(String id);

  // tailscale / settings / audit
  Future<TailscaleStatus> tailscaleStatus();
  Future<void> tailscalePanelServe(bool enable, {int? httpsPort});
  Future<void> tailscaleServer(String id, bool enable, {int? tailnetPort});
  Future<Map<String, String>> getSettings();
  Future<void> putSettings(Map<String, String> settings);
  Future<List<AuditEntry>> auditLog();

  // streams
  Stream<ConsoleEvent> consoleStream(String serverId);
  Stream<PanelEvent> eventsStream();
}

/// A console WebSocket message.
sealed class ConsoleEvent {
  const ConsoleEvent();
}

class ConsoleHistory extends ConsoleEvent {
  final List<String> lines;
  const ConsoleHistory(this.lines);
}

class ConsoleLine extends ConsoleEvent {
  final String line;
  const ConsoleLine(this.line);
}

/// A panel-wide event pushed over `/api/events`.
class PanelEvent {
  final String kind;
  final String serverId;
  final Map<String, dynamic> data;
  const PanelEvent(this.kind, this.serverId, this.data);

  ServerStatus? get status =>
      kind == 'server_status' ? ServerStatus.parse(data['status'] as String?) : null;
}

class InstalledFile {
  final String name;
  final int size;
  const InstalledFile({required this.name, required this.size});
}

/// Same-origin HTTP client. Cookies carry the session; on web the browser
/// includes them automatically.
class HttpApiClient implements ApiClient {
  final http.Client _http;
  final Uri base;

  HttpApiClient({http.Client? httpClient, Uri? base})
      : _http = httpClient ?? http.Client(),
        base = base ?? Uri.base;

  Uri _u(String path, [Map<String, String>? q]) {
    final uri = base.resolve('/api$path');
    return q == null ? uri : uri.replace(queryParameters: q);
  }

  Uri _ws(String path) {
    final s = base.scheme == 'https' ? 'wss' : 'ws';
    return base.replace(scheme: s, path: '/api$path', query: '', fragment: '');
  }

  Never _fail(http.Response r) {
    String msg = 'request failed (${r.statusCode})';
    try {
      final j = jsonDecode(r.body);
      if (j is Map && j['error'] is String) msg = j['error'] as String;
    } catch (_) {}
    throw ApiException(r.statusCode, msg);
  }

  Future<Map<String, dynamic>> _get(String path, [Map<String, String>? q]) async {
    final r = await _http.get(_u(path, q));
    if (r.statusCode >= 400) _fail(r);
    final j = jsonDecode(r.body);
    if (j is! Map<String, dynamic>) {
      throw ApiException(r.statusCode, 'unexpected response body');
    }
    return j;
  }

  Future<Map<String, dynamic>> _send(
      String method, String path, Object? body) async {
    final req = http.Request(method, _u(path));
    if (body != null) {
      req.headers['content-type'] = 'application/json';
      req.body = jsonEncode(body);
    }
    final streamed = await _http.send(req);
    final r = await http.Response.fromStream(streamed);
    if (r.statusCode >= 400) _fail(r);
    if (r.body.isEmpty) return const {};
    final j = jsonDecode(r.body);
    if (j is! Map<String, dynamic>) {
      throw ApiException(r.statusCode, 'unexpected response body');
    }
    return j;
  }

  Future<Map<String, dynamic>> _post(String path, [Object? body]) =>
      _send('POST', path, body);
  Future<Map<String, dynamic>> _patch(String path, [Object? body]) =>
      _send('PATCH', path, body);
  Future<Map<String, dynamic>> _put(String path, [Object? body]) =>
      _send('PUT', path, body);
  Future<Map<String, dynamic>> _delete(String path, [Object? body]) =>
      _send('DELETE', path, body);

  @override
  Future<bool> needsSetup() async =>
      (await _get('/setup/status'))['needs_setup'] == true;

  @override
  Future<void> setup(String username, String password) =>
      _post('/setup', {'username': username, 'password': password});

  @override
  Future<String> login(String username, String password) async {
    final j = await _post('/auth/login', {
      'username': username,
      'password': password,
    });
    return j['username'] as String? ?? username;
  }

  @override
  Future<void> logout() => _post('/auth/logout');

  @override
  Future<String> me() async => (await _get('/auth/me'))['username'] as String;

  @override
  Future<void> changePassword(String current, String next) =>
      _post('/auth/password', {'current': current, 'new_password': next});

  @override
  Future<SystemStats> systemStats() async =>
      SystemStats.fromJson(await _get('/system'));

  @override
  Future<List<ServerDto>> listServers() async {
    final j = await _get('/servers');
    return (j['servers'] as List? ?? [])
        .map((e) => ServerDto.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  @override
  Future<ServerDto> getServer(String id) async =>
      ServerDto.fromJson(await _get('/servers/$id'));

  @override
  Future<ServerDto> createServer(Map<String, Object?> body) async =>
      ServerDto.fromJson(await _post('/servers', body));

  @override
  Future<ServerDto> updateServer(String id, Map<String, Object?> patch) async =>
      ServerDto.fromJson(await _patch('/servers/$id', patch));

  @override
  Future<void> deleteServer(String id) => _delete('/servers/$id');

  @override
  Future<void> lifecycle(String id, String action) =>
      _post('/servers/$id/$action');

  @override
  Future<void> sendCommand(String id, String command) =>
      _post('/servers/$id/command', {'command': command});

  @override
  Future<void> updateJar(String id) => _post('/servers/$id/update');

  @override
  Future<Map<String, String>> getProperties(String id) async {
    final j = await _get('/servers/$id/properties');
    return (j['properties'] as Map? ?? {})
        .map((k, v) => MapEntry(k.toString(), v.toString()));
  }

  @override
  Future<void> putProperties(String id, Map<String, String> props) =>
      _put('/servers/$id/properties', props);

  @override
  Future<String> getPropertiesRaw(String id) async {
    final r = await _http.get(_u('/servers/$id/properties/raw'));
    if (r.statusCode >= 400) _fail(r);
    return r.body;
  }

  @override
  Future<void> putPropertiesRaw(String id, String body) async {
    final req = http.Request('PUT', _u('/servers/$id/properties/raw'));
    req.headers['content-type'] = 'text/plain';
    req.body = body;
    final r = await http.Response.fromStream(await _http.send(req));
    if (r.statusCode >= 400) _fail(r);
  }

  @override
  Future<List<FileEntry>> listFiles(String id, String path) async {
    final j = await _get('/servers/$id/files', {'path': path});
    return (j['entries'] as List? ?? [])
        .map((e) => FileEntry.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  @override
  Future<String> readFile(String id, String path) async =>
      (await _get('/servers/$id/files/read', {'path': path}))['content']
          as String;

  @override
  Future<void> writeFile(String id, String path, String content) =>
      _put('/servers/$id/files/write', {'path': path, 'content': content});

  @override
  Future<void> mkdir(String id, String path) =>
      _post('/servers/$id/files/mkdir', {'path': path});

  @override
  Future<void> deleteFile(String id, String path) =>
      _post('/servers/$id/files/delete', {'path': path});

  @override
  Future<void> renameFile(String id, String from, String to) =>
      _post('/servers/$id/files/rename', {'from': from, 'to': to});

  @override
  String fileDownloadUrl(String id, String path) =>
      _u('/servers/$id/files/download', {'path': path}).toString();

  @override
  Future<void> uploadFile(
      String id, String dir, String name, List<int> bytes) async {
    final req = http.MultipartRequest('POST', _u('/servers/$id/files/upload'));
    req.fields['path'] = dir;
    req.files.add(http.MultipartFile.fromBytes('file', bytes, filename: name));
    final r = await http.Response.fromStream(await _http.send(req));
    if (r.statusCode >= 400) _fail(r);
  }

  @override
  Future<List<Backup>> listBackups(String id) async {
    final j = await _get('/servers/$id/backups');
    return (j['backups'] as List? ?? [])
        .map((e) => Backup.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  @override
  Future<Backup> createBackup(String id, String note) async =>
      Backup.fromJson((await _post('/servers/$id/backups', {'note': note}))['backup']);

  @override
  Future<void> restoreBackup(String id, String backupId) =>
      _post('/servers/$id/backups/$backupId/restore');

  @override
  Future<void> deleteBackup(String id, String backupId) =>
      _delete('/servers/$id/backups/$backupId');

  @override
  String backupDownloadUrl(String id, String backupId) =>
      _u('/servers/$id/backups/$backupId/download').toString();

  @override
  Future<List<PlayerEntry>> listPlayers(String id, String list) async {
    final j = await _get('/servers/$id/players/$list');
    return (j['players'] as List? ?? [])
        .map((e) => PlayerEntry.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  @override
  Future<List<PlayerEntry>> addPlayer(String id, String list, String name) async {
    final j = await _post('/servers/$id/players/$list', {'name': name});
    return (j['players'] as List? ?? [])
        .map((e) => PlayerEntry.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  @override
  Future<List<PlayerEntry>> removePlayer(
      String id, String list, String name) async {
    final j = await _delete('/servers/$id/players/$list/$name');
    return (j['players'] as List? ?? [])
        .map((e) => PlayerEntry.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  @override
  Future<List<Schedule>> listSchedules(String id) async {
    final j = await _get('/servers/$id/schedules');
    return (j['schedules'] as List? ?? [])
        .map((e) => Schedule.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  @override
  Future<Schedule> createSchedule(String id, Map<String, Object?> body) async =>
      Schedule.fromJson(
          (await _post('/servers/$id/schedules', body))['schedule']);

  @override
  Future<Schedule> updateSchedule(
          String id, String sid, Map<String, Object?> body) async =>
      Schedule.fromJson(
          (await _patch('/servers/$id/schedules/$sid', body))['schedule']);

  @override
  Future<void> deleteSchedule(String id, String sid) =>
      _delete('/servers/$id/schedules/$sid');

  @override
  Future<List<ModrinthHit>> searchMods(String id, String query) async {
    final j = await _get('/servers/$id/mods/search', {'q': query});
    return (j['hits'] as List? ?? [])
        .map((e) => ModrinthHit.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  @override
  Future<String> installMod(String id, String project) async =>
      (await _post('/servers/$id/mods/install', {'project': project}))['file']
          as String;

  @override
  Future<List<InstalledFile>> listMods(String id) async {
    final j = await _get('/servers/$id/mods');
    return (j['files'] as List? ?? [])
        .map((e) => InstalledFile(
              name: e['name'] as String? ?? '',
              size: (e['size'] as num?)?.toInt() ?? 0,
            ))
        .toList();
  }

  @override
  Future<List<ServerTypeInfo>> serverTypes() async {
    final j = await _get('/versions/types');
    return (j['types'] as List? ?? [])
        .map((e) => ServerTypeInfo.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  @override
  Future<List<String>> mcVersions(String type) async {
    final j = await _get('/versions', {'type': type});
    return (j['versions'] as List? ?? []).map((e) => e.toString()).toList();
  }

  @override
  Future<List<String>> loaders(String type, String mcVersion) async {
    final j =
        await _get('/versions/loaders', {'type': type, 'mc_version': mcVersion});
    return (j['loaders'] as List? ?? []).map((e) => e.toString()).toList();
  }

  @override
  Future<int> requiredJava(String mcVersion) async =>
      (await _get('/java/required', {'mc_version': mcVersion}))['major'] as int;

  @override
  Future<List<JavaInstall>> listJava() async {
    final j = await _get('/java');
    return (j['installs'] as List? ?? [])
        .map((e) => JavaInstall.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  @override
  Future<String> installJava(int major) async =>
      (await _post('/java', {'major': major}))['path'] as String;

  @override
  Future<void> deleteJava(String id) => _delete('/java/$id');

  @override
  Future<TailscaleStatus> tailscaleStatus() async =>
      TailscaleStatus.fromJson(await _get('/tailscale'));

  @override
  Future<void> tailscalePanelServe(bool enable, {int? httpsPort}) =>
      _post('/tailscale', {'enable': enable, 'https_port': ?httpsPort});

  @override
  Future<void> tailscaleServer(String id, bool enable, {int? tailnetPort}) =>
      _post('/servers/$id/tailscale',
          {'enable': enable, 'tailnet_port': ?tailnetPort});

  @override
  Future<Map<String, String>> getSettings() async {
    final j = await _get('/settings');
    return (j['settings'] as Map? ?? {})
        .map((k, v) => MapEntry(k.toString(), v.toString()));
  }

  @override
  Future<void> putSettings(Map<String, String> settings) =>
      _put('/settings', settings);

  @override
  Future<List<AuditEntry>> auditLog() async {
    final j = await _get('/audit');
    return (j['entries'] as List? ?? [])
        .map((e) => AuditEntry.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  @override
  Stream<ConsoleEvent> consoleStream(String serverId) {
    final ch = WebSocketChannel.connect(_ws('/servers/$serverId/console'));
    return ch.stream.map((raw) {
      final j = jsonDecode(raw as String) as Map<String, dynamic>;
      return switch (j['type']) {
        'history' => ConsoleHistory(
            (j['lines'] as List? ?? []).map((e) => e.toString()).toList()),
        _ => ConsoleLine(j['line'] as String? ?? ''),
      };
    });
  }

  @override
  Stream<PanelEvent> eventsStream() {
    final ch = WebSocketChannel.connect(_ws('/events'));
    return ch.stream.map((raw) {
      final j = jsonDecode(raw as String) as Map<String, dynamic>;
      return PanelEvent(
        j['kind'] as String? ?? '',
        j['server_id'] as String? ?? '',
        (j['data'] as Map?)?.cast<String, dynamic>() ?? const {},
      );
    });
  }
}
