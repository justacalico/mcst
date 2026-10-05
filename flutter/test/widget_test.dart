import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mcst_frontend/state/app_state.dart';
import 'package:mcst_frontend/views/auth_pages.dart';
import 'package:mcst_frontend/views/create_server_page.dart';
import 'package:mcst_frontend/views/dashboard_page.dart';
import 'package:mcst_frontend/views/server/settings_tab.dart';
import 'package:mcst_frontend/views/server_detail_page.dart';
import 'package:mcst_frontend/views/settings_page.dart';

import 'fake_client.dart';
import 'helpers.dart';

void main() {
  group('auth pages', () {
    testWidgets('setup validates matching passwords', (tester) async {
      await pumpPage(tester, const SetupPage());
      await tester.enterText(
          find.widgetWithText(TextField, 'Username'), 'owner');
      await tester.enterText(
          find.widgetWithText(TextField, 'Password'), 'password-1');
      await tester.enterText(
          find.widgetWithText(TextField, 'Confirm password'), 'different');
      await tester.tap(find.text('Create account'));
      await settle(tester);
      expect(find.text('passwords do not match'), findsOneWidget);
    });

    testWidgets('setup completes and notifies app', (tester) async {
      final api = FakeApiClient()..setupNeeded = true;
      final app = AppState(api, enableStatsPoll: false);
      await pumpPage(tester, const SetupPage(), app: app);
      await tester.enterText(
          find.widgetWithText(TextField, 'Username'), 'owner');
      await tester.enterText(
          find.widgetWithText(TextField, 'Password'), 'password-1');
      await tester.enterText(
          find.widgetWithText(TextField, 'Confirm password'), 'password-1');
      await tester.tap(find.text('Create account'));
      await settle(tester);
      expect(app.session, SessionState.ready);
      expect(app.username, 'owner');
    });

    testWidgets('login error is shown', (tester) async {
      final api = FakeApiClient();
      final app = AppState(api, enableStatsPoll: false);
      await pumpPage(tester, const LoginPage(), app: app);
      await tester.enterText(
          find.widgetWithText(TextField, 'Username'), 'owner');
      await tester.enterText(
          find.widgetWithText(TextField, 'Password'), 'wrong');
      await tester.tap(find.widgetWithText(FilledButton, 'Sign in'));
      await settle(tester);
      expect(find.textContaining('invalid credentials'), findsOneWidget);
    });
  });

  group('dashboard', () {
    testWidgets('renders servers and empty state', (tester) async {
      final api = FakeApiClient();
      final app = AppState(api, enableStatsPoll: false);
      await app.init();
      await pumpPage(tester, const DashboardPage(), app: app);
      expect(find.text('No servers yet'), findsOneWidget);
      expect(find.text('New server'), findsOneWidget);
    });

    testWidgets('server card opens detail page', (tester) async {
      final api =
          FakeApiClient(servers: [FakeApiClient.server(name: 'SMP')]);
      final app = AppState(api, enableStatsPoll: false);
      await app.init();
      await pumpPage(tester, const DashboardPage(), app: app);
      await tester.tap(find.text('SMP'));
      await settle(tester);
      expect(find.byType(ServerDetailPage), findsOneWidget);
      expect(find.text('Console'), findsOneWidget);
    });
  });

  group('create wizard', () {
    testWidgets('type picker gates continue', (tester) async {
      final app = AppState(FakeApiClient(), enableStatsPoll: false);
      await pumpPage(tester, const CreateServerPage(), app: app);
      await settle(tester);
      expect(find.text('Type'), findsWidgets);
      await tester.tap(find.text('Paper'));
      await settle(tester);
      await tester.tap(find.text('Continue'));
      await settle(tester);
      // Now on version step; choose a version.
      await tester.tap(find.byType(DropdownMenu<String>).first);
      await settle(tester);
      await tester.tap(find.text('1.21.1').last);
      await settle(tester);
      await tester.tap(find.text('Continue'));
      await settle(tester);
      expect(find.text('Server name'), findsOneWidget);
      expect(find.text('I accept the Minecraft EULA'), findsOneWidget);
    });
  });

  group('server detail', () {
    testWidgets('all tabs render', (tester) async {
      final api = FakeApiClient(servers: [FakeApiClient.server()]);
      final app = AppState(api, enableStatsPoll: false);
      await app.init();
      await pumpPage(tester, const ServerDetailPage(serverId: 'srv-1'),
          app: app);
      await settle(tester);
      for (final t in [
        'Console', 'Files', 'Players', 'Content', 'Backups', 'Schedules', 'Settings'
      ]) {
        expect(find.text(t), findsOneWidget);
      }
      // Switch to files tab → file list appears.
      await tester.tap(find.text('Files'));
      await settle(tester);
      expect(find.text('server.properties'), findsOneWidget);
      // Players tab → ops entry.
      await tester.tap(find.text('Players'));
      await settle(tester);
      expect(find.text('Steve'), findsWidgets);
      // Backups tab.
      await tester.tap(find.text('Backups'));
      await settle(tester);
      expect(find.text('before update'), findsOneWidget);
      // Schedules tab.
      await tester.tap(find.text('Schedules'));
      await settle(tester);
      expect(find.text('nightly restart'), findsOneWidget);
      // Settings tab (scrollable TabBar — bring it on screen first).
      await tester.ensureVisible(find.text('Settings'));
      await settle(tester);
      await tester.tap(find.text('Settings'));
      await settle(tester);
      final settingsList = find.descendant(
        of: find.byType(ServerSettingsTab),
        matching: find.byType(ListView),
      );
      for (var i = 0;
          i < 8 && find.text('Danger zone').evaluate().isEmpty;
          i++) {
        await tester.drag(settingsList.first, const Offset(0, -500));
        await settle(tester);
      }
      expect(find.text('Danger zone'), findsOneWidget);
    });

    testWidgets('console receives lines and sends commands', (tester) async {
      final api = FakeApiClient(servers: [FakeApiClient.server()]);
      final app = AppState(api, enableStatsPoll: false);
      await app.init();
      await pumpPage(tester, const ServerDetailPage(serverId: 'srv-1'),
          app: app);
      await settle(tester);
      api.pushConsoleLine('[12:00:00] [Server thread/INFO]: Done (1.0s)!');
      await settle(tester);
      expect(find.textContaining('Done (1.0s)'), findsOneWidget);
      // Send a command.
      await tester.enterText(
          find.widgetWithText(TextField, 'Type a command… (e.g. say hello, list, stop)'),
          'say hi');
      await tester.tap(find.text('Send'));
      await settle(tester);
      expect(api.calls, contains('cmd:srv-1:say hi'));
    });
  });

  group('settings page', () {
    testWidgets('renders account, java, tailscale, audit', (tester) async {
      final app = AppState(FakeApiClient(), enableStatsPoll: false);
      await pumpPage(tester, const SettingsPage(), app: app);
      await settle(tester);
      expect(find.text('Account'), findsOneWidget);
      expect(find.text('Java runtimes'), findsOneWidget);
      expect(find.text('Tailscale'), findsOneWidget);
      expect(find.text('Activity'), findsOneWidget);
      expect(find.textContaining('homelab.tail.ts.net'), findsWidgets);
      expect(find.textContaining('Java 21'), findsOneWidget);
      expect(find.textContaining('server_start'), findsOneWidget);
    });
  });
}
