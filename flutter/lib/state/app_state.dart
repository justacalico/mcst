/// Global app state: session, server list, live events.
library;

import 'dart:async';

import 'package:flutter/foundation.dart';

import '../api/client.dart';
import '../api/types.dart';

enum SessionState { unknown, needsSetup, loggedOut, ready }

class AppState extends ChangeNotifier {
  final ApiClient api;

  /// Whether to poll /api/system periodically (disabled in tests where
  /// pending timers would fail the test binding).
  final bool enableStatsPoll;

  SessionState session = SessionState.unknown;
  String username = '';
  List<ServerDto> servers = const [];
  SystemStats stats = SystemStats.empty;
  bool apiUnreachable = false;
  Object? lastError;

  StreamSubscription<PanelEvent>? _events;
  Timer? _statsTimer;

  AppState(this.api, {this.enableStatsPoll = true});

  /// Boot: figure out whether we need setup, login, or go straight in.
  Future<void> init() async {
    try {
      if (await api.needsSetup()) {
        session = SessionState.needsSetup;
        notifyListeners();
        return;
      }
      username = await api.me();
      session = SessionState.ready;
      apiUnreachable = false;
      notifyListeners();
      _afterAuth();
    } on ApiException catch (e) {
      if (e.status == 401) {
        session = SessionState.loggedOut;
      } else {
        apiUnreachable = true;
      }
      notifyListeners();
    } catch (_) {
      apiUnreachable = true;
      notifyListeners();
    }
  }

  void _afterAuth() {
    unawaited(refreshServers());
    unawaited(refreshStats());
    _statsTimer?.cancel();
    if (enableStatsPoll) {
      _statsTimer =
          Timer.periodic(const Duration(seconds: 3), (_) => refreshStats());
    }
    _events?.cancel();
    void reconnect() {
      Future.delayed(const Duration(seconds: 3), () {
        if (session == SessionState.ready) _afterAuth();
      });
    }
    _events = api.eventsStream().listen(_onEvent,
        onError: (_) => reconnect(),
        // A clean close (server restart, proxy timeout) also needs a retry.
        onDone: reconnect,
        cancelOnError: false);
  }

  void _onEvent(PanelEvent ev) {
    if (ev.serverId.isEmpty) return;
    final i = servers.indexWhere((s) => s.id == ev.serverId);
    if (i < 0) return;
    final s = servers[i];
    switch (ev.kind) {
      case 'server_status':
        servers[i] = s.copyWith(status: ev.status);
      case 'server_stats':
        servers[i] = s.copyWith(
          cpuPercent: (ev.data['cpu_percent'] as num?)?.toDouble(),
          memBytes: (ev.data['mem_bytes'] as num?)?.toInt(),
        );
      case 'server_players':
        servers[i] = s.copyWith(
          playersOnline: (ev.data['online'] as num?)?.toInt(),
          playersMax: (ev.data['max'] as num?)?.toInt(),
          playerNames: (ev.data['names'] as List?)
              ?.map((e) => e.toString())
              .toList(),
        );
    }
    notifyListeners();
  }

  Future<void> refreshServers() async {
    try {
      servers = await api.listServers();
      apiUnreachable = false;
      lastError = null;
    } on ApiException catch (e) {
      if (e.status == 401) {
        unawaited(logout());
        return;
      }
      lastError = e;
    } catch (e) {
      lastError = e;
    }
    notifyListeners();
  }

  Future<void> refreshStats() async {
    try {
      stats = await api.systemStats();
    } on ApiException catch (e) {
      if (e.status == 401) {
        unawaited(logout());
        return;
      }
    } catch (_) {}
    notifyListeners();
  }

  Future<void> login(String user, String pass) async {
    username = await api.login(user, pass);
    session = SessionState.ready;
    notifyListeners();
    _afterAuth();
  }

  Future<void> completeSetup(String user, String pass) async {
    await api.setup(user, pass);
    username = user;
    session = SessionState.ready;
    notifyListeners();
    _afterAuth();
  }

  Future<void> logout() async {
    try {
      await api.logout();
    } catch (_) {}
    _events?.cancel();
    _statsTimer?.cancel();
    servers = const [];
    username = '';
    session = SessionState.loggedOut;
    notifyListeners();
  }

  /// Apply a lifecycle action and refresh that server.
  Future<void> lifecycle(String id, String action) async {
    await api.lifecycle(id, action);
    await refreshServers();
  }

  /// Update a single server in place (detail page edits).
  void patchServer(ServerDto s) {
    final i = servers.indexWhere((x) => x.id == s.id);
    if (i >= 0) {
      servers[i] = s;
      notifyListeners();
    }
  }

  int get runningCount =>
      servers.where((s) => s.status == ServerStatus.running).length;

  int get totalPlayers =>
      servers.fold(0, (a, s) => a + s.playersOnline);

  int get totalMem =>
      servers.fold(0, (a, s) => a + (s.status.isActive ? s.memBytes : 0));

  @override
  void dispose() {
    _events?.cancel();
    _statsTimer?.cancel();
    super.dispose();
  }
}
