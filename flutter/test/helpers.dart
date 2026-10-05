import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mcst_frontend/state/app_state.dart';
import 'package:mcst_frontend/theme.dart';
import 'package:provider/provider.dart';

import 'fake_client.dart';

/// Load the real app fonts so goldens render text, not Ahem blocks.
Future<void> loadAppFonts() async {
  for (final (family, path) in [
    ('Inter', 'assets/fonts/Inter.ttf'),
    ('JetBrainsMono', 'assets/fonts/JetBrainsMono.ttf'),
  ]) {
    final loader = FontLoader(family)
      ..addFont(Future.value(ByteData.sublistView(
          await File(path).readAsBytes())));
    await loader.load();
  }
}

/// Pump `page` inside the real app shell at a desktop size.
Future<void> pumpPage(
  WidgetTester tester,
  Widget page, {
  FakeApiClient? api,
  AppState? app,
  Size size = const Size(1440, 900),
}) async {
  final a = app ?? AppState(api ?? FakeApiClient(), enableStatsPoll: false);
  addTearDown(a.dispose);
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    ChangeNotifierProvider.value(
      value: a,
      child: MaterialApp(
        theme: AppTheme.dark(),
        home: page,
      ),
    ),
  );
  await settle(tester);
}

/// Bounded pump — `pumpAndSettle` never finishes while periodic timers
/// (stats ticker, console uptime) keep scheduling frames.
Future<void> settle(WidgetTester tester, [int frames = 20]) async {
  // Let real async work (asset image decode, WS) land between the
  // fake-time pumps — otherwise Image.asset races the golden capture.
  await tester.runAsync(() => Future<void>.delayed(
      const Duration(milliseconds: 100)));
  for (var i = 0; i < frames; i++) {
    await tester.pump(const Duration(milliseconds: 100));
  }
  await tester.runAsync(() => Future<void>.delayed(
      const Duration(milliseconds: 50)));
  await tester.pump();
}
