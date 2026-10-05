/// API models matching the mcst Rust backend.
library;

/// Lifecycle status of a server process.
enum ServerStatus {
  stopped,
  installing,
  starting,
  running,
  stopping,
  crashed;

  static ServerStatus parse(String? s) => switch (s) {
        'installing' => ServerStatus.installing,
        'starting' => ServerStatus.starting,
        'running' => ServerStatus.running,
        'stopping' => ServerStatus.stopping,
        'crashed' => ServerStatus.crashed,
        _ => ServerStatus.stopped,
      };

  String get label => name;
  bool get isActive =>
      this == starting || this == running || this == stopping;
}

/// A managed Minecraft server (record + live runtime fields).
class ServerDto {
  final String id;
  final String name;
  final String serverType;
  final String mcVersion;
  final String loaderVersion;
  final int port;
  final int memoryMb;
  final int minMemoryMb;
  final String javaPath;
  final String jvmArgs;
  final String dir;
  final String jar;
  final bool autoStart;
  final bool restartOnCrash;
  final int shutdownTimeoutSec;
  final int emptyStopMinutes;
  final String icon;
  final String createdAt;
  final ServerStatus status;
  final int playersOnline;
  final int playersMax;
  final List<String> playerNames;
  final double cpuPercent;
  final int memBytes;
  final int uptimeSec;
  final int? lastExitCode;

  const ServerDto({
    required this.id,
    required this.name,
    required this.serverType,
    required this.mcVersion,
    required this.loaderVersion,
    required this.port,
    required this.memoryMb,
    required this.minMemoryMb,
    required this.javaPath,
    required this.jvmArgs,
    required this.dir,
    required this.jar,
    required this.autoStart,
    required this.restartOnCrash,
    required this.shutdownTimeoutSec,
    required this.emptyStopMinutes,
    required this.icon,
    required this.createdAt,
    required this.status,
    required this.playersOnline,
    required this.playersMax,
    required this.playerNames,
    required this.cpuPercent,
    required this.memBytes,
    required this.uptimeSec,
    this.lastExitCode,
  });

  factory ServerDto.fromJson(Map<String, dynamic> j) => ServerDto(
        id: j['id'] as String? ?? '',
        name: j['name'] as String? ?? '',
        serverType: j['server_type'] as String? ?? '',
        mcVersion: j['mc_version'] as String? ?? '',
        loaderVersion: j['loader_version'] as String? ?? '',
        port: (j['port'] as num?)?.toInt() ?? 0,
        memoryMb: (j['memory_mb'] as num?)?.toInt() ?? 0,
        minMemoryMb: (j['min_memory_mb'] as num?)?.toInt() ?? 0,
        javaPath: j['java_path'] as String? ?? 'java',
        jvmArgs: j['jvm_args'] as String? ?? '',
        dir: j['dir'] as String? ?? '',
        jar: j['jar'] as String? ?? 'server.jar',
        autoStart: (j['auto_start'] as num?)?.toInt() == 1,
        restartOnCrash: (j['restart_on_crash'] as num?)?.toInt() == 1,
        shutdownTimeoutSec: (j['shutdown_timeout_sec'] as num?)?.toInt() ?? 30,
        emptyStopMinutes: (j['empty_stop_minutes'] as num?)?.toInt() ?? 0,
        icon: j['icon'] as String? ?? '',
        createdAt: j['created_at'] as String? ?? '',
        status: ServerStatus.parse(j['status'] as String?),
        playersOnline: (j['players_online'] as num?)?.toInt() ?? 0,
        playersMax: (j['players_max'] as num?)?.toInt() ?? 0,
        playerNames: (j['player_names'] as List?)
                ?.map((e) => e.toString())
                .toList() ??
            const [],
        cpuPercent: (j['cpu_percent'] as num?)?.toDouble() ?? 0,
        memBytes: (j['mem_bytes'] as num?)?.toInt() ?? 0,
        uptimeSec: (j['uptime_sec'] as num?)?.toInt() ?? 0,
        lastExitCode: (j['last_exit_code'] as num?)?.toInt(),
      );

