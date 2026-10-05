import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../api/client.dart';
import '../../api/types.dart';
import '../../state/app_state.dart';
import '../../widgets.dart';

/// Per-server settings: resources, JVM, flags, server.properties, danger
/// zone, tailscale exposure.
class ServerSettingsTab extends StatefulWidget {
  final ServerDto server;
  const ServerSettingsTab({super.key, required this.server});

  @override
  State<ServerSettingsTab> createState() => _ServerSettingsTabState();
}

class _ServerSettingsTabState extends State<ServerSettingsTab> {
  late final TextEditingController _name;
  late final TextEditingController _port;
  late final TextEditingController _mem;
  late final TextEditingController _minMem;
  late final TextEditingController _jvm;
  late final TextEditingController _timeout;
  late final TextEditingController _emptyStop;
  bool _autoStart = false;
  bool _restartOnCrash = false;
  bool _busy = false;

  ApiClient get api => context.read<AppState>().api;

  bool get _stopped => !widget.server.status.isBusy;

  @override
  void initState() {
    super.initState();
    final s = widget.server;
    _name = TextEditingController(text: s.name);
    _port = TextEditingController(text: '${s.port}');
    _mem = TextEditingController(text: '${s.memoryMb}');
    _minMem = TextEditingController(
        text: s.minMemoryMb > 0 ? '${s.minMemoryMb}' : '');
    _jvm = TextEditingController(text: s.jvmArgs);
    _timeout = TextEditingController(text: '${s.shutdownTimeoutSec}');
    _emptyStop = TextEditingController(
        text: s.emptyStopMinutes > 0 ? '${s.emptyStopMinutes}' : '');
    _autoStart = s.autoStart;
    _restartOnCrash = s.restartOnCrash;
  }

  @override
  void dispose() {
    for (final c in [_name, _port, _mem, _minMem, _jvm, _timeout, _emptyStop]) {
      c.dispose();
    }
    super.dispose();
  }

  Future<void> _save() async {
    setState(() => _busy = true);
    try {
      final updated = await api.updateServer(widget.server.id, {
        'name': _name.text.trim(),
        'port': int.tryParse(_port.text),
        'memory_mb': int.tryParse(_mem.text),
        'min_memory_mb': int.tryParse(_minMem.text) ?? 0,
        'jvm_args': _jvm.text.trim(),
        'auto_start': _autoStart,
        'restart_on_crash': _restartOnCrash,
        'shutdown_timeout_sec': int.tryParse(_timeout.text),
        'empty_stop_minutes': int.tryParse(_emptyStop.text) ?? 0,
      });
      if (mounted) {
        context.read<AppState>().patchServer(updated);
        ScaffoldMessenger.of(context)
            .showSnackBar(const SnackBar(content: Text('Saved')));
      }
    } catch (e) {
      if (mounted) showError(context, e);
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _updateJar() async {
    try {
      await api.updateJar(widget.server.id);
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
            const SnackBar(content: Text('Updated to the latest build')));
      }
    } catch (e) {
      if (mounted) showError(context, e);
    }
  }

