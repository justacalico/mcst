import 'dart:async';
import 'dart:convert';

import 'package:mcst_frontend/api/client.dart';
import 'package:mcst_frontend/api/types.dart';

/// In-memory fake used by widget/unit tests and goldens.
class FakeApiClient implements ApiClient {
  bool setupNeeded = false;
  String user = 'owner';
  List<ServerDto> servers;
  SystemStats stats;
  bool failNext = false;
  final List<String> calls = [];

  final _console = StreamController<ConsoleEvent>.broadcast();
  final _events = StreamController<PanelEvent>.broadcast();

  FakeApiClient({
    List<ServerDto>? servers,
    SystemStats? stats,
  })  : servers = servers ?? [],
        stats = stats ?? _stats;

  static const _stats = SystemStats(
    cpuPercent: 23.5,
    cpuCount: 16,
    memTotal: 34 * 1024 * 1024 * 1024,
    memUsed: 12 * 1024 * 1024 * 1024,
    memAvailable: 22 * 1024 * 1024 * 1024,
    swapTotal: 8 * 1024 * 1024 * 1024,
    swapUsed: 0,
    diskTotal: 500 * 1024 * 1024 * 1024,
    diskUsed: 120 * 1024 * 1024 * 1024,
    uptimeSec: 3 * 86400 + 7200,
    hostname: 'homelab',
    os: 'Fedora Linux 44',
    kernel: '6.19.10',
    arch: 'x86_64',
    loadAvg: [0.5, 0.4, 0.3],
    dataDirBytes: 8 * 1024 * 1024 * 1024,
  );

  static ServerDto server({
    String id = 'srv-1',
    String name = 'SMP',
    String type = 'paper',
    String mc = '1.21.1',
    String loader = '',
    int port = 25565,
    ServerStatus status = ServerStatus.running,
    int online = 2,
    int max = 20,
    List<String> names = const ['Steve', 'Alex'],
    double cpu = 12.4,
    int mem = 1536 * 1024 * 1024,
    int uptime = 5400,
  }) =>
      ServerDto(
        id: id,
        name: name,
        serverType: type,
        mcVersion: mc,
        loaderVersion: loader,
        port: port,
        memoryMb: 4096,
        minMemoryMb: 0,
        javaPath: 'java',
        jvmArgs: '',
        dir: '/data/servers/$id',
        jar: 'server.jar',
        autoStart: true,
        restartOnCrash: true,
        shutdownTimeoutSec: 30,
        emptyStopMinutes: 0,
        icon: '',
        createdAt: '2026-10-01T00:00:00Z',
        status: status,
        playersOnline: online,
        playersMax: max,
        playerNames: names,
        cpuPercent: cpu,
        memBytes: mem,
        uptimeSec: uptime,
      );

  void pushConsoleLine(String line) =>
      _console.add(ConsoleLine(line));
  void pushEvent(PanelEvent ev) => _events.add(ev);

  T _maybeFail<T>(T v) {
    if (failNext) {
      failNext = false;
      throw const ApiException(500, 'boom');
    }
    return v;
  }

  @override
  Future<bool> needsSetup() async => setupNeeded;

  @override
  Future<void> setup(String u, String p) async {
    setupNeeded = false;
    user = u;
  }

  @override
  Future<String> login(String u, String p) async {
    if (p == 'wrong') throw const ApiException(401, 'invalid credentials');
    user = u;
    return u;
  }

  @override
  Future<void> logout() async {}

  @override
  Future<String> me() async => user;

  @override
  Future<void> changePassword(String c, String n) async {
    if (c != 'current') throw const ApiException(403, 'wrong password');
  }

  @override
  Future<SystemStats> systemStats() async => stats;

  @override
  Future<List<ServerDto>> listServers() async => _maybeFail(servers);

  @override
  Future<ServerDto> getServer(String id) async =>
      servers.firstWhere((s) => s.id == id, orElse: () => server(id: id));

  @override
  Future<ServerDto> createServer(Map<String, Object?> body) async {
    calls.add('create:${body['name']}');
    final s = server(id: 'new-1', name: body['name'] as String? ?? 'x');
    servers = [...servers, s];
    return s;
  }

