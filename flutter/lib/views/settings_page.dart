import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../api/client.dart';
import '../api/types.dart';
import '../state/app_state.dart';
import '../widgets.dart';

/// Panel settings: account, Java runtimes, Tailscale, audit log.
class SettingsPage extends StatefulWidget {
  const SettingsPage({super.key});

  @override
  State<SettingsPage> createState() => _SettingsPageState();
}

class _SettingsPageState extends State<SettingsPage> {
  TailscaleStatus _ts = TailscaleStatus.empty;
  List<JavaInstall> _java = const [];
  List<AuditEntry> _audit = const [];
  bool _loading = true;

  ApiClient get api => context.read<AppState>().api;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final ts = await api.tailscaleStatus();
      final java = await api.listJava();
      final audit = await api.auditLog();
      if (mounted) {
        setState(() {
          _ts = ts;
          _java = java;
          _audit = audit;
          _loading = false;
        });
      }
    } catch (e) {
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<void> _installJava() async {
    final c = TextEditingController(text: '21');
    final v = await showDialog<int>(
        context: context,
        builder: (ctx) => AlertDialog(
              title: const Text('Install Java (Temurin)'),
              content: TextField(
                  controller: c,
                  keyboardType: TextInputType.number,
                  decoration: const InputDecoration(
                      labelText: 'Major version', hintText: '8, 17, 21')),
              actions: [
                TextButton(
                    onPressed: () => Navigator.pop(ctx),
                    child: const Text('Cancel')),
                FilledButton(
                    onPressed: () =>
                        Navigator.pop(ctx, int.tryParse(c.text.trim())),
                    child: const Text('Download')),
              ],
            ));
    if (v == null) return;
    try {
      await api.installJava(v);
      await _load();
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(content: Text('Java $v installed')));
      }
    } catch (e) {
      if (mounted) showError(context, e);
    }
  }

  Future<void> _togglePanelServe(bool enable) async {
    try {
      await api.tailscalePanelServe(enable);
      await _load();
    } catch (e) {
      if (mounted) showError(context, e);
    }
  }

  Future<void> _changePassword() async {
    final cur = TextEditingController();
    final next = TextEditingController();
    final ok = await showDialog<bool>(
        context: context,
        builder: (ctx) => AlertDialog(
              title: const Text('Change password'),
              content: Column(mainAxisSize: MainAxisSize.min, children: [
                TextField(
                    controller: cur,
                    obscureText: true,
                    decoration:
                        const InputDecoration(labelText: 'Current password')),
                const SizedBox(height: 12),
                TextField(
                    controller: next,
                    obscureText: true,
                    decoration:
                        const InputDecoration(labelText: 'New password')),
              ]),
              actions: [
                TextButton(
                    onPressed: () => Navigator.pop(ctx, false),
                    child: const Text('Cancel')),
                FilledButton(
                    onPressed: () => Navigator.pop(ctx, true),
                    child: const Text('Change')),
              ],
            ));
    if (ok != true) return;
    try {
      await api.changePassword(cur.text, next.text);
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(const SnackBar(
            content: Text('Password changed — sign in again')));
        await context.read<AppState>().logout();
      }
    } catch (e) {
      if (mounted) showError(context, e);
    }
  }

  @override
  Widget build(BuildContext context) {
    final app = context.watch<AppState>();
    return Scaffold(
      appBar: AppBar(title: const Text('Settings')),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : PageBody(
              maxWidth: 860,
              child: ListView(
                padding: const EdgeInsets.all(20),
                children: [
                  _section(context, 'Account', [
                    ListTile(
                      leading: const Icon(Icons.person_outline),
                      title: Text(app.username),
                      subtitle: const Text('Owner account'),
                      trailing: TextButton(
                          onPressed: _changePassword,
                          child: const Text('Change password')),
                    ),
                  ]),
                  const SizedBox(height: 16),
                  _section(context, 'Java runtimes', [
                    for (final j in _java)
                      ListTile(
                        leading: const Icon(Icons.coffee),
                        title: Text(
                            'Java ${j.major}${j.managed ? ' (managed by mcst)' : ''}'),
                        subtitle: Text(j.path,
                            style: const TextStyle(
                                fontFamily: 'JetBrainsMono', fontSize: 11),
                            overflow: TextOverflow.ellipsis),
                        trailing: j.version.isNotEmpty
                            ? Text(j.version,
                                style: Theme.of(context).textTheme.labelSmall)
                            : null,
                      ),
                    Padding(
                      padding: const EdgeInsets.all(12),
                      child: Align(
                        alignment: Alignment.centerRight,
                        child: FilledButton.tonalIcon(
                            onPressed: _installJava,
                            icon: const Icon(Icons.download, size: 16),
                            label: const Text('Install a Java version')),
                      ),
                    ),
                  ]),
                  const SizedBox(height: 16),
                  _section(context, 'Tailscale', [
                    if (!_ts.installed)
                      const ListTile(
                        leading: Icon(Icons.vpn_lock),
                        title: Text('Tailscale not installed'),
                        subtitle: Text(
                            'Install tailscaled on this host to share the panel or game ports over your tailnet.'),
                      )
                    else ...[
                      ListTile(
                        leading: const Icon(Icons.vpn_lock),
                        title: Text(_ts.running
                            ? 'Connected to ${_ts.tailnet.isEmpty ? "tailnet" : _ts.tailnet}'
                            : 'Installed, not connected'),
                        subtitle:
                            Text('${_ts.hostname} · ${_ts.ip}'),
                      ),
                      SwitchListTile(
                        title: const Text('Serve panel over tailnet'),
                        subtitle: Text(_ts.panelServeEnabled
                            ? 'https://${_ts.hostname}:${_ts.panelServePort}'
                            : 'Expose this panel via Tailscale serve (HTTPS)'),
                        value: _ts.panelServeEnabled,
                        onChanged:
                            _ts.running ? _togglePanelServe : null,
                      ),
                    ],
                  ]),
                  const SizedBox(height: 16),
                  _section(context, 'Activity', [
                    for (final e in _audit.take(20))
                      ListTile(
                        dense: true,
                        leading: const Icon(Icons.bolt, size: 16),
                        title: Text('${e.action} ${e.detail}',
                            style: const TextStyle(fontSize: 13)),
                        subtitle: Text(
                            '${e.ts}${e.actor.isEmpty ? '' : ' · ${e.actor}'}',
                            style: Theme.of(context).textTheme.labelSmall),
                      ),
                    if (_audit.isEmpty)
                      const ListTile(
                          title: Text('No activity yet')),
                  ]),
                ],
              ),
            ),
    );
  }

  Widget _section(BuildContext context, String title, List<Widget> children) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(title, style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 8),
        Card(child: Column(children: children)),
      ],
    );
  }
}
