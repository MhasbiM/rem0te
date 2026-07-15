import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';
import 'package:flutter/material.dart';
import '../services/signaling_service.dart';
import '../services/relay_service.dart';

class RemotePage extends StatefulWidget {
  final SignalingService signaling;
  final RelayService relay;
  final String peerId;
  final String hostname;
  final bool isViewer;

  const RemotePage({super.key, required this.signaling, required this.relay, required this.peerId, required this.hostname, required this.isViewer});

  @override
  State<RemotePage> createState() => _RemotePageState();
}

class _RemotePageState extends State<RemotePage> {
  Uint8List? _frame;
  int _fps = 0;
  int _frameCount = 0;
  Timer? _fpsTimer;
  StreamSubscription<Uint8List>? _frameSub;
  StreamSubscription<Map<String, dynamic>>? _inputSub;

  @override
  void initState() {
    super.initState();
    _fpsTimer = Timer.periodic(const Duration(seconds: 1), (_) {
      setState(() { _fps = _frameCount; _frameCount = 0; });
    });

    if (widget.isViewer) {
      // Viewer: receive frames from relay
      _frameSub = widget.relay.frames.listen((frame) {
        setState(() { _frame = frame; _frameCount++; });
      });
    }

    // Input events (both viewer and target)
    _inputSub = widget.signaling.inputEvent.listen((evt) {
      if (!widget.isViewer) {
        // Target: simulate input (call native FFI or local command)
        _simulateInput(evt);
      }
    });
  }

  void _simulateInput(Map<String, dynamic> evt) {
    // On Linux, call xdotool. On macOS/Windows, use native simulation.
    // For now, log the event.
    debugPrint('Input: ${evt['type']} ${evt['key_code'] ?? evt['button']}');
    // Production: call platform channel or FFI
  }

  void _sendInput(String type, {String? keyCode, double? x, double? y, String? button}) {
    final evt = jsonEncode({'type': type, if (keyCode != null) 'key_code': keyCode, if (x != null) 'x': x, if (y != null) 'y': y, if (button != null) 'button': button});
    widget.signaling.sendInputEvent(widget.peerId, evt);
  }

  String _btnName(int buttons) {
    // buttons bitmask: 1=left, 2=right, 4=middle
    if ((buttons & 2) != 0) return 'right';
    if ((buttons & 4) != 0) return 'middle';
    return 'left';
  }

  void _disconnect() {
    widget.signaling.sendSessionEnd(widget.peerId);
    widget.relay.disconnect();
    Navigator.pop(context);
  }

  @override
  Widget build(BuildContext context) {
    return Focus(
      autofocus: true,
      onKeyEvent: (node, event) {
        if (event is KeyDownEvent) {
          _sendInput('keyDown', keyCode: event.logicalKey.keyLabel);
          return KeyEventResult.handled;
        }
        if (event is KeyUpEvent) {
          _sendInput('keyUp', keyCode: event.logicalKey.keyLabel);
          return KeyEventResult.handled;
        }
        return KeyEventResult.ignored;
      },
      child: Scaffold(
      appBar: AppBar(
        title: Text(widget.hostname, style: const TextStyle(fontSize: 16)),
        actions: [
          Center(child: Text('$_fps FPS', style: const TextStyle(fontSize: 12, color: Colors.grey))),
          const SizedBox(width: 12),
          IconButton(icon: const Icon(Icons.fullscreen), onPressed: () {}),
          IconButton(icon: const Icon(Icons.close, color: Colors.red), onPressed: _disconnect),
        ],
      ),
      body: Listener(
        onPointerDown: (e) => _sendInput('mouseDown', button: _btnName(e.buttons)),
        onPointerUp: (e) => _sendInput('mouseUp', button: _btnName(e.buttons)),
        onPointerMove: (e) {
          final sz = context.size;
          if (sz == null) return;
          _sendInput('mouseMove', x: e.localPosition.dx / sz.width * 1920, y: e.localPosition.dy / sz.height * 1080);
        },
        child: Container(
          color: Colors.black,
          child: _frame != null
              ? InteractiveViewer(
                  child: Image.memory(_frame!, fit: BoxFit.contain, gaplessPlayback: true),
                )
              : const Center(child: Column(mainAxisSize: MainAxisSize.min, children: [
                  Icon(Icons.desktop_windows, size: 64, color: Colors.white24),
                  SizedBox(height: 16),
                  Text('Waiting for stream...', style: TextStyle(color: Colors.white54)),
                ])),
        ),
      ),
      ),
    );
  }

  @override
  void dispose() {
    _frameSub?.cancel();
    _inputSub?.cancel();
    _fpsTimer?.cancel();
    super.dispose();
  }
}
