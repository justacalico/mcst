import 'package:flutter/material.dart';

import 'api/types.dart';
import 'theme.dart';

/// Colored status dot + label.
class StatusBadge extends StatelessWidget {
  final String status;
  const StatusBadge(this.status, {super.key});

  @override
  Widget build(BuildContext context) {
    final c = statusColor(context, status);
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          width: 9,
          height: 9,
          decoration: BoxDecoration(color: c, shape: BoxShape.circle),
        ),
        const SizedBox(width: 6),
        Text(status,
            style: Theme.of(context)
                .textTheme
                .labelMedium
                ?.copyWith(color: c, fontWeight: FontWeight.w600)),
      ],
    );
  }
}

/// A horizontal progress bar with a label + value.
class UsageBar extends StatelessWidget {
  final String label;
  final double fraction;
  final String value;
  const UsageBar(this.label, this.fraction, this.value, {super.key});

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            Text(label, style: Theme.of(context).textTheme.labelLarge),
            Text(value,
                style: Theme.of(context).textTheme.labelMedium?.copyWith(
                    color: scheme.onSurfaceVariant,
                    fontFamily: 'JetBrainsMono')),
          ],
        ),
        const SizedBox(height: 6),
        ClipRRect(
          borderRadius: BorderRadius.circular(4),
          child: LinearProgressIndicator(
            value: fraction.clamp(0, 1),
            minHeight: 8,
            backgroundColor: scheme.surfaceContainerHigh,
          ),
        ),
      ],
    );
  }
}

/// Dashboard stat card: label + big value + subtitle.
class StatCard extends StatelessWidget {
  final IconData icon;
  final String label;
  final String value;
  final String subtitle;
  const StatCard(this.icon, this.label, this.value,
      {super.key, this.subtitle = ''});

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(children: [
              Icon(icon, size: 18, color: scheme.primary),
              const SizedBox(width: 8),
              Expanded(
                child: Text(label,
                    style: Theme.of(context)
                        .textTheme
                        .labelLarge
                        ?.copyWith(color: scheme.onSurfaceVariant),
                    overflow: TextOverflow.ellipsis),
              ),
            ]),
            const SizedBox(height: 10),
            Text(value,
                style: Theme.of(context)
                    .textTheme
                    .headlineSmall
                    ?.copyWith(fontWeight: FontWeight.w700)),
            if (subtitle.isNotEmpty)
              Padding(
                padding: const EdgeInsets.only(top: 2),
                child: Text(subtitle,
                    style: Theme.of(context)
                        .textTheme
                        .labelMedium
                        ?.copyWith(color: scheme.onSurfaceVariant),
                    overflow: TextOverflow.ellipsis),
              ),
          ],
        ),
      ),
    );
  }
}

/// Rounded chip carrying the server type's real logo; falls back to a
/// colored initial for unknown types.
class ServerTypeAvatar extends StatelessWidget {
  final String serverType;
  final double size;
  const ServerTypeAvatar(this.serverType, {super.key, this.size = 40});

  static const colors = {
    'vanilla': Color(0xFF8D6E63),
    'paper': Color(0xFF42A5F5),
    'purpur': Color(0xFFAB47BC),
    'fabric': Color(0xFFFFB300),
    'forge': Color(0xFFEF5350),
    'neoforge': Color(0xFFFF7043),
    'custom': Color(0xFF78909C),
  };

  static const _assets = {
    'vanilla': 'assets/icons/vanilla.png',
    'paper': 'assets/icons/paper.png',
    'purpur': 'assets/icons/purpur.png',
    'fabric': 'assets/icons/fabric.png',
    'forge': 'assets/icons/forge.jpg',
    'neoforge': 'assets/icons/neoforge.png',
    'custom': 'assets/icons/custom.png',
  };

  @override
  Widget build(BuildContext context) {
    final c = colors[serverType] ?? colors['custom']!;
    final asset = _assets[serverType];
    return Container(
      width: size,
      height: size,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: c.withAlpha(40),
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: c.withAlpha(90)),
      ),
      clipBehavior: Clip.antiAlias,
      child: asset != null
          ? Image.asset(asset,
              width: size * 0.82,
              height: size * 0.82,
              filterQuality: FilterQuality.low,
              errorBuilder: (_, _, _) => _initial(c))
          : _initial(c),
    );
  }

  Widget _initial(Color c) {
    final initial = serverType.isEmpty ? '?' : serverType[0].toUpperCase();
    return Text(initial,
        style: TextStyle(
            color: c, fontWeight: FontWeight.w800, fontSize: size * 0.45));
  }
}

