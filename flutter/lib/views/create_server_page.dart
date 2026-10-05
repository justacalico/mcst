import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../api/client.dart';
import '../api/types.dart';
import '../state/app_state.dart';
import '../widgets.dart';
import 'server_detail_page.dart';

/// Create-server wizard: type → version → options → review.
class CreateServerPage extends StatefulWidget {
  const CreateServerPage({super.key});
  @override
  State<CreateServerPage> createState() => _CreateServerPageState();
}

class _CreateServerPageState extends State<CreateServerPage> {
  int _step = 0;

  List<ServerTypeInfo> _types = const [];
  String? _type;
  List<String> _versions = const [];
  String? _version;
  List<String> _loaders = const [];
  String? _loader;
  int _requiredJava = 21;
  List<JavaInstall> _java = const [];
  String _javaPath = '';

  final _name = TextEditingController(text: 'My Server');
  final _port = TextEditingController(text: '25565');
  final _memory = TextEditingController(text: '4096');
  final _jvmArgs = TextEditingController();
  bool _eula = false;
  int _pickSeq = 0;

  bool _loading = false;
  String? _error;
  bool _created = false;

  ApiClient get api => context.read<AppState>().api;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final types = await api.serverTypes();
      final java = await api.listJava();
      setState(() {
        _types = types;
        _java = java;
      });
    } catch (e) {
      if (mounted) setState(() => _error = '$e');
    }
  }

  Future<void> _pickType(String t) async {
    final seq = ++_pickSeq;
    setState(() {
      _type = t;
      _version = null;
      _loader = null;
      _versions = const [];
      _loaders = const [];
    });
    try {
      final v = await api.mcVersions(t);
      if (seq == _pickSeq && mounted) setState(() => _versions = v);
    } catch (e) {
      if (seq == _pickSeq && mounted) setState(() => _error = '$e');
    }
  }

  Future<void> _pickVersion(String v) async {
    final seq = ++_pickSeq;
    setState(() {
      _version = v;
      _loader = null;
      _loaders = const [];
    });
    try {
      final l = await api.loaders(_type!, v);
      final req = await api.requiredJava(v);
      if (seq != _pickSeq || !mounted) return;
      setState(() {
        _loaders = l;
        _loader = l.isEmpty ? null : l.first;
        _requiredJava = req;
        _javaPath = _suggestJava(req);
      });
    } catch (e) {
      if (seq == _pickSeq && mounted) setState(() => _error = '$e');
    }
  }

  String _suggestJava(int required) {
    if (_java.isEmpty) return 'java';
    // Exact, else closest higher, else highest.
    final exact = _java.where((j) => j.major == required);
    if (exact.isNotEmpty) return exact.first.path;
    final higher = _java.where((j) => j.major > required);
    if (higher.isNotEmpty) return higher.first.path;
    return _java.last.path;
  }

  bool get _hasLoaderStep =>
      _types.any((t) => t.id == _type && t.hasLoader);

  Future<void> _create() async {
    setState(() {
      _busy();
    });
    try {
      final app = context.read<AppState>();
      final nav = Navigator.of(context);
      final s = await api.createServer({
        'name': _name.text.trim(),
        'server_type': _type,
        'mc_version': _version ?? '',
        'loader_version': _loader ?? '',
        'port': int.tryParse(_port.text) ?? 25565,
        'memory_mb': int.tryParse(_memory.text) ?? 4096,
        'jvm_args': _jvmArgs.text.trim(),
        'java_path': _javaPath,
        'accept_eula': _eula,
      });
      await app.refreshServers();
      if (!mounted) return;
      setState(() {
        _created = true;
      });
      nav.pushReplacement(MaterialPageRoute(
          builder: (_) => ServerDetailPage(serverId: s.id)));
    } catch (e) {
      if (mounted) setState(() => _error = '$e');
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  void _busy() {
    _loading = true;
    _error = null;
  }

  @override
  void dispose() {
    _name.dispose();
    _port.dispose();
    _memory.dispose();
    _jvmArgs.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('New server')),
      body: PageBody(
        maxWidth: 720,
        child: ListView(
          padding: const EdgeInsets.all(20),
          children: [
            if (_error != null) ErrorCard(_error!),
            Stepper(
              currentStep: _step,
              onStepContinue: _next,
              onStepCancel: _step > 0 ? () => setState(() => _step--) : null,
              controlsBuilder: _controls,
              steps: [
                Step(
                  title: const Text('Type'),
                  isActive: _step >= 0,
                  state: _step > 0 ? StepState.complete : StepState.indexed,
                  content: _typePicker(),
                ),
                Step(
                  title: const Text('Version'),
                  isActive: _step >= 1,
                  state: _step > 1 ? StepState.complete : StepState.indexed,
                  content: _versionPicker(),
                ),
                Step(
                  title: const Text('Options'),
                  isActive: _step >= 2,
                  state: _step > 2 ? StepState.complete : StepState.indexed,
                  content: _options(),
                ),
                Step(
                  title: const Text('Create'),
                  isActive: _step >= 3,
                  content: _review(),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _controls(BuildContext context, ControlsDetails d) {
    if (d.stepIndex != d.currentStep) return const SizedBox.shrink();
    return Padding(
      padding: const EdgeInsets.only(top: 12),
      child: Row(
        children: [
          FilledButton(
            onPressed: _loading ? null : d.onStepContinue,
            child: _loading && _step == 3
                ? const SizedBox(
                    height: 18,
                    width: 18,
                    child: CircularProgressIndicator(strokeWidth: 2))
                : Text(_step == 3 ? 'Create server' : 'Continue'),
          ),
          if (_step > 0) ...[
            const SizedBox(width: 8),
            TextButton(onPressed: d.onStepCancel, child: const Text('Back')),
          ],
        ],
      ),
    );
  }

  bool _canContinue() => switch (_step) {
        0 => _type != null,
        1 =>
          _type == 'custom' ||
              (_version != null && (!_hasLoaderStep || _loader != null)),
        2 =>
          _name.text.trim().isNotEmpty &&
              (int.tryParse(_port.text) ?? 0) >= 1024 &&
              (int.tryParse(_memory.text) ?? 0) >= 256 &&
              (_eula || _type == 'custom'),
        _ => true,
      };

  void _next() {
    if (!_canContinue()) {
      if (_step == 2) {
        final port = int.tryParse(_port.text) ?? 0;
        final mem = int.tryParse(_memory.text) ?? 0;
        final msg = _name.text.trim().isEmpty
            ? 'Name the server'
            : port < 1024
                ? 'Port must be 1024-65535'
                : mem < 256
                    ? 'Memory must be at least 256 MiB'
                    : 'You must accept the Minecraft EULA';
        setState(() => _error = msg);
      }
      return;
    }
    setState(() {
      _error = null;
      _loading = false;
    });
    if (_step < 3) {
      setState(() => _step++);
    } else {
      _create();
    }
  }

  Widget _typePicker() {
    if (_types.isEmpty && _error == null) {
      return const Padding(
          padding: EdgeInsets.all(24),
          child: Center(child: CircularProgressIndicator()));
    }
    return Wrap(
      spacing: 10,
      runSpacing: 10,
      children: [
        for (final t in _types)
          ChoiceChip(
            avatar: ServerTypeAvatar(t.id, size: 24),
            label: Text(t.name),
            selected: _type == t.id,
            onSelected: (_) => _pickType(t.id),
          ),
      ],
    );
  }

  Widget _versionPicker() {
    if (_type == null) return const Text('Pick a type first.');
    if (_type == 'custom') {
      return const Text(
          'Custom servers run a jar you drop in — no download needed.');
    }
    final widgets = <Widget>[
      DropdownMenu<String>(
        key: ValueKey('ver-$_type-$_version'),
        label: const Text('Minecraft version'),
        expandedInsets: EdgeInsets.zero,
        initialSelection: _version,
        dropdownMenuEntries: [
          for (final v in _versions)
            DropdownMenuEntry(value: v, label: v),
        ],
        onSelected: (v) => v == null ? null : _pickVersion(v),
      ),
    ];
    if (_loaders.isNotEmpty) {
      widgets.add(const SizedBox(height: 12));
      widgets.add(DropdownMenu<String>(
        key: ValueKey('loader-$_loader'),
        label: const Text('Loader / build'),
        expandedInsets: EdgeInsets.zero,
        initialSelection: _loader,
        dropdownMenuEntries: [
          for (final l in _loaders)
            DropdownMenuEntry(value: l, label: l),
        ],
        onSelected: (v) => setState(() => _loader = v),
      ));
    }
    // The vertical Stepper centers step content — stretch so the dropdowns
    // fill the column instead of floating centered.
    return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch, children: widgets);
  }

  Widget _options() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        TextField(
            controller: _name,
            decoration: const InputDecoration(labelText: 'Server name')),
        const SizedBox(height: 12),
        Row(children: [
          Expanded(
              child: TextField(
                  controller: _port,
                  keyboardType: TextInputType.number,
                  decoration:
                      const InputDecoration(labelText: 'Port'))),
          const SizedBox(width: 12),
          Expanded(
              child: TextField(
                  controller: _memory,
                  keyboardType: TextInputType.number,
                  decoration:
                      const InputDecoration(labelText: 'Memory (MiB)'))),
        ]),
        const SizedBox(height: 12),
        DropdownMenu<String>(
          key: ValueKey('java-$_javaPath'),
          label: Text('Java (needs Java $_requiredJava+)'),
          expandedInsets: EdgeInsets.zero,
          initialSelection: _javaPath.isEmpty ? null : _javaPath,
          dropdownMenuEntries: [
            for (final j in _java)
              DropdownMenuEntry(
                  value: j.path,
                  label: 'Java ${j.major}${j.managed ? ' (managed)' : ''} — ${j.path}'),
            if (_java.isEmpty)
              const DropdownMenuEntry(value: 'java', label: 'java (from PATH)'),
          ],
          onSelected: (v) => setState(() => _javaPath = v ?? 'java'),
        ),
        const SizedBox(height: 12),
        TextField(
            controller: _jvmArgs,
            decoration: const InputDecoration(
                labelText: 'JVM flags (optional)',
                hintText: '-XX:+UseG1GC -XX:+ParallelRefProcEnabled')),
        const SizedBox(height: 12),
        // Custom servers install whatever jar the user drops — no EULA.
        if (_type != 'custom')
          CheckboxListTile(
            value: _eula,
            onChanged: (v) => setState(() => _eula = v ?? false),
            title: const Text('I accept the Minecraft EULA'),
            subtitle: const Text('minecraft.net/eula'),
            controlAffinity: ListTileControlAffinity.leading,
            contentPadding: EdgeInsets.zero,
          ),
      ],
    );
  }

  Widget _review() {
    String loader() =>
        _loader == null || _loader!.isEmpty ? '' : ' · loader $_loader';
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        ListTile(
          leading: ServerTypeAvatar(_type ?? 'custom'),
          title: Text(_name.text),
          subtitle: Text(
              '$_type ${_version ?? ''}${loader()} · port ${_port.text} · ${_memory.text} MiB'),
          contentPadding: EdgeInsets.zero,
        ),
        const SizedBox(height: 8),
        Text(
          'mcst will download the server, write eula.txt and '
          'server.properties, and prepare it for its first start.',
          style: Theme.of(context).textTheme.bodySmall,
        ),
        if (_created) const Text('Created!'),
      ],
    );
  }
}
