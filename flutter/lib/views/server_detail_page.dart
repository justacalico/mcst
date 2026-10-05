import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../api/types.dart';
import '../state/app_state.dart';
import '../widgets.dart';
import 'server/backups_tab.dart';
import 'server/console_tab.dart';
import 'server/files_tab.dart';
import 'server/mods_tab.dart';
import 'server/players_tab.dart';
import 'server/schedules_tab.dart';
import 'server/settings_tab.dart';

/// Server detail: header with lifecycle controls + tabbed sections.
class ServerDetailPage extends StatefulWidget {
  final String serverId;
  const ServerDetailPage({super.key, required this.serverId});

  @override
  State<ServerDetailPage> createState() => _ServerDetailPageState();
}

class _ServerDetailPageState extends State<ServerDetailPage>
    with TickerProviderStateMixin {
  late TabController _tabs;

  @override
  void initState() {
    super.initState();
    _tabs = TabController(length: 7, vsync: this);
    _refresh();
  }

  Future<void> _refresh() async {
    try {
      final s = await context.read<AppState>().api.getServer(widget.serverId);
      if (!mounted) return;
      context.read<AppState>().patchServer(s);
    } catch (_) {}
  }

  @override
  void dispose() {
    _tabs.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final app = context.watch<AppState>();
    final server = app.servers.where((s) => s.id == widget.serverId).firstOrNull;
    if (server == null) {
      return Scaffold(
        appBar: AppBar(),
        body: const Center(child: CircularProgressIndicator()),
      );
    }
    final running = server.status == ServerStatus.running;
    final active = server.status.isActive;
    return Scaffold(
      appBar: AppBar(
        title: Row(children: [
          ServerTypeAvatar(server.serverType, size: 30),
          const SizedBox(width: 10),
          Flexible(
            child: Text(server.name, overflow: TextOverflow.ellipsis),
          ),
          const SizedBox(width: 10),
          StatusBadge(server.status.name),
        ]),
        actions: [
          if (!active)
            IconButton(
                tooltip: 'Start',
                icon: const Icon(Icons.play_arrow),
                onPressed: () => app.lifecycle(server.id, 'start')),
          if (running)
            IconButton(
                tooltip: 'Restart',
                icon: const Icon(Icons.restart_alt),
                onPressed: () => app.lifecycle(server.id, 'restart')),
          if (active)
            IconButton(
                tooltip: 'Stop',
                icon: const Icon(Icons.stop),
                onPressed: () => app.lifecycle(server.id, 'stop')),
          if (active)
            IconButton(
                tooltip: 'Kill',
                icon: const Icon(Icons.bolt),
                onPressed: () => _confirmKill(context, server)),
          IconButton(
              tooltip: 'Refresh',
              icon: const Icon(Icons.refresh),
              onPressed: _refresh),
        ],
        bottom: TabBar(
          controller: _tabs,
          isScrollable: true,
          tabs: const [
            Tab(text: 'Console'),
            Tab(text: 'Files'),
            Tab(text: 'Players'),
            Tab(text: 'Content'),
            Tab(text: 'Backups'),
            Tab(text: 'Schedules'),
            Tab(text: 'Settings'),
          ],
        ),
      ),
      body: TabBarView(
        controller: _tabs,
        children: [
          ConsoleTab(server: server),
          FilesTab(server: server),
          PlayersTab(server: server),
          ModsTab(server: server),
          BackupsTab(server: server),
          SchedulesTab(server: server),
          ServerSettingsTab(server: server),
        ],
      ),
    );
  }

  Future<void> _confirmKill(BuildContext context, ServerDto s) async {
    final app = context.read<AppState>();
    final ok = await showDialog<bool>(
      context: context,
      builder: (c) => AlertDialog(
        title: const Text('Kill server?'),
        content: const Text(
            'Sends SIGKILL — unsaved world changes will be lost.'),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(c, false),
              child: const Text('Cancel')),
          FilledButton(
              onPressed: () => Navigator.pop(c, true),
              child: const Text('Kill')),
        ],
      ),
    );
    if (ok == true) await app.lifecycle(s.id, 'kill');
  }
}