  Future<void> _tailscale(bool enable) async {
    try {
      await api.tailscaleServer(widget.server.id, enable);
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(
            content: Text(enable
                ? 'Server is now reachable on your tailnet'
                : 'Tailscale forwarding removed')));
      }
    } catch (e) {
      if (mounted) showError(context, e);
    }
  }

  Future<void> _delete() async {
    final typed = await promptText(context,
        title: 'Delete ${widget.server.name}?',
        label: 'Server name',
        confirmLabel: 'Delete',
        danger: true,
        body:
            'Worlds, configs, and backups are permanently deleted. Type the server name to confirm.');
    if (typed == widget.server.name && mounted) {
      final nav = Navigator.of(context);
      final app = context.read<AppState>();
      try {
        await api.deleteServer(widget.server.id);
        await app.refreshServers();
        nav.pop();
      } catch (e) {
        if (mounted) showError(context, e);
      }
    }
  }

  Future<void> _editProperties() async {
    try {
      final raw = await api.getPropertiesRaw(widget.server.id);
      if (!mounted) return;
      final text = await showDialog<String>(
          context: context,
          builder: (_) => _PropertiesDialog(initial: raw));
      if (text != null) {
        await api.putPropertiesRaw(widget.server.id, text);
        if (mounted) {
          ScaffoldMessenger.of(context)
              .showSnackBar(const SnackBar(content: Text('Properties saved')));
        }
      }
    } catch (e) {
      if (mounted) showError(context, e);
    }
  }

  @override
  Widget build(BuildContext context) {
    final s = widget.server;
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        if (!_stopped)
          const Card(
            child: Padding(
              padding: EdgeInsets.all(12),
              child: Row(children: [
                Icon(Icons.info_outline, size: 16),
                SizedBox(width: 8),
                Expanded(
                    child: Text(
                        'Stop the server to edit its settings.')),
              ]),
            ),
          ),
        Card(
          child: Padding(
            padding: const EdgeInsets.all(16),
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text('General', style: Theme.of(context).textTheme.titleMedium),
              const SizedBox(height: 12),
              TextField(
                  controller: _name,
                  enabled: _stopped,
                  decoration:
                      const InputDecoration(labelText: 'Server name')),
              const SizedBox(height: 12),
              Row(children: [
                Expanded(
                    child: TextField(
                        controller: _port,
                        enabled: _stopped,
                        keyboardType: TextInputType.number,
                        decoration:
                            const InputDecoration(labelText: 'Port'))),
                const SizedBox(width: 12),
                Expanded(
                    child: TextField(
                        controller: _mem,
                        enabled: _stopped,
                        keyboardType: TextInputType.number,
                        decoration: const InputDecoration(
                            labelText: 'Max memory (MiB)'))),
                const SizedBox(width: 12),
                Expanded(
                    child: TextField(
                        controller: _minMem,
                        enabled: _stopped,
                        keyboardType: TextInputType.number,
                        decoration: const InputDecoration(
                            labelText: 'Min memory (MiB, optional)'))),
              ]),
              const SizedBox(height: 12),
              TextField(
                  controller: _jvm,
                  enabled: _stopped,
                  decoration: const InputDecoration(
                      labelText: 'JVM flags',
                      hintText: '-XX:+UseG1GC')),
              const SizedBox(height: 12),
              Row(children: [
                Expanded(
                    child: TextField(
                        controller: _timeout,
                        enabled: _stopped,
                        keyboardType: TextInputType.number,
                        decoration: const InputDecoration(
                            labelText: 'Stop timeout (s)'))),
                const SizedBox(width: 12),
                Expanded(
                    child: TextField(
                        controller: _emptyStop,
                        enabled: _stopped,
                        keyboardType: TextInputType.number,
                        decoration: const InputDecoration(
                            labelText: 'Auto-stop when empty (min)'))),
              ]),
              SwitchListTile(
                  title: const Text('Start with mcst'),
                  subtitle: const Text('Start this server when the panel boots'),
                  value: _autoStart,
                  onChanged:
                      _stopped ? (v) => setState(() => _autoStart = v) : null,
                  contentPadding: EdgeInsets.zero),
              SwitchListTile(
                  title: const Text('Restart on crash'),
                  subtitle: const Text('Auto-restart after an unexpected exit'),
                  value: _restartOnCrash,
                  onChanged: _stopped
                      ? (v) => setState(() => _restartOnCrash = v)
                      : null,
                  contentPadding: EdgeInsets.zero),
              const SizedBox(height: 8),
              Align(
                alignment: Alignment.centerRight,
                child: FilledButton.icon(
                    onPressed: _stopped && !_busy ? _save : null,
                    icon: const Icon(Icons.save, size: 16),
                    label: const Text('Save')),
              ),
            ]),
          ),
        ),
        const SizedBox(height: 12),
        Card(
          child: Column(children: [
            ListTile(
              leading: const Icon(Icons.tune),
              title: const Text('server.properties'),
              subtitle: const Text('Edit the raw properties file'),
              trailing: const Icon(Icons.chevron_right),
              onTap: _editProperties,
            ),
            const Divider(height: 1),
            ListTile(
              leading: const Icon(Icons.system_update_alt),
              title: const Text('Update server jar'),
              subtitle: Text(
                  'Re-download the latest ${s.serverType} build for ${s.mcVersion}'),
              trailing: const Icon(Icons.chevron_right),
              onTap: _stopped ? _updateJar : null,
            ),
            const Divider(height: 1),
            ListTile(
              leading: const Icon(Icons.vpn_lock),
              title: const Text('Share on Tailscale'),
              subtitle: const Text(
                  'Forward this server\'s port over your tailnet — friends join without port forwarding'),
              trailing: PopupMenuButton<bool>(
                onSelected: _tailscale,
                itemBuilder: (_) => const [
                  PopupMenuItem(value: true, child: Text('Enable')),
                  PopupMenuItem(value: false, child: Text('Disable')),
                ],
                child: const Icon(Icons.chevron_right),
              ),
            ),
          ]),
        ),
        const SizedBox(height: 12),
        Card(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 12, 16, 0),
                child: Text('Danger zone',
                    style: Theme.of(context)
                        .textTheme
                        .titleSmall
                        ?.copyWith(
                            color: Theme.of(context).colorScheme.error)),
              ),
              ListTile(
                leading: Icon(Icons.delete_forever,
                    color: Theme.of(context).colorScheme.error),
                title: const Text('Delete server'),
                subtitle: Text(s.dir,
                    style: const TextStyle(
                        fontFamily: 'JetBrainsMono', fontSize: 11)),
                onTap: _stopped ? _delete : null,
              ),
            ],
          ),
        ),
      ],
    );
  }
}

/// Raw server.properties editor — owns its controller for the dialog's whole
/// lifecycle.
class _PropertiesDialog extends StatefulWidget {
  final String initial;
  const _PropertiesDialog({required this.initial});

  @override
  State<_PropertiesDialog> createState() => _PropertiesDialogState();
}

class _PropertiesDialogState extends State<_PropertiesDialog> {
  late final TextEditingController _c;

  @override
  void initState() {
    super.initState();
    _c = TextEditingController(text: widget.initial);
  }

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
        title: const Text('server.properties'),
        content: SizedBox(
          width: 560,
          height: 420,
          child: TextField(
            controller: _c,
            maxLines: null,
            expands: true,
            textAlignVertical: TextAlignVertical.top,
            style: const TextStyle(fontFamily: 'JetBrainsMono', fontSize: 12),
            decoration: const InputDecoration(border: InputBorder.none),
          ),
        ),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(context),
              child: const Text('Cancel')),
          FilledButton(
              onPressed: () => Navigator.pop(context, _c.text),
              child: const Text('Save')),
        ],
      );
}
