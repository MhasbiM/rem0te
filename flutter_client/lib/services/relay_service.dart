import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

class RelayService {
  Socket? _socket;
  final _frameController = StreamController<Uint8List>.broadcast();
  bool _connected = false;

  Stream<Uint8List> get frames => _frameController.stream;
  bool get isConnected => _connected;

  /// Create a relay session (called by viewer)
  Future<String> createSession(String serverAddr) async {
    // Use relay port 21117, not WS port
    final host = serverAddr.split(':').first;
    final port = 21117;
    _socket = await Socket.connect(host, port, timeout: const Duration(seconds: 5));
    _connected = true;

    // Generate 36-char session ID (like UUID)
    final now = DateTime.now().microsecondsSinceEpoch.toRadixString(36);
    final rand = (DateTime.now().millisecond * 9999).toRadixString(36);
    final sessionId = '${now.padLeft(18, '0')}-${rand.padLeft(17, '0')}';
    _socket!.add(utf8.encode(sessionId));
    _socket!.add([0]); // role 0 = initiator (viewer)

    _startReading();
    return sessionId;
  }

  /// Join an existing relay session (called by target)
  Future<void> joinSession(String relayHost, String sessionId) async {
    final parts = relayHost.split(':');
    final host = parts[0];
    final port = parts.length > 1 ? int.tryParse(parts[1]) ?? 21117 : 21117;

    _socket = await Socket.connect(host, port, timeout: const Duration(seconds: 5));
    _connected = true;

    // Pad to exactly 36 bytes with trailing zeros
    final sid = sessionId.padRight(36, '0').substring(0, 36);
    _socket!.add(utf8.encode(sid));
    _socket!.add([1]); // role 1 = joiner (target)

    _startReading();
  }

  void _startReading() {
    _socket?.listen(
      (data) {
        // Relay forwards raw bytes, parse our framing
        _buffer.addAll(data);
        _parseFrames();
      },
      onError: (e) {
        _connected = false;
        _frameController.addError(e);
      },
      onDone: () {
        _connected = false;
      },
    );
  }

  final _buffer = <int>[];

  void _parseFrames() {
    while (_buffer.length >= 4) {
      // Read 4-byte total_len
      final totalLen = (_buffer[0] << 24) | (_buffer[1] << 16) | (_buffer[2] << 8) | _buffer[3];
      if (totalLen <= 0 || totalLen > 10000000) {
        _buffer.clear();
        break;
      }
      if (_buffer.length < 4 + totalLen) break;

      // Remove header
      _buffer.removeRange(0, 4);

      final frame = Uint8List.fromList(_buffer.take(totalLen).toList());
      _buffer.removeRange(0, totalLen);

      if (frame.length >= 9 && frame[0] == 0) {
        // MSG_FRAME (type 0): [type=0][4-byte payload_len][payload]
        final plen = (frame[1] << 24) | (frame[2] << 16) | (frame[3] << 8) | frame[4];
        if (plen > 0 && plen <= frame.length - 5) {
          final payload = frame.sublist(5, 5 + plen);
          _frameController.add(payload);
        }
      }
    }
  }

  void sendFrame(Uint8List jpegData) {
    if (_socket == null) return;
    final totalLen = 1 + 4 + jpegData.length;
    final header = ByteData(9)
      ..setUint32(0, totalLen, Endian.big)
      ..setUint8(4, 0) // MSG_FRAME
      ..setUint32(5, jpegData.length, Endian.big);
    _socket!.add(header.buffer.asUint8List());
    _socket!.add(jpegData);
  }

  void sendInput(Uint8List jsonData) {
    if (_socket == null) return;
    final totalLen = 1 + 4 + jsonData.length;
    final header = ByteData(9)
      ..setUint32(0, totalLen, Endian.big)
      ..setUint8(4, 1) // MSG_INPUT
      ..setUint32(5, jsonData.length, Endian.big);
    _socket!.add(header.buffer.asUint8List());
    _socket!.add(jsonData);
  }

  void disconnect() {
    _socket?.destroy();
    _socket = null;
    _connected = false;
  }

  void dispose() {
    disconnect();
    _frameController.close();
  }
}
