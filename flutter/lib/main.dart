import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import 'api/client.dart';
import 'state/app_state.dart';
import 'theme.dart';
import 'views/auth_pages.dart';
import 'views/dashboard_page.dart';

void main() {
  runApp(const McstApp());
}

class McstApp extends StatefulWidget {
  const McstApp({super.key});

  @override
  State<McstApp> createState() => _McstAppState();
}

class _McstAppState extends State<McstApp> {
  late final AppState _app;

  @override
  void initState() {
    super.initState();
    _app = AppState(HttpApiClient());
    _app.init();
  }

  @override
  void dispose() {
    _app.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider.value(
      value: _app,
      child: MaterialApp(
        title: 'mcst',
        debugShowCheckedModeBanner: false,
        theme: AppTheme.light(),
        darkTheme: AppTheme.dark(),
        themeMode: ThemeMode.dark,
        home: const _Gate(),
      ),
    );
  }
}

/// Picks the top-level page from session state.
class _Gate extends StatelessWidget {
  const _Gate();

  @override
  Widget build(BuildContext context) {
    final app = context.watch<AppState>();
    return switch (app.session) {
      SessionState.unknown => const Scaffold(
          body: Center(child: CircularProgressIndicator())),
      SessionState.needsSetup => const SetupPage(),
      SessionState.loggedOut => const LoginPage(),
      SessionState.ready =>
        app.apiUnreachable
            ? UnreachablePage(onRetry: app.init)
            : const DashboardPage(),
    };
  }
}