  /// "Paper 1.21" style label.
  String get typeLabel {
    const names = {
      'vanilla': 'Vanilla',
      'paper': 'Paper',
      'purpur': 'Purpur',
      'fabric': 'Fabric',
      'forge': 'Forge',
      'neoforge': 'NeoForge',
      'custom': 'Custom',
    };
    final t = names[serverType] ?? serverType;
    return loaderVersion.isEmpty ? '$t $mcVersion' : '$t $mcVersion ($loaderVersion)';
  }

  /// Whether this type supports Modrinth content.
  bool get supportsMods => const {
        'paper',
        'purpur',
        'fabric',
        'forge',
        'neoforge',
      }.contains(serverType);

  ServerDto copyWith({
    ServerStatus? status,
    int? playersOnline,
    int? playersMax,
    List<String>? playerNames,
    double? cpuPercent,
    int? memBytes,
    int? uptimeSec,
    int? lastExitCode,
  }) =>
      ServerDto(
        id: id,
        name: name,
        serverType: serverType,
        mcVersion: mcVersion,
        loaderVersion: loaderVersion,
        port: port,
        memoryMb: memoryMb,
        minMemoryMb: minMemoryMb,
        javaPath: javaPath,
        jvmArgs: jvmArgs,
        dir: dir,
        jar: jar,
        autoStart: autoStart,
        restartOnCrash: restartOnCrash,
        shutdownTimeoutSec: shutdownTimeoutSec,
        emptyStopMinutes: emptyStopMinutes,
        icon: icon,
        createdAt: createdAt,
        status: status ?? this.status,
        playersOnline: playersOnline ?? this.playersOnline,
        playersMax: playersMax ?? this.playersMax,
        playerNames: playerNames ?? this.playerNames,
        cpuPercent: cpuPercent ?? this.cpuPercent,
        memBytes: memBytes ?? this.memBytes,
        uptimeSec: uptimeSec ?? this.uptimeSec,
        lastExitCode: lastExitCode ?? this.lastExitCode,
      );
}

/// Host system metrics.
class SystemStats {
  final double cpuPercent;
  final int cpuCount;
  final int memTotal;
  final int memUsed;
  final int memAvailable;
  final int swapTotal;
  final int swapUsed;
  final int diskTotal;
  final int diskUsed;
  final int uptimeSec;
  final String hostname;
  final String os;
  final String kernel;
  final String arch;
  final List<double> loadAvg;
  final int dataDirBytes;

  const SystemStats({
    required this.cpuPercent,
    required this.cpuCount,
    required this.memTotal,
    required this.memUsed,
    required this.memAvailable,
    required this.swapTotal,
    required this.swapUsed,
    required this.diskTotal,
    required this.diskUsed,
    required this.uptimeSec,
    required this.hostname,
    required this.os,
    required this.kernel,
    required this.arch,
    required this.loadAvg,
    required this.dataDirBytes,
  });

  factory SystemStats.fromJson(Map<String, dynamic> j) => SystemStats(
        cpuPercent: (j['cpu_percent'] as num?)?.toDouble() ?? 0,
        cpuCount: (j['cpu_count'] as num?)?.toInt() ?? 0,
        memTotal: (j['mem_total'] as num?)?.toInt() ?? 0,
        memUsed: (j['mem_used'] as num?)?.toInt() ?? 0,
        memAvailable: (j['mem_available'] as num?)?.toInt() ?? 0,
        swapTotal: (j['swap_total'] as num?)?.toInt() ?? 0,
        swapUsed: (j['swap_used'] as num?)?.toInt() ?? 0,
        diskTotal: (j['disk_total'] as num?)?.toInt() ?? 0,
        diskUsed: (j['disk_used'] as num?)?.toInt() ?? 0,
        uptimeSec: (j['uptime_sec'] as num?)?.toInt() ?? 0,
        hostname: j['hostname'] as String? ?? '',
        os: j['os'] as String? ?? '',
        kernel: j['kernel'] as String? ?? '',
        arch: j['arch'] as String? ?? '',
        loadAvg: (j['load_avg'] as List?)
                ?.map((e) => (e as num).toDouble())
                .toList() ??
            const [0, 0, 0],
        dataDirBytes: (j['data_dir_bytes'] as num?)?.toInt() ?? 0,
      );

