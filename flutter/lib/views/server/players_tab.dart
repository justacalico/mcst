import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../api/client.dart';
import '../../api/types.dart';
import '../../state/app_state.dart';
import '../../widgets.dart';

/// Ops / whitelist / bans management.
class PlayersTab extends StatefulWidget {
  final ServerDto server;
  const PlayersTab({super.key, required this.server});

  @override
  State<PlayersTab> createState() => _PlayersTabState();
}

class _PlayersTabState extends State<PlayersTab>
    with SingleTickerProviderStateMixin {
  late TabController _inner;
  final _lists = <String, List<PlayerEntry>>{
    'ops': const [],
    'whitelist': const [],
    'bans': const [],
  };

  ApiClient get api => context.read<AppState>().api;

  @override
  void initState() {
    super.initState();
    _inner = TabController(length: 3, vsync: this);
    _loadAll();
  }

  Future<void> _loadAll() async {
    for (final k in _lists.keys) {
      try {
        _lists[k] = await api.listPlayers(widget.server.id, k);
      } catch (_) {}
    }
    if (mounted) setState(() {});
  }

  Future<void> _add(String list) async {
    final c = TextEditingController();
    final name = await showDialog<String>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text('Add to $list'),
        content: TextField(
            controller: c,
            autofocus: true,
            decoration: const InputDecoration(labelText: 'Player name')),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(ctx), child: const Text('Cancel')),
          FilledButton(
              onPressed: () => Navigator.pop(ctx, c.text.trim()),
              child: const Text('Add')),
        ],
      ),
    );
    if (name == null || name.isEmpty) return;
    try {
      _lists[list] = await api.addPlayer(widget.server.id, list, name);
      setState(() {});
    } catch (e) {
      if (mounted) showError(context, e);
    }
  }

  Future<void> _remove(String list, String name) async {
    try {
      _lists[list] = await api.removePlayer(widget.server.id, list, name);
      setState(() {});
    } catch (e) {
      if (mounted) showError(context, e);
    }
  }

  @override
  void dispose() {
    _inner.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final s = widget.server;
    return Column(
      children: [
        if (s.playerNames.isNotEmpty)
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 12, 16, 0),
            child: Card(
              child: Padding(
                padding: const EdgeInsets.all(12),
                child: Row(children: [
                  const Icon(Icons.wifi_tethering, size: 16),
                  const SizedBox(width: 8),
                  Text('Online now: ${s.playerNames.join(", ")}',
                      style: Theme.of(context).textTheme.bodyMedium),
                ]),
              ),
            ),
          ),
        TabBar.secondary(
          controller: _inner,
          tabs: const [
            Tab(text: 'Ops'),
            Tab(text: 'Whitelist'),
            Tab(text: 'Bans'),
          ],
        ),
        Expanded(
          child: TabBarView(
            controller: _inner,
            children: [
              for (final k in _lists.keys) _listView(k),
            ],
          ),
        ),
      ],
    );
  }

  Widget _listView(String kind) {
    final entries = _lists[kind]!;
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.all(12),
          child: Row(children: [
            Text('${entries.length} $kind',
                style: Theme.of(context).textTheme.labelLarge),
            const Spacer(),
            FilledButton.tonalIcon(
                onPressed: () => _add(kind),
                icon: const Icon(Icons.person_add, size: 16),
                label: const Text('Add')),
          ]),
        ),
        Expanded(
          child: entries.isEmpty
              ? Center(
                  child: Text('Nobody on the $kind list',
                      style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                          color: Theme.of(context)
                              .colorScheme
                              .onSurfaceVariant)))
              : ListView.builder(
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                  itemCount: entries.length,
                  itemBuilder: (context, i) {
                    final p = entries[i];
                    return ListTile(
                      dense: true,
                      leading: CircleAvatar(
                          radius: 14, child: Text(p.name[0].toUpperCase())),
                      title: Text(p.name),
                      subtitle: p.reason != null ? Text(p.reason!) : null,
                      trailing: IconButton(
                          icon: const Icon(Icons.remove_circle_outline,
                              size: 18),
                          onPressed: () => _remove(kind, p.name)),
                    );
                  },
                ),
        ),
      ],
    );
  }
}