  @override
  Future<ServerDto> updateServer(String id, Map<String, Object?> patch) async {
    final i = servers.indexWhere((s) => s.id == id);
    if (i < 0) throw const ApiException(404, 'not found');
    final s = servers[i];
    final updated = ServerDto(
      id: s.id,
      name: patch['name'] as String? ?? s.name,
      serverType: s.serverType,
      mcVersion: s.mcVersion,
      loaderVersion: s.loaderVersion,
      port: patch['port'] as int? ?? s.port,
      memoryMb: patch['memory_mb'] as int? ?? s.memoryMb,
      minMemoryMb: s.minMemoryMb,
      javaPath: s.javaPath,
      jvmArgs: patch['jvm_args'] as String? ?? s.jvmArgs,
      dir: s.dir,
      jar: s.jar,
      autoStart: patch['auto_start'] as bool? ?? s.autoStart,
      restartOnCrash: patch['restart_on_crash'] as bool? ?? s.restartOnCrash,
      shutdownTimeoutSec:
          patch['shutdown_timeout_sec'] as int? ?? s.shutdownTimeoutSec,
      emptyStopMinutes: patch['empty_stop_minutes'] as int? ?? s.emptyStopMinutes,
      icon: s.icon,
      createdAt: s.createdAt,
      status: s.status,
      playersOnline: s.playersOnline,
      playersMax: s.playersMax,
      playerNames: s.playerNames,
      cpuPercent: s.cpuPercent,
      memBytes: s.memBytes,
      uptimeSec: s.uptimeSec,
    );
    servers[i] = updated;
    return updated;
  }

  @override
  Future<void> deleteServer(String id) async =>
      servers = servers.where((s) => s.id != id).toList();

  @override
  Future<void> lifecycle(String id, String action) async {
    calls.add('$action:$id');
  }

  @override
  Future<void> sendCommand(String id, String command) async {
    calls.add('cmd:$id:$command');
  }

  @override
  Future<void> updateJar(String id) async => calls.add('update:$id');

  @override
  Future<Map<String, String>> getProperties(String id) async =>
      {'server-port': '25565', 'motd': 'SMP', 'max-players': '20'};

  @override
  Future<void> putProperties(String id, Map<String, String> props) async {}

  @override
  Future<String> getPropertiesRaw(String id) async =>
      'server-port=25565\nmotd=SMP\n';

  @override
  Future<void> putPropertiesRaw(String id, String body) async {}

  @override
  Future<List<FileEntry>> listFiles(String id, String path) async => const [
        FileEntry(
            name: 'world', path: 'world', isDir: true, size: 0, modified: ''),
        FileEntry(
            name: 'server.properties',
            path: 'server.properties',
            isDir: false,
            size: 512,
            modified: '2026-10-04T00:00:00Z'),
        FileEntry(
            name: 'server.jar',
            path: 'server.jar',
            isDir: false,
            size: 50 * 1024 * 1024,
            modified: ''),
      ];

  @override
  Future<String> readFile(String id, String path) async => 'a=1\nb=2\n';

  @override
  Future<void> writeFile(String id, String path, String content) async {}

  @override
  Future<void> mkdir(String id, String path) async {}

  @override
  Future<void> deleteFile(String id, String path) async {}

  @override
  Future<void> renameFile(String id, String from, String to) async {}

  @override
  String fileDownloadUrl(String id, String path) =>
      'http://x/api/servers/$id/files/download?path=$path';

  @override
  Future<void> uploadFile(
      String id, String dir, String name, List<int> bytes) async {}

  @override
  Future<List<Backup>> listBackups(String id) async => const [
        Backup(
            id: 'b1',
            serverId: 's',
            path: '/b/1.zip',
            sizeBytes: 120 * 1024 * 1024,
            note: 'before update',
            createdAt: '2026-10-03T04:00:00Z'),
      ];

  @override
  Future<Backup> createBackup(String id, String note) async => Backup(
      id: 'b2',
      serverId: id,
      path: '/b/2.zip',
      sizeBytes: 1024,
      note: note,
      createdAt: '2026-10-04T00:00:00Z');

  @override
  Future<void> restoreBackup(String id, String bid) async {}

  @override
  Future<void> deleteBackup(String id, String bid) async {}

  @override
  String backupDownloadUrl(String id, String bid) =>
      'http://x/api/servers/$id/backups/$bid/download';

  @override
  Future<List<PlayerEntry>> listPlayers(String id, String list) async =>
      list == 'ops'
          ? const [PlayerEntry(uuid: 'u', name: 'Steve', level: 4)]
          : const [];