/// Card on the dashboard grid.
class ServerCard extends StatelessWidget {
  final ServerDto server;
  final VoidCallback onTap;
  const ServerCard({super.key, required this.server, required this.onTap});

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final running = server.status == ServerStatus.running;
    return Card(
      child: InkWell(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  ServerTypeAvatar(server.serverType),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(server.name,
                            style: Theme.of(context)
                                .textTheme
                                .titleMedium
                                ?.copyWith(fontWeight: FontWeight.w700),
                            overflow: TextOverflow.ellipsis),
                        Text(server.typeLabel,
                            style: Theme.of(context)
                                .textTheme
                                .labelMedium
                                ?.copyWith(color: scheme.onSurfaceVariant)),
                      ],
                    ),
                  ),
                  StatusBadge(server.status.name),
                ],
              ),
              const Spacer(),
              Row(
                children: [
                  _metric(context, Icons.people,
                      '${server.playersOnline}/${server.playersMax}'),
                  const SizedBox(width: 16),
                  _metric(context, Icons.public, ':${server.port}'),
                  const Spacer(),
                  if (running)
                    Text(
                      '${server.cpuPercent.toStringAsFixed(0)}% · ${humanBytes(server.memBytes)}',
                      style: Theme.of(context)
                          .textTheme
                          .labelMedium
                          ?.copyWith(
                              color: scheme.onSurfaceVariant,
                              fontFamily: 'JetBrainsMono'),
                    ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _metric(BuildContext context, IconData icon, String text) {
    final scheme = Theme.of(context).colorScheme;
    return Row(mainAxisSize: MainAxisSize.min, children: [
      Icon(icon, size: 14, color: scheme.onSurfaceVariant),
      const SizedBox(width: 4),
      Text(text,
          style: Theme.of(context)
              .textTheme
              .labelMedium
              ?.copyWith(color: scheme.onSurfaceVariant)),
    ]);
  }
}

/// Page scaffold with a max content width (desktop-first layout).
class PageBody extends StatelessWidget {
  final Widget child;
  final double maxWidth;
  const PageBody({super.key, required this.child, this.maxWidth = 1280});

  @override
  Widget build(BuildContext context) {
    return Center(
      child: ConstrainedBox(
        constraints: BoxConstraints(maxWidth: maxWidth),
        child: child,
      ),
    );
  }
}

/// Async error + retry row.
class ErrorCard extends StatelessWidget {
  final Object error;
  final VoidCallback? onRetry;
  const ErrorCard(this.error, {super.key, this.onRetry});

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Row(
          children: [
            Icon(Icons.error_outline, color: Theme.of(context).colorScheme.error),
            const SizedBox(width: 12),
            Expanded(child: Text('$error')),
            if (onRetry != null)
              TextButton.icon(
                  onPressed: onRetry,
                  icon: const Icon(Icons.refresh),
                  label: const Text('Retry')),
          ],
        ),
      ),
    );
  }
}

/// Shows a snackbar for an action's error.
void showError(BuildContext context, Object e) {
  ScaffoldMessenger.of(context).showSnackBar(
    SnackBar(content: Text('$e')),
  );
}

/// Single-line text prompt. The controller lives inside the dialog's own
/// State, so it is disposed with the route — not between `pop()` and the end
/// of the pop animation, where a caller-side `dispose()` would still find the
/// TextField mounted.
Future<String?> promptText(
  BuildContext context, {
  required String title,
  String label = '',
  String hint = '',
  String initial = '',
  TextInputType? keyboardType,
  String confirmLabel = 'Save',
  bool danger = false,
  String? body,
}) =>
    showDialog<String>(
      context: context,
      builder: (_) => _TextPrompt(
          title: title,
          label: label,
          hint: hint,
          initial: initial,
          keyboardType: keyboardType,
          confirmLabel: confirmLabel,
          danger: danger,
          body: body),
    );

class _TextPrompt extends StatefulWidget {
  final String title, label, hint, initial, confirmLabel;
  final TextInputType? keyboardType;
  final bool danger;
  final String? body;
  const _TextPrompt(
      {required this.title,
      this.label = '',
      this.hint = '',
      this.initial = '',
      this.keyboardType,
      this.confirmLabel = 'Save',
      this.danger = false,
      this.body});

  @override
  State<_TextPrompt> createState() => _TextPromptState();
}

class _TextPromptState extends State<_TextPrompt> {
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
        title: Text(widget.title),
        content: Column(mainAxisSize: MainAxisSize.min, children: [
          if (widget.body != null) ...[
            Text(widget.body!),
            const SizedBox(height: 12),
          ],
          TextField(
              controller: _c,
              autofocus: true,
              keyboardType: widget.keyboardType,
              decoration: InputDecoration(
                  labelText: widget.label.isEmpty ? null : widget.label,
                  hintText: widget.hint.isEmpty ? null : widget.hint)),
        ]),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(context),
              child: const Text('Cancel')),
          FilledButton(
              style: widget.danger
                  ? FilledButton.styleFrom(
                      backgroundColor: Theme.of(context).colorScheme.error)
                  : null,
              onPressed: () => Navigator.pop(context, _c.text.trim()),
              child: Text(widget.confirmLabel)),
        ],
      );
}

/// Two-field password change prompt (current + new).
Future<(String, String)?> promptPasswordChange(BuildContext context) =>
    showDialog<(String, String)>(
        context: context, builder: (_) => const _PasswordPrompt());

class _PasswordPrompt extends StatefulWidget {
  const _PasswordPrompt();

  @override
  State<_PasswordPrompt> createState() => _PasswordPromptState();
}

class _PasswordPromptState extends State<_PasswordPrompt> {
  final _cur = TextEditingController();
  final _next = TextEditingController();

  @override
  void dispose() {
    _cur.dispose();
    _next.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
        title: const Text('Change password'),
        content: Column(mainAxisSize: MainAxisSize.min, children: [
          TextField(
              controller: _cur,
              obscureText: true,
              decoration: const InputDecoration(labelText: 'Current password')),
          const SizedBox(height: 12),
          TextField(
              controller: _next,
              obscureText: true,
              decoration: const InputDecoration(labelText: 'New password')),
        ]),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(context),
              child: const Text('Cancel')),
          FilledButton(
              onPressed: () =>
                  Navigator.pop(context, (_cur.text, _next.text)),
              child: const Text('Change')),
        ],
      );
}
