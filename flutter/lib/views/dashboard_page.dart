import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../api/types.dart';
import '../state/app_state.dart';
import '../widgets.dart';
import 'create_server_page.dart';
import 'server_detail_page.dart';
import 'settings_page.dart';

/// Home dashboard: host stats + server grid + create button.
class DashboardPage extends StatelessWidget {
  const DashboardPage({super.key});

  @override
  Widget build(BuildContext context) {
    final app = context.watch<AppState>();
    return Scaffold(
      appBar: AppBar(
        title: Row(children: [
          Image.asset('assets/icon.png', width: 28, height: 28),
          const SizedBox(width: 10),
          const Text('mcst',
              style: TextStyle(fontWeight: FontWeight.w800)),
        ]),
        actions: [
          if (app.stats.hostname.isNotEmpty)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12),
              child: Center(
                child: Text(app.stats.hostname,
                    style: Theme.of(context).textTheme.labelMedium?.copyWith(
                        color:
                            Theme.of(context).colorScheme.onSurfaceVariant)),
              ),
            ),
          IconButton(
            tooltip: 'Settings',
            icon: const Icon(Icons.settings_outlined),
            onPressed: () => Navigator.of(context).push(
                MaterialPageRoute(builder: (_) => const SettingsPage())),
          ),
          IconButton(
            tooltip: 'Sign out',
            icon: const Icon(Icons.logout),
            onPressed: () => app.logout(),
          ),
        ],
      ),
      body: PageBody(
        child: ListView(
          padding: const EdgeInsets.all(20),
          children: [
            _StatsRow(stats: app.stats, app: app),
            const SizedBox(height: 20),
            Row(
              children: [
                Text('Servers',
                    style: Theme.of(context).textTheme.titleLarge),
                const Spacer(),
                FilledButton.icon(
                  icon: const Icon(Icons.add),
                  label: const Text('New server'),
                  onPressed: () => Navigator.of(context).push(
                      MaterialPageRoute(
                          builder: (_) => const CreateServerPage())),
                ),
              ],
            ),
            const SizedBox(height: 12),
            if (app.lastError != null)
              ErrorCard(app.lastError!, onRetry: app.refreshServers)
            else if (app.servers.isEmpty)
              const _EmptyState()
            else
              LayoutBuilder(builder: (context, c) {
                final cols = c.maxWidth > 900 ? 3 : c.maxWidth > 600 ? 2 : 1;
                return GridView.count(
                  crossAxisCount: cols,
                  shrinkWrap: true,
                  physics: const NeverScrollableScrollPhysics(),
                  mainAxisSpacing: 12,
                  crossAxisSpacing: 12,
                  // Fixed height — on portrait windows a 1-column grid would
                  // otherwise stretch each card with the full width.
                  mainAxisExtent: 132,
                  children: [
                    for (final s in app.servers)
                      ServerCard(
                        server: s,
                        onTap: () => Navigator.of(context).push(
                            MaterialPageRoute(
                                builder: (_) =>
                                    ServerDetailPage(serverId: s.id))),
                      ),
                  ],
                );
              }),
          ],
        ),
      ),
    );
  }
}

class _StatsRow extends StatelessWidget {
  final SystemStats stats;
  final AppState app;
  const _StatsRow({required this.stats, required this.app});

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(builder: (context, c) {
      final cols = c.maxWidth > 900 ? 4 : 2;
      return GridView.count(
        crossAxisCount: cols,
        shrinkWrap: true,
        physics: const NeverScrollableScrollPhysics(),
        mainAxisSpacing: 12,
        crossAxisSpacing: 12,
        mainAxisExtent: 128,
        children: [
          StatCard(Icons.memory, 'CPU',
              '${stats.cpuPercent.toStringAsFixed(0)}%',
              subtitle: '${stats.cpuCount} cores'),
          StatCard(Icons.storage, 'Memory',
              humanBytes(stats.memUsed),
              subtitle: 'of ${humanBytes(stats.memTotal)}'),
          StatCard(Icons.dns, 'Running',
              '${app.runningCount}/${app.servers.length}',
              subtitle: '${app.totalPlayers} players online'),
          StatCard(Icons.schedule, 'Uptime',
              humanDuration(stats.uptimeSec),
              subtitle: stats.os.isEmpty ? '' : stats.os),
        ],
      );
    });
  }
}

class _EmptyState extends StatelessWidget {
  const _EmptyState();

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 56),
        child: Column(
          children: [
            Icon(Icons.dns_outlined,
                size: 48, color: Theme.of(context).colorScheme.outline),
            const SizedBox(height: 12),
            Text('No servers yet',
                style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 4),
            Text('Create your first Minecraft server to get started.',
                style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                    color: Theme.of(context).colorScheme.onSurfaceVariant)),
          ],
        ),
      ),
    );
  }
}