  @override
  Future<List<PlayerEntry>> addPlayer(String id, String list, String name) async =>
      [PlayerEntry(uuid: 'x', name: name)];

  @override
  Future<List<PlayerEntry>> removePlayer(
          String id, String list, String name) async =>
      const [];

  @override
  Future<List<Schedule>> listSchedules(String id) async => const [
        Schedule(
            id: 'sc1',
            serverId: 's',
            name: 'nightly restart',
            action: 'restart',
            payload: '',
            everyMinutes: 0,
            dailyTime: '04:00',
            enabled: true),
      ];

  @override
  Future<Schedule> createSchedule(String id, Map<String, Object?> b) async =>
      Schedule(
          id: 'n',
          serverId: id,
          name: b['name'] as String? ?? '',
          action: b['action'] as String? ?? '',
          payload: b['payload'] as String? ?? '',
          everyMinutes: b['every_minutes'] as int? ?? 0,
          dailyTime: b['daily_time'] as String? ?? '',
          enabled: true);

  @override
  Future<Schedule> updateSchedule(
          String id, String sid, Map<String, Object?> b) async =>
      createSchedule(id, b);

  @override
  Future<void> deleteSchedule(String id, String sid) async {}

  @override
  Future<List<ModrinthHit>> searchMods(String id, String query) async => const [
        ModrinthHit(
            projectId: 'p1',
            slug: 'fabric-api',
            title: 'Fabric API',
            description: 'Core library for Fabric mods.',
            downloads: 50000000,
            iconUrl: null,
            projectType: 'mod'),
      ];

  @override
  Future<String> installMod(String id, String project) async =>
      '$project-1.0.jar';

  @override
  Future<List<InstalledFile>> listMods(String id) async =>
      const [InstalledFile(name: 'fabric-api.jar', size: 2048)];

  @override
  Future<List<ServerTypeInfo>> serverTypes() async => const [
        ServerTypeInfo(
            id: 'vanilla', name: 'Vanilla', hasLoader: false, contentDir: 'mods'),
        ServerTypeInfo(
            id: 'paper', name: 'Paper', hasLoader: false, contentDir: 'plugins'),
        ServerTypeInfo(
            id: 'fabric', name: 'Fabric', hasLoader: true, contentDir: 'mods'),
        ServerTypeInfo(
            id: 'forge', name: 'Forge', hasLoader: true, contentDir: 'mods'),
      ];

  @override
  Future<List<String>> mcVersions(String type) async =>
      ['1.21.1', '1.21', '1.20.4'];

  @override
  Future<List<String>> loaders(String type, String mc) async =>
      type == 'fabric' ? ['0.16.5', '0.16.4'] : [];

  @override
  Future<int> requiredJava(String mc) async => 21;

  @override
  Future<List<JavaInstall>> listJava() async => const [
        JavaInstall(
            path: '/usr/bin/java', major: 21, version: '21.0.4', managed: false),
      ];

  @override
  Future<String> installJava(int major) async => '/data/java/jdk-$major/bin/java';

  @override
  Future<void> deleteJava(String id) async {}

  @override
  Future<TailscaleStatus> tailscaleStatus() async => const TailscaleStatus(
      installed: true,
      running: true,
      ip: '100.64.1.5',
      hostname: 'homelab.tail.ts.net',
      tailnet: 'example.com',
      panelServePort: 443,
      panelServeEnabled: false);

  @override
  Future<void> tailscalePanelServe(bool enable, {int? httpsPort}) async {}

  @override
  Future<void> tailscaleServer(String id, bool enable, {int? tailnetPort}) async {}

  @override
  Future<Map<String, String>> getSettings() async => {'theme': 'dark'};

  @override
  Future<void> putSettings(Map<String, String> s) async {}

  @override
  Future<List<AuditEntry>> auditLog() async => const [
        AuditEntry(
            id: 1,
            ts: '2026-10-04T00:00:00Z',
            actor: 'owner',
            action: 'server_start',
            detail: 'srv-1'),
      ];

  @override
  Stream<ConsoleEvent> consoleStream(String serverId) => _console.stream;

  @override
  Stream<PanelEvent> eventsStream() => _events.stream;

  /// Encode a console history frame as the backend does (for stream tests).
  static String frame(Map<String, Object?> v) => jsonEncode(v);
}