  static const empty = SystemStats(
    cpuPercent: 0,
    cpuCount: 0,
    memTotal: 0,
    memUsed: 0,
    memAvailable: 0,
    swapTotal: 0,
    swapUsed: 0,
    diskTotal: 0,
    diskUsed: 0,
    uptimeSec: 0,
    hostname: '',
    os: '',
    kernel: '',
    arch: '',
    loadAvg: [0, 0, 0],
    dataDirBytes: 0,
  );
}

class FileEntry {
  final String name;
  final String path;
  final bool isDir;
  final int size;
  final String modified;

  const FileEntry({
    required this.name,
    required this.path,
    required this.isDir,
    required this.size,
    required this.modified,
  });

  factory FileEntry.fromJson(Map<String, dynamic> j) => FileEntry(
        name: j['name'] as String? ?? '',
        path: j['path'] as String? ?? '',
        isDir: j['is_dir'] as bool? ?? false,
        size: (j['size'] as num?)?.toInt() ?? 0,
        modified: j['modified'] as String? ?? '',
      );
}

class Backup {
  final String id;
  final String serverId;
  final String path;
  final int sizeBytes;
  final String note;
  final String createdAt;

  const Backup({
    required this.id,
    required this.serverId,
    required this.path,
    required this.sizeBytes,
    required this.note,
    required this.createdAt,
  });

  factory Backup.fromJson(Map<String, dynamic> j) => Backup(
        id: j['id'] as String? ?? '',
        serverId: j['server_id'] as String? ?? '',
        path: j['path'] as String? ?? '',
        sizeBytes: (j['size_bytes'] as num?)?.toInt() ?? 0,
        note: j['note'] as String? ?? '',
        createdAt: j['created_at'] as String? ?? '',
      );
}

class Schedule {
  final String id;
  final String serverId;
  final String name;
  final String action;
  final String payload;
  final int everyMinutes;
  final String dailyTime;
  final bool enabled;
  final String? lastRunAt;

  const Schedule({
    required this.id,
    required this.serverId,
    required this.name,
    required this.action,
    required this.payload,
    required this.everyMinutes,
    required this.dailyTime,
    required this.enabled,
    this.lastRunAt,
  });

  factory Schedule.fromJson(Map<String, dynamic> j) => Schedule(
        id: j['id'] as String? ?? '',
        serverId: j['server_id'] as String? ?? '',
        name: j['name'] as String? ?? '',
        action: j['action'] as String? ?? '',
        payload: j['payload'] as String? ?? '',
        everyMinutes: (j['every_minutes'] as num?)?.toInt() ?? 0,
        dailyTime: j['daily_time'] as String? ?? '',
        enabled: (j['enabled'] as num?)?.toInt() == 1,
        lastRunAt: j['last_run_at'] as String?,
      );

  /// "every 30m" / "daily 04:00" summary.
  String get whenLabel {
    if (everyMinutes > 0) {
      if (everyMinutes >= 1440) return 'every ${everyMinutes ~/ 1440}d';
      if (everyMinutes >= 60) return 'every ${everyMinutes ~/ 60}h';
      return 'every ${everyMinutes}m';
    }
    return 'daily at $dailyTime';
  }
}

class PlayerEntry {
  final String uuid;
  final String name;
  final int? level;
  final String? reason;

  const PlayerEntry({
    required this.uuid,
    required this.name,
    this.level,
    this.reason,
  });

  factory PlayerEntry.fromJson(Map<String, dynamic> j) => PlayerEntry(
        uuid: j['uuid'] as String? ?? '',
        name: j['name'] as String? ?? '',
        level: (j['level'] as num?)?.toInt(),
        reason: j['reason'] as String?,
      );
}

class ModrinthHit {
  final String projectId;
  final String slug;
  final String title;
  final String description;
  final int downloads;
  final String? iconUrl;
  final String projectType;

  const ModrinthHit({
    required this.projectId,
    required this.slug,
    required this.title,
    required this.description,
    required this.downloads,
    this.iconUrl,
    required this.projectType,
  });

