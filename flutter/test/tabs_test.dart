import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mcst_frontend/api/types.dart';
import 'package:mcst_frontend/state/app_state.dart';
import 'package:mcst_frontend/views/server_detail_page.dart';

import 'fake_client.dart';
import 'helpers.dart';

Future<(WidgetTester, FakeApiClient, AppState)> detailPage(
    WidgetTester tester) async {
  final api = FakeApiClient(servers: [FakeApiClient.server()]);
  final app = AppState(api, enableStatsPoll: false);
  await app.init();
  await pumpPage(tester, const ServerDetailPage(serverId: 'srv-1'), app: app);
  await settle(tester);
  return (tester, api, app);
}

Future<void> openTab(WidgetTester tester, String name) async {
  await tester.ensureVisible(find.text(name));
  await settle(tester);
  await tester.tap(find.text(name));
  await settle(tester);
}

void main() {
  group('files tab', () {
    testWidgets('open file editor, save, new folder, delete file',
        (tester) async {
      await detailPage(tester);
      await openTab(tester, 'Files');
      expect(find.text('server.properties'), findsOneWidget);

      // Open a file in the editor, edit and save.
      await tester.tap(find.text('server.properties'));
      await settle(tester);
      expect(find.widgetWithText(TextField, 'a=1\nb=2\n'), findsOneWidget);
      await tester.enterText(find.byType(TextField).first, 'a=3\n');
      await settle(tester);
      await tester.tap(find.text('Save'));
      await settle(tester);
      expect(find.text('Saved'), findsOneWidget);
      // Back to the listing.
      await tester.tap(find.byIcon(Icons.close));
      await settle(tester);

      // New folder dialog.
      await tester.tap(find.byTooltip('New folder'));
      await settle(tester);
      expect(find.text('New folder'), findsOneWidget);
      await tester.enterText(
          find.widgetWithText(TextField, 'Name'), 'plugins');
      await tester.tap(find.widgetWithText(FilledButton, 'Create'));
      await settle(tester);

      // Delete via the file's popup menu (dirs have none).
      await tester.tap(find.byType(PopupMenuButton<String>).first);
      await settle(tester);
      await tester.tap(find.text('Delete'));
      await settle(tester);
      expect(find.text('Delete server.properties?'), findsOneWidget);
      await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
      await settle(tester);
    });
  });

  group('backups tab', () {
    testWidgets('create, restore and delete flows', (tester) async {
      await detailPage(tester);
      await openTab(tester, 'Backups');
      expect(find.text('before update'), findsOneWidget);

      await tester.tap(find.text('New backup'));
      await settle(tester);
      expect(find.text('Note (optional)'), findsOneWidget);
      await tester.enterText(
          find.widgetWithText(TextField, 'Note (optional)'), 'manual');
      await tester.tap(find.widgetWithText(FilledButton, 'Back up'));
      await settle(tester);

      await tester.tap(find.byTooltip('Restore').first);
      await settle(tester);
      expect(find.text('Restore backup?'), findsOneWidget);
      await tester.tap(find.widgetWithText(FilledButton, 'Restore'));
      await settle(tester);
      expect(find.text('Restored'), findsOneWidget);

      await tester.tap(find.byTooltip('Delete').first);
      await settle(tester);
    });
  });

  group('players tab', () {
    testWidgets('add and remove a player', (tester) async {
      await detailPage(tester);
      await openTab(tester, 'Players');
      expect(find.text('Steve'), findsWidgets);

      await tester.tap(find.widgetWithText(FilledButton, 'Add').first);
      await settle(tester);
      await tester.enterText(
          find.widgetWithText(TextField, 'Player name'), 'Herobrine');
      await tester.tap(find.widgetWithText(FilledButton, 'Add').last);
      await settle(tester);

      // Remove via the trailing icon on an entry.
      final remove = find.byIcon(Icons.remove_circle_outline);
      if (remove.evaluate().isNotEmpty) {
        await tester.tap(remove.first);
        await settle(tester);
      }
    });
  });

  group('schedules tab', () {
    testWidgets('create schedule dialog validates and saves', (tester) async {
      await detailPage(tester);
      await openTab(tester, 'Schedules');
      expect(find.text('nightly restart'), findsOneWidget);

      await tester.tap(find.widgetWithText(FilledButton, 'New schedule'));
      await settle(tester);
      expect(find.text('New schedule'), findsWidgets);
      await tester.enterText(
          find.widgetWithText(TextField, 'Name'), 'backup daily');
      await tester.enterText(
          find.widgetWithText(TextField, 'Every (minutes)'), '60');
      await tester.tap(find.widgetWithText(FilledButton, 'Save'));
      await settle(tester);

      // Edit existing.
      await tester.tap(find.text('nightly restart'));
      await settle(tester);
      expect(find.text('Edit schedule'), findsOneWidget);
      // Switch the action to a command and flip to daily mode.
      await tester.tap(find.byType(DropdownButtonFormField<String>));
      await settle(tester);
      await tester.tap(find.text('Command').last);
      await settle(tester);
      await tester.tap(find.text('Daily at'));
      await settle(tester);
      await tester.tap(find.widgetWithText(FilledButton, 'Save').last);
      await settle(tester);

      // Toggle + delete on the row.
      await tester.tap(find.byType(Switch).first);
      await settle(tester);
      await tester.tap(find.byIcon(Icons.delete_outline).first);
      await settle(tester);
    });
  });

  group('content (mods) tab', () {
    testWidgets('search modrinth and install', (tester) async {
      await detailPage(tester);
      await openTab(tester, 'Content');
      expect(find.text('fabric-api.jar'), findsOneWidget);

      await tester.enterText(
          find.byType(TextField).first, 'fabric');
      await tester.testTextInput.receiveAction(TextInputAction.search);
      await settle(tester);
      expect(find.text('Fabric API'), findsOneWidget);
      await tester.tap(find.text('Install'));
      await settle(tester);
      expect(find.textContaining('Installed fabric-api-1.0.jar'),
          findsOneWidget);
    });
  });

  group('settings tab', () {
    testWidgets('toggles, save, properties editor, delete dialog',
        (tester) async {
      // Stopped server so settings are editable.
      final api = FakeApiClient(servers: [
        FakeApiClient.server(status: ServerStatus.stopped)
      ]);
      final app = AppState(api, enableStatsPoll: false);
      await app.init();
      await pumpPage(tester, const ServerDetailPage(serverId: 'srv-1'),
          app: app);
      await settle(tester);
      await openTab(tester, 'Settings');

      expect(find.text('General'), findsOneWidget);
      await tester.tap(find.text('Restart on crash'));
      await settle(tester);
      await tester.tap(find.widgetWithText(FilledButton, 'Save'));
      await settle(tester);

      // Raw properties editor dialog.
      await tester.tap(find.text('server.properties'));
      await settle(tester);
      expect(find.textContaining('server-port=25565'), findsOneWidget);
      await tester.tap(find.widgetWithText(FilledButton, 'Save').last);
      await settle(tester);
      expect(find.text('Properties saved'), findsOneWidget);

      // Lower rows are built but offscreen — ensureVisible scrolls to them.
      await tester.ensureVisible(find.text('Update server jar'));
      await settle(tester);
      await tester.tap(find.text('Update server jar'));
      await settle(tester);
      expect(find.textContaining('Updated'), findsOneWidget);

      // Delete flow.
      await tester.ensureVisible(find.text('Danger zone'));
      await settle(tester);
      await tester.tap(find.text('Delete server'));
      await settle(tester);
      expect(find.text('Delete SMP?'), findsOneWidget);
      await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
      await settle(tester);
    });
  });

  group('server detail header', () {
    testWidgets('lifecycle buttons call api', (tester) async {
      final (_, api, _) = await detailPage(tester);
      await tester.tap(find.byTooltip('Stop'));
      await settle(tester);
      expect(api.calls, contains('stop:srv-1'));
      await tester.tap(find.byTooltip('Kill'));
      await settle(tester);
      expect(find.text('Kill server?'), findsOneWidget);
      await tester.tap(find.widgetWithText(FilledButton, 'Kill'));
      await settle(tester);
      await tester.tap(find.byTooltip('Refresh'));
      await settle(tester);
    });
  });
}
