import 'package:flutter_test/flutter_test.dart';
import 'package:mcst_frontend/api/client.dart';
import 'package:mcst_frontend/api/types.dart';
import 'package:mcst_frontend/state/app_state.dart';

import 'fake_client.dart';

void main() {
  group('AppState.init', () {
    test('needs setup → needsSetup', () async {
      final api = FakeApiClient()..setupNeeded = true;
      final app = AppState(api, enableStatsPoll: false);
      addTearDown(app.dispose);
      await app.init();
      await pumpEventQueue();
      expect(app.session, SessionState.needsSetup);
    });

    test('valid session → ready + loads servers', () async {
      final api = FakeApiClient(servers: [FakeApiClient.server()]);
      final app = AppState(api, enableStatsPoll: false);
      addTearDown(app.dispose);
      await app.init();
      await pumpEventQueue();
      expect(app.session, SessionState.ready);
      expect(app.servers.length, 1);
    });

    test('401 → loggedOut', () async {
      final api = _FailAuthClient();
      final app = AppState(api, enableStatsPoll: false);
      addTearDown(app.dispose);
      await app.init();
      await pumpEventQueue();
      expect(app.session, SessionState.loggedOut);
    });
  });

  group('session flows', () {
    test('login success transitions to ready', () async {
      final api = FakeApiClient();
      final app = AppState(api, enableStatsPoll: false);
      addTearDown(app.dispose);
      await app.init(); // me() succeeds by default
      await app.login('u', 'p');
      expect(app.session, SessionState.ready);
      expect(app.username, 'u');
    });

    test('completeSetup creates account and goes ready', () async {
      final api = FakeApiClient()..setupNeeded = true;
      final app = AppState(api, enableStatsPoll: false);
      addTearDown(app.dispose);
      await app.init();
      await pumpEventQueue();
      expect(app.session, SessionState.needsSetup);
      await app.completeSetup('owner', 'secret-pw');
      expect(app.session, SessionState.ready);
      expect(app.username, 'owner');
    });

    test('logout clears state', () async {
      final api = FakeApiClient(servers: [FakeApiClient.server()]);
      final app = AppState(api, enableStatsPoll: false);
      addTearDown(app.dispose);
      await app.init();
      await pumpEventQueue();
      await app.logout();
      expect(app.session, SessionState.loggedOut);
      expect(app.servers, isEmpty);
      expect(app.username, '');
    });
  });

  group('derived stats + events', () {
    test('runningCount / totals', () async {
      final api = FakeApiClient(servers: [
        FakeApiClient.server(status: ServerStatus.running, mem: 100),
        FakeApiClient.server(
            id: 'b', status: ServerStatus.stopped, online: 0, mem: 0),
      ]);
      final app = AppState(api, enableStatsPoll: false);
      addTearDown(app.dispose);
      await app.init();
      await pumpEventQueue();
      expect(app.runningCount, 1);
      expect(app.totalPlayers, 2);
      expect(app.totalMem, 100);
    });

    test('server_status event patches the server', () async {
      final api = FakeApiClient(servers: [FakeApiClient.server()]);
      final app = AppState(api, enableStatsPoll: false);
      addTearDown(app.dispose);
      await app.init();
      await pumpEventQueue();
      api.pushEvent(const PanelEvent(
          'server_status', 'srv-1', {'status': 'stopping'}));
      await pumpEventQueue();
      expect(app.servers.first.status, ServerStatus.stopping);
    });

    test('stats and players events patch fields', () async {
      final api = FakeApiClient(servers: [FakeApiClient.server()]);
      final app = AppState(api, enableStatsPoll: false);
      addTearDown(app.dispose);
      await app.init();
      await pumpEventQueue();
      api.pushEvent(const PanelEvent(
          'server_stats', 'srv-1', {'cpu_percent': 55.5, 'mem_bytes': 999}));
      api.pushEvent(const PanelEvent(
          'server_players', 'srv-1', {'online': 4, 'max': 30, 'names': []}));
      api.pushEvent(const PanelEvent('server_status', '', {})); // no-op
      await pumpEventQueue();
      expect(app.servers.first.cpuPercent, 55.5);
      expect(app.servers.first.memBytes, 999);
      expect(app.servers.first.playersOnline, 4);
    });

    test('unknown server event ignored', () async {
      final api = FakeApiClient();
      final app = AppState(api, enableStatsPoll: false);
      addTearDown(app.dispose);
      await app.init();
      await pumpEventQueue();
      api.pushEvent(const PanelEvent('server_status', 'ghost', {'status': 'running'}));
      expect(app.servers, isEmpty);
    });
  });

  group('lifecycle + refresh', () {
    test('lifecycle calls api and refreshes', () async {
      final api = FakeApiClient(servers: [FakeApiClient.server()]);
      final app = AppState(api, enableStatsPoll: false);
      addTearDown(app.dispose);
      await app.init();
      await pumpEventQueue();
      await app.lifecycle('srv-1', 'stop');
      expect(api.calls, contains('stop:srv-1'));
    });

    test('refreshServers records errors', () async {
      final api = FakeApiClient();
      final app = AppState(api, enableStatsPoll: false);
      addTearDown(app.dispose);
      await app.init();
      await pumpEventQueue();
      api.failNext = true;
      await app.refreshServers();
      expect(app.lastError, isNotNull);
    });

    test('patchServer updates in place', () async {
      final api = FakeApiClient(servers: [FakeApiClient.server()]);
      final app = AppState(api, enableStatsPoll: false);
      addTearDown(app.dispose);
      await app.init();
      await pumpEventQueue();
      app.patchServer(FakeApiClient.server(status: ServerStatus.crashed));
      expect(app.servers.first.status, ServerStatus.crashed);
      // unknown id → no-op
      app.patchServer(FakeApiClient.server(id: 'ghost'));
      expect(app.servers.length, 1);
    });
  });
}

class _FailAuthClient extends FakeApiClient {
  @override
  Future<String> me() =>
      Future.error(const ApiException(401, 'session expired'));
}
