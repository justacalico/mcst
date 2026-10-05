import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../api/client.dart';
import '../../api/types.dart';
import '../../state/app_state.dart';
import '../../widgets.dart';

/// Backups list + create/restore/delete/download.
class BackupsTab extends StatefulWidget {
  final ServerDto server;
  const BackupsTab({super.key, required this.server});

  @override
  State<BackupsTab> createState() => _BackupsTabState();
}

class _BackupsTabState extends State<BackupsTab> {
  List<Backup> _backups = const [];
  bool _loading = true;
  bool _creating = false;

  ApiClient get api => context.read<AppState>().api;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    Object? err;
    try {
      _backups = await api.listBackups(widget.server.id);
    } catch (e) {
      err = e;
    }
    if (!mounted) return;
    if (err != null) showError(context, err);
    setState(() => _loading = false);
  }

  Future<void> _create() async {
    final note = await promptText(context,
        title: 'New backup', label: 'Note (optional)', confirmLabel: 'Back up');
    if (note == null) return;
    setState(() => _creating = true);
    try {
      await api.createBackup(widget.server.id, note);
      await _load();
    } catch (e) {
      if (mounted) showError(context, e);
    } finally {
      if (mounted) setState(() => _creating = false);
    }
  }

  Future<void> _restore(Backup b) async {
    final ok = await showDialog<bool>(
        context: context,
        builder: (c) => AlertDialog(
              title: const Text('Restore backup?'),
              content: const Text(
                  'Current server files will be overwritten by this backup. '
                  'The server must be stopped first.'),
              actions: [
                TextButton(
                    onPressed: () => Navigator.pop(c, false),
                    child: const Text('Cancel')),
                FilledButton(
                    onPressed: () => Navigator.pop(c, true),
                    child: const Text('Restore')),
              ],
            ));
    if (ok != true) return;
    try {
      await api.restoreBackup(widget.server.id, b.id);
      if (mounted) {
        ScaffoldMessenger.of(context)
            .showSnackBar(const SnackBar(content: Text('Restored')));
      }
    } catch (e) {
      if (mounted) showError(context, e);
    }
  }

  Future<void> _delete(Backup b) async {
    try {
      await api.deleteBackup(widget.server.id, b.id);
      await _load();
    } catch (e) {
      if (mounted) showError(context, e);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.all(12),
          child: Row(children: [
            Text('${widget.server.name} backups',
                style: Theme.of(context).textTheme.labelLarge),
            const Spacer(),
            FilledButton.tonalIcon(
                onPressed: _creating ? null : _create,
                icon: const Icon(Icons.save_alt, size: 16),
                label: Text(_creating ? 'Backing up…' : 'New backup')),
          ]),
        ),
        Expanded(
          child: _loading
              ? const Center(child: CircularProgressIndicator())
              : _backups.isEmpty
                  ? const Center(child: Text('No backups yet'))
                  : ListView.builder(
                      padding: const EdgeInsets.symmetric(horizontal: 12),
                      itemCount: _backups.length,
                      itemBuilder: (context, i) {
                        final b = _backups[i];
                        return Card(
                          child: ListTile(
                            leading: const Icon(Icons.archive_outlined),
                            title: Text(b.note.isEmpty
                                ? 'Backup ${b.createdAt}'
                                : b.note),
                            subtitle: Text(
                                '${b.createdAt} · ${humanBytes(b.sizeBytes)}'),
                            trailing: Row(mainAxisSize: MainAxisSize.min, children: [
                              IconButton(
                                  tooltip: 'Download',
                                  icon: const Icon(Icons.download, size: 18),
                                  onPressed: () => launchUrl(Uri.parse(api
                                      .backupDownloadUrl(
                                          widget.server.id, b.id)))),
                              IconButton(
                                  tooltip: 'Restore',
                                  icon: const Icon(Icons.restore, size: 18),
                                  onPressed: () => _restore(b)),
                              IconButton(
                                  tooltip: 'Delete',
                                  icon: const Icon(Icons.delete_outline,
                                      size: 18),
                                  onPressed: () => _delete(b)),
                            ]),
                          ),
                        );
                      },
                    ),
        ),
      ],
    );
  }
}