  factory ModrinthHit.fromJson(Map<String, dynamic> j) => ModrinthHit(
        projectId: j['project_id'] as String? ?? '',
        slug: j['slug'] as String? ?? '',
        title: j['title'] as String? ?? '',
        description: j['description'] as String? ?? '',
        downloads: (j['downloads'] as num?)?.toInt() ?? 0,
        iconUrl: j['icon_url'] as String?,
        projectType: j['project_type'] as String? ?? '',
      );
}

class JavaInstall {
  final String id;
  final String path;
  final int major;
  final String version;
  final bool managed;

  const JavaInstall({
    this.id = '',
    required this.path,
    required this.major,
    required this.version,
    required this.managed,
  });

  factory JavaInstall.fromJson(Map<String, dynamic> j) => JavaInstall(
        id: j['id'] as String? ?? '',
        path: j['path'] as String? ?? '',
        major: (j['major'] as num?)?.toInt() ?? 0,
        version: j['version'] as String? ?? '',
        managed: j['managed'] as bool? ?? false,
      );
}

class TailscaleStatus {
  final bool installed;
  final bool running;
  final String ip;
  final String hostname;
  final String tailnet;
  final int panelServePort;
  final bool panelServeEnabled;

  const TailscaleStatus({
    required this.installed,
    required this.running,
    required this.ip,
    required this.hostname,
    required this.tailnet,
    required this.panelServePort,
    required this.panelServeEnabled,
  });

  factory TailscaleStatus.fromJson(Map<String, dynamic> j) {
    final st = j['status'] as Map<String, dynamic>? ?? const {};
    return TailscaleStatus(
      installed: st['installed'] as bool? ?? false,
      running: st['running'] as bool? ?? false,
      ip: st['ip'] as String? ?? '',
      hostname: st['hostname'] as String? ?? '',
      tailnet: st['tailnet'] as String? ?? '',
      panelServePort: (st['panel_serve_port'] as num?)?.toInt() ?? 443,
      panelServeEnabled: j['panel_serve_enabled'] as bool? ?? false,
    );
  }

  static const empty = TailscaleStatus(
    installed: false,
    running: false,
    ip: '',
    hostname: '',
    tailnet: '',
    panelServePort: 443,
    panelServeEnabled: false,
  );
}

class ServerTypeInfo {
  final String id;
  final String name;
  final bool hasLoader;
  final String contentDir;

  const ServerTypeInfo({
    required this.id,
    required this.name,
    required this.hasLoader,
    required this.contentDir,
  });

  factory ServerTypeInfo.fromJson(Map<String, dynamic> j) => ServerTypeInfo(
        id: j['id'] as String? ?? '',
        name: j['name'] as String? ?? '',
        hasLoader: j['has_loader'] as bool? ?? false,
        contentDir: j['content_dir'] as String? ?? 'mods',
      );
}

class AuditEntry {
  final int id;
  final String ts;
  final String actor;
  final String action;
  final String detail;

  const AuditEntry({
    required this.id,
    required this.ts,
    required this.actor,
    required this.action,
    required this.detail,
  });

  factory AuditEntry.fromJson(Map<String, dynamic> j) => AuditEntry(
        id: (j['id'] as num?)?.toInt() ?? 0,
        ts: j['ts'] as String? ?? '',
        actor: j['actor'] as String? ?? '',
        action: j['action'] as String? ?? '',
        detail: j['detail'] as String? ?? '',
      );
}

/// Human-readable byte formatter.
String humanBytes(int bytes) {
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
  var v = bytes.toDouble();
  var i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  if (i == 0) return '$bytes B';
  return '${v.toStringAsFixed(1)} ${units[i]}';
}

/// Human-readable uptime ("2d 4h", "5m 12s").
String humanDuration(int secs) {
  final d = secs ~/ 86400;
  final h = secs % 86400 ~/ 3600;
  final m = secs % 3600 ~/ 60;
  if (d > 0) return '${d}d ${h}h';
  if (h > 0) return '${h}h ${m}m';
  if (m > 0) return '${m}m ${secs % 60}s';
  return '${secs}s';
}
