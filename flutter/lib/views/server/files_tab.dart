import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../api/client.dart';
import '../../api/types.dart';
import '../../state/app_state.dart';
import '../../widgets.dart';

/// File browser + text editor.
class FilesTab extends StatefulWidget {
  final ServerDto server;
  const FilesTab({super.key, required this.server});

  @override
  State<FilesTab> createState() => _FilesTabState();
}

class _FilesTabState extends State<FilesTab> {
  String _path = '';
  List<FileEntry> _entries = const [];
  bool _loading = true;
  String? _error;

  // Editor state
  String? _editing;
  final _editor = TextEditingController();
  bool _dirty = false;

  ApiClient get api => context.read<AppState>().api;
  String get _sid => widget.server.id;

  @override
  void initState() {
    super.initState();
    _load('');
  }

  Future<void> _load(String path) async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final e = await api.listFiles(_sid, path);
      setState(() {
        _path = path;
        _entries = e;
        _loading = false;
      });
    } catch (e) {
      setState(() {
        _error = '$e';
        _loading = false;
      });
    }
  }

  Future<void> _open(FileEntry f) async {
    if (f.isDir) return _load(f.path);
    try {
      final content = await api.readFile(_sid, f.path);
      setState(() {
        _editing = f.path;
        _editor.text = content;
        _dirty = false;
      });
    } catch (e) {
      if (mounted) showError(context, e);
    }
  }

  Future<void> _save() async {
    if (_editing == null) return;
    try {
      await api.writeFile(_sid, _editing!, _editor.text);
      setState(() => _dirty = false);
      if (mounted) {
        ScaffoldMessenger.of(context)
            .showSnackBar(const SnackBar(content: Text('Saved')));
      }
    } catch (e) {
      if (mounted) showError(context, e);
    }
  }

  Future<void> _upload() async {
    final res = await FilePicker.platform.pickFiles(withData: true);
    if (res == null) return;
    for (final f in res.files) {
      if (f.bytes == null) continue;
      await api.uploadFile(_sid, _path, f.name, f.bytes!);
    }
    await _load(_path);
  }

  Future<void> _delete(FileEntry f) async {
    final ok = await showDialog<bool>(
        context: context,
        builder: (c) => AlertDialog(
              title: Text('Delete ${f.name}?'),
              actions: [
                TextButton(
                    onPressed: () => Navigator.pop(c, false),
                    child: const Text('Cancel')),
                FilledButton(
                    onPressed: () => Navigator.pop(c, true),
                    child: const Text('Delete')),
              ],
            ));
    if (ok == true) {
      await api.deleteFile(_sid, f.path);
      await _load(_path);
    }
  }

  Future<void> _newFolder() async {
    final c = TextEditingController();
    final name = await showDialog<String>(
        context: context,
        builder: (ctx) => AlertDialog(
              title: const Text('New folder'),
              content: TextField(
                  controller: c, autofocus: true, decoration: const InputDecoration(labelText: 'Name')),
              actions: [
                TextButton(
                    onPressed: () => Navigator.pop(ctx),
                    child: const Text('Cancel')),
                FilledButton(
                    onPressed: () => Navigator.pop(ctx, c.text.trim()),
                    child: const Text('Create')),
              ],
            ));
    if (name != null && name.isNotEmpty) {
      await api.mkdir(
          _sid, _path.isEmpty ? name : '$_path/$name');
      await _load(_path);
    }
  }

  @override
  void dispose() {
    _editor.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (_editing != null) return _editorView();
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 8),
          child: Row(children: [
            IconButton(
                icon: const Icon(Icons.arrow_upward, size: 18),
                tooltip: 'Up',
                onPressed: _path.isEmpty
                    ? null
                    : () => _load(_path.contains('/')
                        ? _path.substring(0, _path.lastIndexOf('/'))
                        : '')),
            Expanded(
              child: Text('/$_path',
                  style: const TextStyle(
                      fontFamily: 'JetBrainsMono', fontSize: 12),
                  overflow: TextOverflow.ellipsis),
            ),
            IconButton(
                tooltip: 'New folder',
                icon: const Icon(Icons.create_new_folder_outlined, size: 18),
                onPressed: _newFolder),
            IconButton(
                tooltip: 'Upload',
                icon: const Icon(Icons.upload_file, size: 18),
                onPressed: _upload),
            IconButton(
                tooltip: 'Refresh',
                icon: const Icon(Icons.refresh, size: 18),
                onPressed: () => _load(_path)),
          ]),
        ),
        Expanded(
          child: _loading
              ? const Center(child: CircularProgressIndicator())
              : _error != null
                  ? Center(child: Text(_error!))
                  : _entries.isEmpty
                      ? const Center(child: Text('Empty directory'))
                      : ListView.builder(
                          padding: const EdgeInsets.symmetric(horizontal: 12),
                          itemCount: _entries.length,
                          itemBuilder: (context, i) {
                            final f = _entries[i];
                            return ListTile(
                              dense: true,
                              leading: Icon(f.isDir
                                  ? Icons.folder_outlined
                                  : _iconFor(f.name)),
                              title: Text(f.name,
                                  style: const TextStyle(
                                      fontFamily: 'JetBrainsMono',
                                      fontSize: 13)),
                              subtitle: f.isDir
                                  ? null
                                  : Text(humanBytes(f.size),
                                      style: Theme.of(context)
                                          .textTheme
                                          .labelSmall),
                              trailing: f.isDir
                                  ? null
                                  : PopupMenuButton<String>(
                                      onSelected: (v) => switch (v) {
                                            'download' => launchUrl(Uri.parse(
                                                api.fileDownloadUrl(
                                                    _sid, f.path))),
                                            'delete' => _delete(f),
                                            _ => null
                                          },
                                      itemBuilder: (_) => const [
                                            PopupMenuItem(
                                                value: 'download',
                                                child: Text('Download')),
                                            PopupMenuItem(
                                                value: 'delete',
                                                child: Text('Delete')),
                                          ]),
                              onTap: () => _open(f),
                            );
                          },
                        ),
        ),
      ],
    );
  }

  IconData _iconFor(String name) {
    if (name.endsWith('.jar')) return Icons.coffee;
    if (name.endsWith('.properties') || name.endsWith('.yml') || name.endsWith('.yaml')) {
      return Icons.tune;
    }
    if (name.endsWith('.json')) return Icons.data_object;
    if (name.endsWith('.png')) return Icons.image_outlined;
    return Icons.insert_drive_file_outlined;
  }

  Widget _editorView() {
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 8),
          child: Row(children: [
            IconButton(
                icon: const Icon(Icons.close, size: 18),
                onPressed: () => setState(() {
                      _editing = null;
                      _dirty = false;
                    })),
            Expanded(
              child: Text(_editing!,
                  style: const TextStyle(
                      fontFamily: 'JetBrainsMono', fontSize: 12),
                  overflow: TextOverflow.ellipsis),
            ),
            if (_dirty)
              FilledButton.icon(
                  onPressed: _save,
                  icon: const Icon(Icons.save, size: 16),
                  label: const Text('Save')),
          ]),
        ),
        Expanded(
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: TextField(
              controller: _editor,
              maxLines: null,
              expands: true,
              textAlignVertical: TextAlignVertical.top,
              style: const TextStyle(fontFamily: 'JetBrainsMono', fontSize: 13),
              decoration: const InputDecoration(border: InputBorder.none),
              onChanged: (_) => setState(() => _dirty = true),
            ),
          ),
        ),
        const SizedBox(height: 8),
      ],
    );
  }
}
