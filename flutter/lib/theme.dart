import 'package:flutter/material.dart';

/// mcst theme — dark-first Material 3 with a grass-green seed.
class AppTheme {
  static const seed = Color(0xFF5CBC4A);

  static ThemeData dark() {
    final scheme = ColorScheme.fromSeed(
      seedColor: seed,
      brightness: Brightness.dark,
      surface: const Color(0xFF141A16),
      surfaceContainerLowest: const Color(0xFF0D120E),
      surfaceContainer: const Color(0xFF1A211C),
      surfaceContainerHigh: const Color(0xFF232B25),
    );
    return _base(scheme);
  }

  static ThemeData light() {
    final scheme = ColorScheme.fromSeed(
      seedColor: seed,
      brightness: Brightness.light,
    );
    return _base(scheme);
  }

  static ThemeData _base(ColorScheme scheme) {
    return ThemeData(
      colorScheme: scheme,
      useMaterial3: true,
      fontFamily: 'Inter',
      scaffoldBackgroundColor: scheme.surfaceContainerLowest,
      cardTheme: CardThemeData(
        color: scheme.surfaceContainer,
        clipBehavior: Clip.antiAlias,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(14)),
      ),
      inputDecorationTheme: InputDecorationTheme(
        border: OutlineInputBorder(borderRadius: BorderRadius.circular(10)),
        isDense: true,
      ),
      snackBarTheme: const SnackBarThemeData(behavior: SnackBarBehavior.floating),
      dividerTheme: DividerThemeData(color: scheme.outlineVariant.withAlpha(60)),
      tabBarTheme: const TabBarThemeData(dividerColor: Colors.transparent),
    );
  }
}

/// Status → color mapping shared by badges/dots.
Color statusColor(BuildContext context, String status) {
  final scheme = Theme.of(context).colorScheme;
  return switch (status) {
    'running' => const Color(0xFF5CBC4A),
    'starting' || 'stopping' || 'installing' => Colors.orange,
    'crashed' => scheme.error,
    _ => scheme.outline,
  };
}
