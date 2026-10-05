import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../api/client.dart';
import '../state/app_state.dart';

/// Shared card layout for the setup and login pages.
class _AuthShell extends StatelessWidget {
  final String title;
  final String subtitle;
  final Widget child;
  const _AuthShell(
      {required this.title, required this.subtitle, required this.child});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 400),
          child: Card(
            child: Padding(
              padding: const EdgeInsets.all(32),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Row(
                    children: [
                      Image.asset('assets/icon.png', width: 40, height: 40),
                      const SizedBox(width: 12),
                      Text('mcst',
                          style: Theme.of(context)
                              .textTheme
                              .headlineSmall
                              ?.copyWith(fontWeight: FontWeight.w800)),
                    ],
                  ),
                  const SizedBox(height: 24),
                  Text(title, style: Theme.of(context).textTheme.titleLarge),
                  const SizedBox(height: 4),
                  Text(subtitle,
                      style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                          color: Theme.of(context).colorScheme.onSurfaceVariant)),
                  const SizedBox(height: 24),
                  child,
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// First-run: create the owner account.
class SetupPage extends StatefulWidget {
  const SetupPage({super.key});
  @override
  State<SetupPage> createState() => _SetupPageState();
}

class _SetupPageState extends State<SetupPage> {
  final _user = TextEditingController();
  final _pass = TextEditingController();
  final _confirm = TextEditingController();
  bool _busy = false;
  String? _error;

  Future<void> _submit() async {
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      if (_pass.text != _confirm.text) {
        throw const ApiException(0, 'passwords do not match');
      }
      await context
          .read<AppState>()
          .completeSetup(_user.text.trim(), _pass.text);
    } catch (e) {
      setState(() => _error = '$e');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  void dispose() {
    _user.dispose();
    _pass.dispose();
    _confirm.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return _AuthShell(
      title: 'Welcome to mcst',
      subtitle: 'Create your admin account to finish setup.',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          TextField(
              controller: _user,
              decoration: const InputDecoration(labelText: 'Username'),
              textInputAction: TextInputAction.next,
              autofillHints: const [AutofillHints.username]),
          const SizedBox(height: 12),
          TextField(
              controller: _pass,
              decoration: const InputDecoration(labelText: 'Password'),
              obscureText: true,
              textInputAction: TextInputAction.next,
              autofillHints: const [AutofillHints.newPassword]),
          const SizedBox(height: 12),
          TextField(
              controller: _confirm,
              decoration:
                  const InputDecoration(labelText: 'Confirm password'),
              obscureText: true,
              onSubmitted: (_) => _busy ? null : _submit(),
              autofillHints: const [AutofillHints.newPassword]),
          if (_error != null) ...[
            const SizedBox(height: 12),
            Text(_error!,
                style:
                    TextStyle(color: Theme.of(context).colorScheme.error)),
          ],
          const SizedBox(height: 20),
          FilledButton(
            onPressed: _busy ? null : _submit,
            child: _busy
                ? const SizedBox(
                    height: 20,
                    width: 20,
                    child: CircularProgressIndicator(strokeWidth: 2))
                : const Text('Create account'),
          ),
        ],
      ),
    );
  }
}

/// Login for existing installs.
class LoginPage extends StatefulWidget {
  const LoginPage({super.key});
  @override
  State<LoginPage> createState() => _LoginPageState();
}

class _LoginPageState extends State<LoginPage> {
  final _user = TextEditingController();
  final _pass = TextEditingController();
  bool _busy = false;
  String? _error;

  Future<void> _submit() async {
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await context.read<AppState>().login(_user.text.trim(), _pass.text);
    } catch (e) {
      setState(() => _error = '$e');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  void dispose() {
    _user.dispose();
    _pass.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return _AuthShell(
      title: 'Sign in',
      subtitle: 'Manage your Minecraft servers.',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          TextField(
              controller: _user,
              decoration: const InputDecoration(labelText: 'Username'),
              textInputAction: TextInputAction.next,
              autofillHints: const [AutofillHints.username]),
          const SizedBox(height: 12),
          TextField(
              controller: _pass,
              decoration: const InputDecoration(labelText: 'Password'),
              obscureText: true,
              onSubmitted: (_) => _busy ? null : _submit(),
              autofillHints: const [AutofillHints.password]),
          if (_error != null) ...[
            const SizedBox(height: 12),
            Text(_error!,
                style:
                    TextStyle(color: Theme.of(context).colorScheme.error)),
          ],
          const SizedBox(height: 20),
          FilledButton(
            onPressed: _busy ? null : _submit,
            child: _busy
                ? const SizedBox(
                    height: 20,
                    width: 20,
                    child: CircularProgressIndicator(strokeWidth: 2))
                : const Text('Sign in'),
          ),
        ],
      ),
    );
  }
}

/// Shown when the backend can't be reached (e.g. the static Pages copy).
class UnreachablePage extends StatelessWidget {
  final VoidCallback onRetry;
  const UnreachablePage({super.key, required this.onRetry});

  @override
  Widget build(BuildContext context) {
    return _AuthShell(
      title: 'Can’t reach the backend',
      subtitle:
          'mcst is self-hosted — this UI must be served by the mcst binary. '
          'If you are looking at a static mirror, run `mcst` locally instead.',
      child: FilledButton.icon(
        onPressed: onRetry,
        icon: const Icon(Icons.refresh),
        label: const Text('Retry'),
      ),
    );
  }
}
