import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mcst_frontend/state/app_state.dart';
import 'package:mcst_frontend/views/create_server_page.dart';
import 'package:mcst_frontend/views/dashboard_page.dart';
import 'package:mcst_frontend/views/settings_page.dart';
import 'package:mcst_frontend/widgets.dart';

import 'fake_client.dart';
import 'helpers.dart';

void main() {
  group('settings page interactions', () {
    testWidgets('install java dialog downloads a runtime', (tester) async {
      final app = AppState(FakeApiClient(), enableStatsPoll: false);
      await pumpPage(tester, const SettingsPage(), app: app);
      await settle(tester);
      await tester.tap(find.text('Install a Java version'));
      await settle(tester);
      expect(find.text('Install Java (Temurin)'), findsOneWidget);
      await tester.enterText(
          find.widgetWithText(TextField, 'Major version'), '21');
      await tester.tap(find.widgetWithText(FilledButton, 'Download'));
      await settle(tester);
      expect(find.text('Java 21 installed'), findsOneWidget);
    });

    testWidgets('change password dialog submits both fields',
        (tester) async {
      final api = FakeApiClient();
      final app = AppState(api, enableStatsPoll: false);
      await app.init();
      await pumpPage(tester, const SettingsPage(), app: app);
      await settle(tester);
      await tester.tap(find.text('Change password'));
      await settle(tester);
      await tester.enterText(
          find.widgetWithText(TextField, 'Current password'), 'current');
      await tester.enterText(
          find.widgetWithText(TextField, 'New password'), 'new-pw-456');
      await tester.tap(find.widgetWithText(FilledButton, 'Change'));
      await settle(tester);
      expect(find.textContaining('Password changed'), findsOneWidget);
    });

    testWidgets('tailscale switch toggles panel serve', (tester) async {
      final app = AppState(FakeApiClient(), enableStatsPoll: false);
      await pumpPage(tester, const SettingsPage(), app: app);
      await settle(tester);
      expect(find.text('homelab.tail.ts.net · 100.64.1.5'), findsOneWidget);
      await tester.tap(find.text('Serve panel over tailnet'));
      await settle(tester);
    });
  });

  group('dashboard interactions', () {
    testWidgets('new server button opens the wizard', (tester) async {
      final app = AppState(FakeApiClient(), enableStatsPoll: false);
      await app.init();
      await pumpPage(tester, const DashboardPage(), app: app);
      await settle(tester);
      await tester.tap(find.text('New server'));
      await settle(tester);
      expect(find.byType(CreateServerPage), findsOneWidget);
    });

    testWidgets('settings icon opens settings page', (tester) async {
      final app = AppState(FakeApiClient(), enableStatsPoll: false);
      await app.init();
      await pumpPage(tester, const DashboardPage(), app: app);
      await settle(tester);
      await tester.tap(find.byTooltip('Settings'));
      await settle(tester);
      expect(find.byType(SettingsPage), findsOneWidget);
    });

    testWidgets('sign out returns to logged out state', (tester) async {
      final api = FakeApiClient();
      final app = AppState(api, enableStatsPoll: false);
      await app.init();
      await pumpPage(tester, const DashboardPage(), app: app);
      await settle(tester);
      await tester.tap(find.byTooltip('Sign out'));
      await settle(tester);
      expect(app.session, SessionState.loggedOut);
    });
  });

  group('widgets', () {
    testWidgets('error card retry callback fires', (tester) async {
      var retried = false;
      await pumpPage(
          tester,
          Scaffold(
              body: ErrorCard('boom', onRetry: () => retried = true)));
      expect(find.textContaining('boom'), findsOneWidget);
      await tester.tap(find.text('Retry'));
      await settle(tester);
      expect(retried, isTrue);
    });
  });

  group('app state errors', () {
    test('refreshServers records api failure', () async {
      final api = FakeApiClient()..failNext = true;
      final app = AppState(api, enableStatsPoll: false);
      addTearDown(app.dispose);
      await app.refreshServers();
      expect(app.lastError, isNotNull);
    });

    test('login propagates auth errors', () async {
      final api = FakeApiClient();
      final app = AppState(api, enableStatsPoll: false);
      addTearDown(app.dispose);
      expect(() => app.login('a', 'wrong'), throwsA(anything));
    });
  });
}
