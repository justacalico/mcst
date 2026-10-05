import 'package:flutter_test/flutter_test.dart';
import 'package:mcst_frontend/api/types.dart';
import 'package:mcst_frontend/state/app_state.dart';
import 'package:mcst_frontend/views/auth_pages.dart';
import 'package:mcst_frontend/views/create_server_page.dart';
import 'package:mcst_frontend/views/dashboard_page.dart';
import 'package:mcst_frontend/views/server_detail_page.dart';
import 'package:mcst_frontend/views/settings_page.dart';

import 'fake_client.dart';
import 'helpers.dart';

void main() {
  setUpAll(loadAppFonts);

  Future<AppState> readyApp(WidgetTester tester, FakeApiClient api) async {
    final app = AppState(api, enableStatsPoll: false);
    await app.init();
    return app;
  }

  testWidgets('golden: setup page', (tester) async {
    await pumpPage(tester, const SetupPage());
    await expectLater(
        find.byType(SetupPage), matchesGoldenFile('goldens/setup.png'));
  });

  testWidgets('golden: login page', (tester) async {
    await pumpPage(tester, const LoginPage());
    await expectLater(
        find.byType(LoginPage), matchesGoldenFile('goldens/login.png'));
  });

  testWidgets('golden: unreachable page', (tester) async {
    await pumpPage(tester, UnreachablePage(onRetry: () {}));
    await expectLater(find.byType(UnreachablePage),
        matchesGoldenFile('goldens/unreachable.png'));
  });

  testWidgets('golden: dashboard empty', (tester) async {
    final api = FakeApiClient();
    final app = await readyApp(tester, api);
    await pumpPage(tester, const DashboardPage(), app: app);
    await expectLater(find.byType(DashboardPage),
        matchesGoldenFile('goldens/dashboard_empty.png'));
  });

  testWidgets('golden: dashboard with servers', (tester) async {
    final api = FakeApiClient(servers: [
      FakeApiClient.server(),
      FakeApiClient.server(
          id: 's2',
          name: 'Creative',
          type: 'fabric',
          status: ServerStatus.stopped,
          online: 0,
          names: const [],
          mem: 0),
      FakeApiClient.server(
          id: 's3',
          name: 'Modded',
          type: 'forge',
          status: ServerStatus.crashed,
          online: 0,
          names: const [],
          mem: 0),
    ]);
    final app = await readyApp(tester, api);
    await pumpPage(tester, const DashboardPage(), app: app);
    await expectLater(find.byType(DashboardPage),
        matchesGoldenFile('goldens/dashboard_servers.png'));
  });

  testWidgets('golden: create server wizard', (tester) async {
    final app = await readyApp(tester, FakeApiClient());
    await pumpPage(tester, const CreateServerPage(), app: app);
    await settle(tester);
    await expectLater(find.byType(CreateServerPage),
        matchesGoldenFile('goldens/create_wizard.png'));
  });

  testWidgets('golden: server detail console', (tester) async {
    final api = FakeApiClient(servers: [FakeApiClient.server()]);
    final app = await readyApp(tester, api);
    await pumpPage(tester, const ServerDetailPage(serverId: 'srv-1'),
        app: app);
    await settle(tester);
    await expectLater(find.byType(ServerDetailPage),
        matchesGoldenFile('goldens/server_console.png'));
  });

  testWidgets('golden: settings page', (tester) async {
    final app = await readyApp(tester, FakeApiClient());
    await pumpPage(tester, const SettingsPage(), app: app);
    await settle(tester);
    await expectLater(find.byType(SettingsPage),
        matchesGoldenFile('goldens/settings.png'));
  });
}
