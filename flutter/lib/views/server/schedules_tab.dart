import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../api/client.dart';
import '../../api/types.dart';
import '../../state/app_state.dart';
import '../../widgets.dart';

/// Scheduled tasks: interval or daily actions.
class SchedulesTab extends StatefulWidget {
  final ServerDto server;
  const SchedulesTab({super.key, required this.server});

  @override
  State<SchedulesTab> createState() => _SchedulesTabState();
}

class _SchedulesTabState extends State<SchedulesTab> {
  List<Schedule> _items = const [];
  bool _loading = true;

  ApiClient get api => context.read<AppState>().api;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      _items = await api.listSchedules(widget.server.id);
    } catch (_) {}
    if (mounted) setState(() => _loading = false);
  }

  Future<void> _edit([Schedule? existing]) async {
    final result = await showDialog<Schedule>(
        context: context,
        builder: (_) =>
            _ScheduleDialog(existing: existing, serverId: widget.server.id));
    if (result == null) return;
    await _load();
  }

  Future<void> _toggle(Schedule s, bool enabled) async {
    try {
      await api.updateSchedule(widget.server.id, s.id, {
        'action': s.action,
        'payload': s.payload,
        'every_minutes': s.everyMinutes,
        'daily_time': s.dailyTime,
        'enabled': enabled,
        'name': s.name,
      });
      await _load();
    } catch (e) {
      if (mounted) showError(context, e);
    }
  }

  Future<void> _delete(Schedule s) async {
    try {
      await api.deleteSchedule(widget.server.id, s.id);
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
            Text('${_items.length} schedules',
                style: Theme.of(context).textTheme.labelLarge),
            const Spacer(),
            FilledButton.tonalIcon(
                onPressed: () => _edit(),
                icon: const Icon(Icons.add, size: 16),
                label: const Text('New schedule')),
          ]),
        ),
        Expanded(
          child: _loading
              ? const Center(child: CircularProgressIndicator())
              : _items.isEmpty
                  ? const Center(
                      child: Text(
                          'Schedule restarts, backups, or commands on a timer.'))
                  : ListView.builder(
                      padding: const EdgeInsets.symmetric(horizontal: 12),
                      itemCount: _items.length,
                      itemBuilder: (context, i) {
                        final s = _items[i];
                        return Card(
                          child: ListTile(
                            leading: Icon(_iconFor(s.action)),
                            title: Text(s.name),
                            subtitle: Text(
                                '${s.action}${s.payload.isEmpty ? '' : ' · ${s.payload}'} — ${s.whenLabel}'),
                            trailing: Row(mainAxisSize: MainAxisSize.min, children: [
                              Switch(
                                  value: s.enabled,
                                  onChanged: (v) => _toggle(s, v)),
                              IconButton(
                                  icon: const Icon(Icons.delete_outline,
                                      size: 18),
                                  onPressed: () => _delete(s)),
                            ]),
                            onTap: () => _edit(s),
                          ),
                        );
                      },
                    ),
        ),
      ],
    );
  }

  IconData _iconFor(String action) => switch (action) {
        'command' => Icons.terminal,
        'restart' => Icons.restart_alt,
        'backup' => Icons.save_alt,
        'start' => Icons.play_arrow,
        'stop' => Icons.stop,
        _ => Icons.schedule,
      };
}

class _ScheduleDialog extends StatefulWidget {
  final Schedule? existing;
  final String serverId;
  const _ScheduleDialog({this.existing, required this.serverId});

  @override
  State<_ScheduleDialog> createState() => _ScheduleDialogState();
}

class _ScheduleDialogState extends State<_ScheduleDialog> {
  late final TextEditingController _name;
  late String _action;
  late final TextEditingController _payload;
  bool _daily = false;
  late final TextEditingController _every;
  late final TextEditingController _time;

  @override
  void initState() {
    super.initState();
    final s = widget.existing;
    _name = TextEditingController(text: s?.name ?? '');
    _action = s?.action ?? 'restart';
    _payload = TextEditingController(text: s?.payload ?? '');
    _daily = s != null && s.dailyTime.isNotEmpty;
    _every = TextEditingController(
        text: (s?.everyMinutes ?? 0) > 0 ? '${s!.everyMinutes}' : '60');
    _time = TextEditingController(text: s?.dailyTime ?? '04:00');
  }

  @override
  void dispose() {
    _name.dispose();
    _payload.dispose();
    _every.dispose();
    _time.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    final api = context.read<AppState>().api;
    final body = {
      'name': _name.text.trim().isEmpty ? _action : _name.text.trim(),
      'action': _action,
      'payload': _payload.text.trim(),
      'every_minutes': _daily ? 0 : (int.tryParse(_every.text) ?? 0),
      'daily_time': _daily ? _time.text.trim() : '',
      'enabled': widget.existing?.enabled ?? true,
    };
    try {
      if (widget.existing == null) {
        await api.createSchedule(widget.serverId, body);
      } else {
        await api.updateSchedule(widget.serverId, widget.existing!.id, body);
      }
      if (mounted) Navigator.pop(context, widget.existing);
    } catch (e) {
      if (mounted) showError(context, e);
    }
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Text(widget.existing == null ? 'New schedule' : 'Edit schedule'),
      content: SizedBox(
        width: 420,
        child: Column(mainAxisSize: MainAxisSize.min, children: [
          TextField(
              controller: _name,
              decoration: const InputDecoration(labelText: 'Name')),
          const SizedBox(height: 12),
          DropdownButtonFormField<String>(
            initialValue: _action,
            decoration: const InputDecoration(labelText: 'Action'),
            items: const [
              DropdownMenuItem(value: 'restart', child: Text('Restart')),
              DropdownMenuItem(value: 'start', child: Text('Start')),
              DropdownMenuItem(value: 'stop', child: Text('Stop')),
              DropdownMenuItem(value: 'backup', child: Text('Backup')),
              DropdownMenuItem(value: 'command', child: Text('Command')),
            ],
            onChanged: (v) => setState(() => _action = v!),
          ),
          if (_action == 'command' || _action == 'backup') ...[
            const SizedBox(height: 12),
            TextField(
                controller: _payload,
                decoration: InputDecoration(
                    labelText:
                        _action == 'command' ? 'Command' : 'Note')),
          ],
          const SizedBox(height: 12),
          SegmentedButton<bool>(
            segments: const [
              ButtonSegment(value: false, label: Text('Every N minutes')),
              ButtonSegment(value: true, label: Text('Daily at')),
            ],
            selected: {_daily},
            onSelectionChanged: (v) => setState(() => _daily = v.first),
          ),
          const SizedBox(height: 12),
          if (_daily)
            TextField(
                controller: _time,
                decoration:
                    const InputDecoration(labelText: 'Time (HH:MM)'))
          else
            TextField(
                controller: _every,
                keyboardType: TextInputType.number,
                decoration:
                    const InputDecoration(labelText: 'Every (minutes)')),
        ]),
      ),
      actions: [
        TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Cancel')),
        FilledButton(onPressed: _save, child: const Text('Save')),
      ],
    );
  }
}
