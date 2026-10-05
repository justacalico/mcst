import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';

import '../../api/client.dart';
import '../../api/types.dart';
import '../../state/app_state.dart';
import '../../widgets.dart';

/// Live console: scrollable log + command input + stats strip.
class ConsoleTab extends StatefulWidget {
  final ServerDto server;
  const ConsoleTab({super.key, required this.server});

  @override
  State<ConsoleTab> createState() => _ConsoleTabState();
}

class _ConsoleTabState extends State<ConsoleTab> {
  final List<String> _lines = [];
  final _scroll = ScrollController();
  final _cmd = TextEditingController();
  late final FocusNode _cmdFocus;
  final _history = <String>[];
  int _historyIdx = -1;
  StreamSubscription<ConsoleEvent>? _sub;
  bool _autoscroll = true;

  @override
  void initState() {
    super.initState();
    _connect();
    _scroll.addListener(() {
      _autoscroll = _scroll.position.pixels >=
          _scroll.position.maxScrollExtent - 60;
    });
    _cmdFocus = FocusNode(onKeyEvent: (node, e) {
      // Up/down recalls previous commands.
      if (e is! KeyDownEvent || _history.isEmpty) {
        return KeyEventResult.ignored;
      }
      if (e.logicalKey == LogicalKeyboardKey.arrowUp && _historyIdx > 0) {
        _historyIdx--;
        _cmd.text = _history[_historyIdx];
        _cmd.selection =
            TextSelection.collapsed(offset: _cmd.text.length);
        return KeyEventResult.handled;
      }
      if (e.logicalKey == LogicalKeyboardKey.arrowDown) {
        if (_historyIdx < _history.length - 1) {
          _historyIdx++;
          _cmd.text = _history[_historyIdx];
        } else {
          _historyIdx = _history.length;
          _cmd.clear();
        }
        _cmd.selection = TextSelection.collapsed(offset: _cmd.text.length);
        return KeyEventResult.handled;
      }
      return KeyEventResult.ignored;
    });
  }

  void _connect() {
    _sub = context
        .read<AppState>()
        .api
        .consoleStream(widget.server.id)
        .listen((ev) {
      setState(() {
        switch (ev) {
          case ConsoleHistory(:final lines):
            _lines
              ..clear()
              ..addAll(lines);
          case ConsoleLine(:final line):
            _lines.add(line);
            if (_lines.length > 3000) _lines.removeRange(0, 500);
        }
      });
      if (_autoscroll) _toBottom();
    });
  }

  void _toBottom() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (_scroll.hasClients) {
        _scroll.jumpTo(_scroll.position.maxScrollExtent);
      }
    });
  }

  Future<void> _send() async {
    final text = _cmd.text.trim();
    if (text.isEmpty) return;
    _history.add(text);
    _historyIdx = _history.length;
    _cmd.clear();
    try {
      await context
          .read<AppState>()
          .api
          .sendCommand(widget.server.id, text);
    } catch (e) {
      if (mounted) showError(context, e);
    }
  }

  @override
  void dispose() {
    _sub?.cancel();
    _scroll.dispose();
    _cmd.dispose();
    _cmdFocus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final s = widget.server;
    final scheme = Theme.of(context).colorScheme;
    return Column(
      children: [
        // Stats strip
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 0),
          child: Row(children: [
            _chip(context, Icons.memory,
                '${s.cpuPercent.toStringAsFixed(0)}% CPU'),
            const SizedBox(width: 8),
            _chip(context, Icons.storage, humanBytes(s.memBytes)),
            const SizedBox(width: 8),
            _chip(context, Icons.schedule, humanDuration(s.uptimeSec)),
            const SizedBox(width: 8),
            _chip(context, Icons.people,
                '${s.playersOnline}/${s.playersMax}'),
            const Spacer(),
            IconButton(
                tooltip: 'Copy logs',
                icon: const Icon(Icons.copy, size: 18),
                onPressed: () {}),
          ]),
        ),
        const SizedBox(height: 8),
        Expanded(
          child: Container(
            margin: const EdgeInsets.symmetric(horizontal: 16),
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              color: scheme.surfaceContainerLowest,
              borderRadius: BorderRadius.circular(10),
              border: Border.all(color: scheme.outlineVariant.withAlpha(60)),
            ),
            child: _lines.isEmpty
                ? Center(
                    child: Text('No output yet — start the server.',
                        style: TextStyle(color: scheme.onSurfaceVariant)),
                  )
                : ListView.builder(
                    controller: _scroll,
                    itemCount: _lines.length,
                    itemBuilder: (context, i) => _logLine(context, _lines[i]),
                  ),
          ),
        ),
        Padding(
          padding: const EdgeInsets.all(16),
          child: Row(children: [
            Expanded(
              child: TextField(
                controller: _cmd,
                focusNode: _cmdFocus,
                style: const TextStyle(fontFamily: 'JetBrainsMono', fontSize: 13),
                decoration: const InputDecoration(
                  hintText: 'Type a command… (e.g. say hello, list, stop)',
                  prefixIcon: Icon(Icons.chevron_right),
                ),
                onSubmitted: (_) => _send(),
                textInputAction: TextInputAction.send,
              ),
            ),
            const SizedBox(width: 8),
            FilledButton.tonalIcon(
              onPressed: _send,
              icon: const Icon(Icons.send, size: 16),
              label: const Text('Send'),
            ),
          ]),
        ),
      ],
    );
  }

  Widget _logLine(BuildContext context, String line) {
    final scheme = Theme.of(context).colorScheme;
    Color? color;
    if (line.contains('/ERROR]') || line.contains('/FATAL]')) {
      color = scheme.error;
    } else if (line.contains('/WARN]')) {
      color = Colors.orange;
    } else if (line.startsWith('[mcst]')) {
      color = scheme.primary;
    }
    return Text(line,
        style: TextStyle(
          fontFamily: 'JetBrainsMono',
          fontSize: 12,
          height: 1.4,
          color: color,
        ));
  }

  Widget _chip(BuildContext context, IconData icon, String text) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Row(mainAxisSize: MainAxisSize.min, children: [
        Icon(icon, size: 14),
        const SizedBox(width: 6),
        Text(text,
            style: const TextStyle(
                fontFamily: 'JetBrainsMono', fontSize: 12)),
      ]),
    );
  }
}
