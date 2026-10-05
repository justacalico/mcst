import 'dart:async';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../api/client.dart';
import '../../api/types.dart';
import '../../state/app_state.dart';
import '../../widgets.dart';

/// Modrinth browser: search + install mods/plugins.
class ModsTab extends StatefulWidget {
  final ServerDto server;
  const ModsTab({super.key, required this.server});

  @override
  State<ModsTab> createState() => _ModsTabState();
}

class _ModsTabState extends State<ModsTab> {
  final _search = TextEditingController();
  List<ModrinthHit> _hits = const [];
  List<InstalledFile> _installed = const [];
  bool _searching = false;
  String? _error;
  Timer? _debounce;

  ApiClient get api => context.read<AppState>().api;

  bool get _supported => widget.server.supportsMods;

  @override
  void initState() {
    super.initState();
    _loadInstalled();
    if (_supported) _doSearch('');
  }

  Future<void> _loadInstalled() async {
    try {
      _installed = await api.listMods(widget.server.id);
      if (mounted) setState(() {});
    } catch (_) {}
  }

  void _onQuery(String q) {
    _debounce?.cancel();
    _debounce = Timer(const Duration(milliseconds: 350), () => _doSearch(q));
  }

  Future<void> _doSearch(String q) async {
    setState(() {
      _searching = true;
      _error = null;
    });
    try {
      _hits = await api.searchMods(widget.server.id, q);
    } catch (e) {
      _error = '$e';
    }
    if (mounted) setState(() => _searching = false);
  }

  Future<void> _install(ModrinthHit h) async {
    try {
      final file = await api.installMod(widget.server.id, h.slug);
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(content: Text('Installed $file — restart to load it')));
      }
      await _loadInstalled();
    } catch (e) {
      if (mounted) showError(context, e);
    }
  }

  @override
  void dispose() {
    _debounce?.cancel();
    _search.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (!_supported) {
      return const Center(
          child: Padding(
              padding: EdgeInsets.all(24),
              child: Text(
                  'Modrinth content is only available for Paper, Purpur, Fabric, Forge and NeoForge servers.')));
    }
    final noun = widget.server.serverType == 'paper' ||
            widget.server.serverType == 'purpur'
        ? 'plugins'
        : 'mods';
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.all(16),
          child: TextField(
            controller: _search,
            decoration: InputDecoration(
              hintText: 'Search Modrinth $noun…',
              prefixIcon: const Icon(Icons.search),
              suffixIcon: _searching
                  ? const Padding(
                      padding: EdgeInsets.all(12),
                      child: SizedBox(
                          width: 16,
                          height: 16,
                          child: CircularProgressIndicator(strokeWidth: 2)))
                  : null,
            ),
            onChanged: _onQuery,
          ),
        ),
        if (_installed.isNotEmpty)
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Align(
              alignment: Alignment.centerLeft,
              child: Wrap(
                spacing: 6,
                runSpacing: 6,
                children: [
                  for (final f in _installed)
                    Chip(
                        label: Text(f.name,
                            style: const TextStyle(fontSize: 11)),
                        avatar: const Icon(Icons.check_circle, size: 14)),
                ],
              ),
            ),
          ),
        const SizedBox(height: 8),
        Expanded(
          child: _error != null
              ? ErrorCard(_error!, onRetry: () => _doSearch(_search.text))
              : _hits.isEmpty
                  ? Center(
                      child: Text(
                          _searching ? 'Searching…' : 'No results',
                          style: Theme.of(context).textTheme.bodyMedium))
                  : ListView.builder(
                      padding: const EdgeInsets.symmetric(horizontal: 12),
                      itemCount: _hits.length,
                      itemBuilder: (context, i) {
                        final h = _hits[i];
                        return Card(
                          child: ListTile(
                            leading: h.iconUrl != null
                                ? Image.network(h.iconUrl!,
                                    width: 36,
                                    height: 36,
                                    errorBuilder: (_, _, _) =>
                                        const Icon(Icons.extension))
                                : const Icon(Icons.extension),
                            title: Text(h.title),
                            subtitle: Text(
                              '${h.description} — ${h.downloads} ↓',
                              maxLines: 2,
                              overflow: TextOverflow.ellipsis,
                            ),
                            trailing: TextButton(
                                onPressed: () => _install(h),
                                child: const Text('Install')),
                          ),
                        );
                      },
                    ),
        ),
      ],
    );
  }
}
