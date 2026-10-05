import 'package:flutter_test/flutter_test.dart';
import 'package:mcst_frontend/api/types.dart';

void main() {
  group('ServerStatus', () {
    test('parses all backend values', () {
      expect(ServerStatus.parse('running'), ServerStatus.running);
      expect(ServerStatus.parse('stopped'), ServerStatus.stopped);
      expect(ServerStatus.parse('crashed'), ServerStatus.crashed);
      expect(ServerStatus.parse('installing'), ServerStatus.installing);
      expect(ServerStatus.parse('stopping'), ServerStatus.stopping);
      expect(ServerStatus.parse('garbage'), ServerStatus.stopped);
      expect(ServerStatus.parse(null), ServerStatus.stopped);
    });

    test('isActive covers transitional states', () {
      expect(ServerStatus.running.isActive, isTrue);
      expect(ServerStatus.starting.isActive, isTrue);
      expect(ServerStatus.stopping.isActive, isTrue);
      expect(ServerStatus.stopped.isActive, isFalse);
      expect(ServerStatus.crashed.isActive, isFalse);
    });
  });

  group('ServerDto', () {
    test('fromJson maps snake_case fields', () {
      final s = ServerDto.fromJson({
        'id': 'a', 'name': 'SMP', 'server_type': 'paper',
        'mc_version': '1.21', 'loader_version': '130',
        'port': 25565, 'memory_mb': 4096, 'min_memory_mb': 512,
        'java_path': '/j/21/bin/java', 'jvm_args': '-XX:+UseZGC',
        'dir': '/data/servers/a', 'jar': 'server.jar',
        'auto_start': 1, 'restart_on_crash': 0, 'shutdown_timeout_sec': 45,
        'empty_stop_minutes': 15, 'icon': '', 'created_at': 't', 'updated_at': 'u',
        'status': 'running', 'players_online': 3, 'players_max': 20,
        'player_names': ['Steve'], 'cpu_percent': 12.5,
        'mem_bytes': 1024, 'uptime_sec': 60, 'last_exit_code': 0,
      });
      expect(s.id, 'a');
      expect(s.typeLabel, 'Paper 1.21 (130)');
      expect(s.autoStart, isTrue);
      expect(s.restartOnCrash, isFalse);
      expect(s.playersOnline, 3);
      expect(s.playerNames, ['Steve']);
      expect(s.lastExitCode, 0);
      expect(s.supportsMods, isTrue);
    });

    test('fromJson tolerates missing fields', () {
      final s = ServerDto.fromJson({});
      expect(s.status, ServerStatus.stopped);
      expect(s.memoryMb, 0);
      expect(s.playerNames, isEmpty);
      expect(s.typeLabel, ' ');
      expect(s.supportsMods, isFalse);
    });

    test('typeLabel without loader', () {
      final s = ServerDto.fromJson({'server_type': 'vanilla', 'mc_version': '1.21'});
      expect(s.typeLabel, 'Vanilla 1.21');
    });

    test('copyWith overrides only given fields', () {
      final s = ServerDto.fromJson({'id': 'x', 'status': 'stopped'});
      final r = s.copyWith(status: ServerStatus.running, cpuPercent: 5);
      expect(r.status, ServerStatus.running);
      expect(r.cpuPercent, 5);
      expect(r.memBytes, s.memBytes);
      expect(r.id, 'x');
    });
  });

  group('SystemStats', () {
    test('fromJson', () {
      final s = SystemStats.fromJson({
        'cpu_percent': 42.5, 'cpu_count': 8, 'mem_total': 1000,
        'mem_used': 500, 'mem_available': 500, 'swap_total': 0,
        'swap_used': 0, 'disk_total': 2000, 'disk_used': 100,
        'uptime_sec': 3661, 'hostname': 'h', 'os': 'os', 'kernel': 'k',
        'arch': 'x86_64', 'load_avg': [1.0, 0.5, 0.25], 'data_dir_bytes': 1,
      });
      expect(s.cpuPercent, 42.5);
      expect(s.loadAvg, [1.0, 0.5, 0.25]);
      expect(s.hostname, 'h');
    });
    test('empty + partial json', () {
      expect(SystemStats.empty.memTotal, 0);
      final s = SystemStats.fromJson({'cpu_count': 4});
      expect(s.cpuCount, 4);
      expect(s.loadAvg.length, 3);
    });
  });

  group('small models', () {
    test('FileEntry', () {
      final f = FileEntry.fromJson(
          {'name': 'a', 'path': 'x/a', 'is_dir': true, 'size': 5, 'modified': 'm'});
      expect(f.isDir, isTrue);
      expect(f.path, 'x/a');
    });
    test('Backup', () {
      final b = Backup.fromJson(
          {'id': 'b', 'server_id': 's', 'path': 'p', 'size_bytes': 9, 'note': 'n', 'created_at': 't'});
      expect(b.note, 'n');
    });
    test('Schedule + whenLabel', () {
      final s = Schedule.fromJson({
        'id': 'x', 'server_id': 's', 'name': 'n', 'action': 'restart',
        'payload': '', 'every_minutes': 60, 'daily_time': '', 'enabled': 1,
      });
      expect(s.enabled, isTrue);
      expect(s.whenLabel, 'every 1h');
      expect(
          Schedule.fromJson({'every_minutes': 30, 'daily_time': ''}).whenLabel,
          'every 30m');
      expect(
          Schedule.fromJson({'every_minutes': 2880, 'daily_time': ''}).whenLabel,
          'every 2d');
      expect(
          Schedule.fromJson({'every_minutes': 0, 'daily_time': '04:00'}).whenLabel,
          'daily at 04:00');
    });
    test('PlayerEntry', () {
      final p = PlayerEntry.fromJson(
          {'uuid': 'u', 'name': 'Steve', 'level': 4, 'reason': 'griefing'});
      expect(p.level, 4);
      expect(p.reason, 'griefing');
    });
    test('ModrinthHit', () {
      final h = ModrinthHit.fromJson({
        'project_id': 'p', 'slug': 's', 'title': 't', 'description': 'd',
        'downloads': 42, 'icon_url': 'u', 'project_type': 'mod',
      });
      expect(h.downloads, 42);
      expect(h.iconUrl, 'u');
    });
    test('JavaInstall', () {
      final j = JavaInstall.fromJson(
          {'path': '/j', 'major': 21, 'version': '21.0.4', 'managed': true});
      expect(j.managed, isTrue);
      expect(j.major, 21);
    });
    test('TailscaleStatus from nested json + empty', () {
      final t = TailscaleStatus.fromJson({
        'status': {
          'installed': true, 'running': true, 'ip': '100.x',
          'hostname': 'h.tn', 'tailnet': 'tn', 'panel_serve_port': 443,
        },
        'panel_serve_enabled': true,
      });
      expect(t.running, isTrue);
      expect(t.ip, '100.x');
      expect(t.panelServeEnabled, isTrue);
      expect(TailscaleStatus.empty.installed, isFalse);
    });
    test('ServerTypeInfo + AuditEntry', () {
      final t = ServerTypeInfo.fromJson(
          {'id': 'paper', 'name': 'Paper', 'has_loader': true, 'content_dir': 'plugins'});
      expect(t.hasLoader, isTrue);
      final a = AuditEntry.fromJson(
          {'id': 3, 'ts': 't', 'actor': 'u', 'action': 'x', 'detail': 'd'});
      expect(a.id, 3);
      expect(a.detail, 'd');
    });
  });

  group('formatters', () {
    test('humanBytes', () {
      expect(humanBytes(0), '0 B');
      expect(humanBytes(512), '512 B');
      expect(humanBytes(2048), '2.0 KiB');
      expect(humanBytes(3 * 1024 * 1024 * 1024), '3.0 GiB');
    });
    test('humanDuration', () {
      expect(humanDuration(30), '30s');
      expect(humanDuration(90), '1m 30s');
      expect(humanDuration(3700), '1h 1m');
      expect(humanDuration(90000), '1d 1h');
    });
  });
}
