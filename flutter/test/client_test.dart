import 'dart:convert';
import 'dart:io';

import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:mcst_frontend/api/client.dart';
import 'package:mcst_frontend/api/types.dart';
import 'package:flutter_test/flutter_test.dart';

HttpApiClient clientFor(Map<String, Object> Function(http.BaseRequest) handler,
    {List<String>? calls}) {
  final mock = MockClient((req) async {
    final body = handler(req);
    calls?.add('${req.method} ${req.url.path}');
    return http.Response(jsonEncode(body), 200,
        headers: {'content-type': 'application/json'});
  });
  return HttpApiClient(
      httpClient: mock, base: Uri.parse('http://localhost:25580/'));
}

void main() {
  group('HttpApiClient', () {
    test('setup/status + auth endpoints', () async {
      final calls = <String>[];
      final api = clientFor((req) {
        switch (req.url.path) {
          case '/api/setup/status':
            return {'needs_setup': true};
          case '/api/auth/me':
            return {'username': 'owner'};
          case '/api/auth/login':
            return {'username': 'owner'};
          default:
            return <String, Object>{};
        }
      }, calls: calls);

      expect(await api.needsSetup(), isTrue);
      await api.setup('owner', 'pw-123456');
      expect(await api.login('owner', 'pw-123456'), 'owner');
      await api.logout();
      expect(await api.me(), 'owner');
      await api.changePassword('old', 'new');
      expect(calls, contains('POST /api/setup'));
      expect(calls, contains('POST /api/auth/logout'));
      expect(calls, contains('POST /api/auth/password'));
    });

    test('servers CRUD + lifecycle', () async {
      final api = clientFor((req) {
        if (req.url.path == '/api/servers' && req.method == 'GET') {
          return {
            'servers': [
              {'id': 'srv-1', 'name': 'survival', 'status': 'running'}
            ]
          };
        }
        if (req.url.path.startsWith('/api/servers')) {
          return {'id': 'srv-1', 'name': 'survival', 'status': 'stopped'};
        }
        return <String, Object>{};
      });
      final list = await api.listServers();
      expect(list.single.id, 'srv-1');
      expect(list.single.status.name, 'running');
      expect((await api.getServer('srv-1')).name, 'survival');
      expect(await api.createServer({'name': 'x'}), isA<ServerDto>());
      expect(await api.updateServer('srv-1', {'port': 25566}),
          isA<ServerDto>());
      await api.deleteServer('srv-1');
      await api.lifecycle('srv-1', 'start');
      await api.sendCommand('srv-1', 'say hi');
      await api.updateJar('srv-1');
    });

    test('properties + files', () async {
      final api = clientFor((req) {
        switch (req.url.path) {
          case '/api/servers/srv-1/properties':
            return {
              'properties': {'motd': 'hi', 'max-players': '20'}
            };
          case '/api/servers/srv-1/properties/raw':
            return {};
          case '/api/servers/srv-1/files':
            return {
              'entries': [
                {'name': 'world', 'path': 'world', 'is_dir': true}
              ]
            };
          case '/api/servers/srv-1/files/read':
            return {'content': 'file body'};
          default:
            return <String, Object>{};
        }
      });
      expect((await api.getProperties('srv-1'))['motd'], 'hi');
      await api.putProperties('srv-1', {'motd': 'x'});
      await api.listFiles('srv-1', '');
      expect(await api.readFile('srv-1', 'a.txt'), 'file body');
      await api.writeFile('srv-1', 'a.txt', 'new');
      await api.mkdir('srv-1', 'dir');
      await api.deleteFile('srv-1', 'a.txt');
      await api.renameFile('srv-1', 'a', 'b');
      await api.uploadFile('srv-1', '', 'up.bin', [1, 2, 3]);
      expect(api.fileDownloadUrl('srv-1', 'logs/x.log'),
          'http://localhost:25580/api/servers/srv-1/files/download?path=logs%2Fx.log');
    });

    test('raw properties use text/plain', () async {
      String? ct;
      String? body;
      final mock = MockClient((req) async {
        if (req.method == 'PUT') {
          ct = req.headers['content-type'];
          body = req.body;
          return http.Response('{}', 200);
        }
        return http.Response('motd=hello\n', 200,
            headers: {'content-type': 'text/plain'});
      });
      final api = HttpApiClient(
          httpClient: mock, base: Uri.parse('http://localhost:25580/'));
      expect(await api.getPropertiesRaw('srv-1'), 'motd=hello\n');
      await api.putPropertiesRaw('srv-1', 'motd=bye\n');
      expect(ct, startsWith('text/plain'));
      expect(body, 'motd=bye\n');
    });

    test('backups + players + schedules', () async {
      final api = clientFor((req) {
        final p = req.url.path;
        if (p.endsWith('/backups') && req.method == 'GET') {
          return {
            'backups': [
              {'id': 'b1', 'note': 'n', 'size_bytes': 5}
            ]
          };
        }
        if (p.endsWith('/backups')) {
          return {
            'backup': {'id': 'b1', 'note': 'n', 'size_bytes': 5}
          };
        }
        if (p.contains('/players')) {
          return {
            'players': [
              {'uuid': 'u', 'name': 'Steve'}
            ]
          };
        }
        if (p.endsWith('/schedules') && req.method == 'GET') {
          return {
            'schedules': [
              {'id': 's1', 'name': 'nightly', 'action': 'restart'}
            ]
          };
        }
        if (p.contains('/schedules')) {
          return {
            'schedule': {'id': 's1', 'name': 'nightly'}
          };
        }
        return <String, Object>{};
      });
      expect((await api.listBackups('s')).single.id, 'b1');
      expect((await api.createBackup('s', 'note')).id, 'b1');
      await api.restoreBackup('s', 'b1');
      await api.deleteBackup('s', 'b1');
      expect(api.backupDownloadUrl('s', 'b1'),
          'http://localhost:25580/api/servers/s/backups/b1/download');
      expect((await api.listPlayers('s', 'ops')).single.name, 'Steve');
      expect((await api.addPlayer('s', 'ops', 'Alex')).single.uuid, 'u');
      expect(await api.removePlayer('s', 'ops', 'Alex'), isA<List>());
      expect((await api.listSchedules('s')).single.name, 'nightly');
      expect((await api.createSchedule('s', {'name': 'x'})).id, 's1');
      expect(await api.updateSchedule('s', 's1', {'enabled': 0}),
          isA<Schedule>());
      await api.deleteSchedule('s', 's1');
    });

    test('mods + versions + java + tailscale + settings + audit', () async {
      final api = clientFor((req) {
        final p = req.url.path;
        if (p.endsWith('/mods/search')) {
          return {
            'hits': [
              {'project_id': 'A', 'slug': 'sodium', 'title': 'Sodium'}
            ]
          };
        }
        if (p.endsWith('/mods/install')) return {'file': 'sodium.jar'};
        if (p.endsWith('/mods')) {
          return {
            'files': [
              {'name': 'sodium.jar', 'size': 10}
            ]
          };
        }
        if (p == '/api/versions/types') {
          return {
            'types': [
              {'id': 'paper', 'name': 'Paper', 'has_loader': false}
            ]
          };
        }
        if (p == '/api/versions') return {'versions': ['1.21.1', '1.21']};
        if (p == '/api/versions/loaders') return {'loaders': ['0.16.0']};
        if (p == '/api/java/required') return {'major': 21};
        if (p == '/api/java' && req.method == 'GET') {
          return {
            'installs': [
              {'path': '/opt/java', 'major': 21, 'version': '21.0.3'}
            ]
          };
        }
        if (p == '/api/java') return {'path': '/opt/java21'};
        if (p == '/api/tailscale') {
          return {
            'status': {'installed': true, 'running': true, 'ip': '100.1.2.3'},
            'panel_serve_enabled': true
          };
        }
        if (p == '/api/settings') {
          return {
            'settings': {'panel_name': 'mcst'}
          };
        }
        if (p == '/api/audit') {
          return {
            'entries': [
              {'id': 1, 'ts': 't', 'actor': 'owner', 'action': 'login'}
            ]
          };
        }
        return <String, Object>{};
      });
      expect((await api.searchMods('s', 'sodium')).single.slug, 'sodium');
      expect(await api.installMod('s', 'sodium'), 'sodium.jar');
      expect((await api.listMods('s')).single.name, 'sodium.jar');
      expect((await api.serverTypes()).single.id, 'paper');
      expect(await api.mcVersions('paper'), ['1.21.1', '1.21']);
      expect(await api.loaders('fabric', '1.21.1'), ['0.16.0']);
      expect(await api.requiredJava('1.21.1'), 21);
      expect((await api.listJava()).single.major, 21);
      expect(await api.installJava(21), '/opt/java21');
      await api.deleteJava('x');
      expect((await api.tailscaleStatus()).ip, '100.1.2.3');
      await api.tailscalePanelServe(true, httpsPort: 443);
      await api.tailscaleServer('s', true, tailnetPort: 25565);
      expect((await api.getSettings())['panel_name'], 'mcst');
      await api.putSettings({'k': 'v'});
      expect((await api.auditLog()).single.actor, 'owner');
    });

    test('error responses raise ApiException with server message', () async {
      final mock = MockClient((req) async => http.Response(
          jsonEncode({'error': 'server not found'}), 404));
      final api = HttpApiClient(
          httpClient: mock, base: Uri.parse('http://localhost:25580/'));
      try {
        await api.listServers();
        fail('expected ApiException');
      } on ApiException catch (e) {
        expect(e.status, 404);
        expect(e.message, 'server not found');
      }
      // Non-JSON error body falls back to a generic message.
      final mock2 = MockClient((req) async => http.Response('oops', 500));
      final api2 = HttpApiClient(
          httpClient: mock2, base: Uri.parse('http://localhost:25580/'));
      try {
        await api2.me();
        fail('expected ApiException');
      } on ApiException catch (e) {
        expect(e.message, 'request failed (500)');
      }
    });

    test('console + events websocket streams decode frames', () async {
      final server = await HttpServer.bind('127.0.0.1', 0);
      addTearDown(server.close);
      server.listen((req) async {
        final ws = await WebSocketTransformer.upgrade(req);
        if (req.uri.path.endsWith('/console')) {
          ws.add(jsonEncode({
            'type': 'history',
            'lines': ['one', 'two']
          }));
          ws.add(jsonEncode({'type': 'line', 'line': 'three'}));
        } else {
          ws.add(jsonEncode({
            'kind': 'server_status',
            'server_id': 'srv-1',
            'data': {'status': 'running'}
          }));
        }
      });
      final base = Uri.parse('http://127.0.0.1:${server.port}/');
      final api = HttpApiClient(
          httpClient: MockClient((r) async => http.Response('{}', 200)),
          base: base);

      final console = await api.consoleStream('srv-1').take(2).toList();
      expect(console[0], isA<ConsoleHistory>());
      expect((console[0] as ConsoleHistory).lines, ['one', 'two']);
      expect((console[1] as ConsoleLine).line, 'three');

      final ev = await api.eventsStream().first;
      expect(ev.kind, 'server_status');
      expect(ev.serverId, 'srv-1');
      expect(ev.status, ServerStatus.running);
    });
  });
}
