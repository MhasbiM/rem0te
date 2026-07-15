import 'dart:async';
import 'dart:convert';
import 'package:web_socket_channel/web_socket_channel.dart';

class SignalingService {
  WebSocketChannel? _channel;
  final _peerListController = StreamController<List<PeerInfo>>.broadcast();
  final _connectionResponseController = StreamController<ConnectionResponseEvent>.broadcast();
  final _requestConnectionController = StreamController<String>.broadcast();
  final _relayInfoController = StreamController<RelayInfoEvent>.broadcast();
  final _sessionEndController = StreamController<void>.broadcast();
  final _inputEventController = StreamController<Map<String, dynamic>>.broadcast();
  final _errorController = StreamController<String>.broadcast();

  String _peerId = '';
  bool _connected = false;

  Stream<List<PeerInfo>> get peerList => _peerListController.stream;
  Stream<ConnectionResponseEvent> get connectionResponse => _connectionResponseController.stream;
  Stream<String> get requestConnection => _requestConnectionController.stream;
  Stream<RelayInfoEvent> get relayInfo => _relayInfoController.stream;
  Stream<void> get sessionEnd => _sessionEndController.stream;
  Stream<Map<String, dynamic>> get inputEvent => _inputEventController.stream;
  Stream<String> get errors => _errorController.stream;
  bool get isConnected => _connected;
  String get peerId => _peerId;

  Future<void> connect(String serverAddr) async {
    final uri = Uri.parse('ws://$serverAddr');
    _channel = WebSocketChannel.connect(uri);
    _connected = true;

    _channel!.stream.listen(
      (data) {
        try {
          final msg = jsonDecode(data as String);
          _handleMessage(msg);
        } catch (_) {}
      },
      onError: (e) {
        _connected = false;
        _errorController.add('Connection error: $e');
      },
      onDone: () {
        _connected = false;
      },
    );

    // Wait a bit, then register
    await Future.delayed(const Duration(milliseconds: 100));
    _peerId = 'peer-${DateTime.now().millisecondsSinceEpoch.toString().substring(5)}';
    _send({
      'type': 'Register',
      'payload': {
        'peer_id': _peerId,
        'os': 'flutter',
        'hostname': 'Flutter Client',
      },
    });
  }

  void sendConnectionRequest(String targetPeerId) {
    _send({
      'type': 'RequestConnection',
      'payload': {
        'from_peer': _peerId,
        'to_peer': targetPeerId,
        'sdp': null,
      },
    });
  }

  void acceptConnection(String fromPeer) {
    _send({
      'type': 'ConnectionResponse',
      'payload': {
        'from_peer': _peerId,
        'to_peer': fromPeer,
        'accepted': true,
        'sdp': null,
      },
    });
  }

  void sendRelayInfo(String toPeer, String sessionId, String serverAddr) {
    _send({
      'type': 'RelayInfo',
      'payload': {
        'relay_host': serverAddr,
        'relay_port': 21117,
        'session_id': sessionId,
        'to_peer': toPeer,
      },
    });
  }

  void sendInputEvent(String toPeer, String eventJson) {
    _send({
      'type': 'InputEvent',
      'payload': {
        'from_peer': _peerId,
        'to_peer': toPeer,
        'event': eventJson,
      },
    });
  }

  void sendSessionEnd(String toPeer) {
    _send({
      'type': 'SessionEnd',
      'payload': {
        'from_peer': _peerId,
        'to_peer': toPeer,
      },
    });
  }

  void _send(Map<String, dynamic> msg) {
    _channel?.sink.add(jsonEncode(msg));
  }

  void _handleMessage(Map<String, dynamic> msg) {
    final type = msg['type'] as String?;
    final payload = msg['payload'] as Map<String, dynamic>?;

    switch (type) {
      case 'Registered':
        break;
      case 'PeerList':
        final peers = (payload?['peers'] as List?)
            ?.map((p) => PeerInfo.fromJson(p as Map<String, dynamic>))
            .toList() ?? [];
        _peerListController.add(peers);
        break;
      case 'PeerOnline':
        if (payload != null) {
          _peerListController.add([PeerInfo.fromJson(payload['peer'] as Map<String, dynamic>)]);
        }
        break;
      case 'PeerOffline':
        break;
      case 'ConnectionResponse':
        _connectionResponseController.add(ConnectionResponseEvent(
          fromPeer: payload?['from_peer'] ?? '',
          accepted: payload?['accepted'] == true,
        ));
        break;
      case 'RequestConnection':
        _requestConnectionController.add(payload?['from_peer'] ?? '');
        break;
      case 'RelayInfo':
        _relayInfoController.add(RelayInfoEvent(
          sessionId: payload?['session_id'] ?? '',
          relayHost: payload?['relay_host'] ?? '',
          relayPort: payload?['relay_port'] ?? 21117,
        ));
        break;
      case 'SessionEnd':
        _sessionEndController.add(null);
        break;
      case 'InputEvent':
        if (payload?['event'] is String) {
          try {
            final evt = jsonDecode(payload!['event'] as String);
            _inputEventController.add(evt as Map<String, dynamic>);
          } catch (_) {}
        }
        break;
      case 'Error':
        _errorController.add(payload?['message'] ?? 'Unknown error');
        break;
    }
  }

  void disconnect() {
    _channel?.sink.close();
    _connected = false;
  }

  void dispose() {
    disconnect();
    _peerListController.close();
    _connectionResponseController.close();
    _requestConnectionController.close();
    _relayInfoController.close();
    _sessionEndController.close();
    _inputEventController.close();
    _errorController.close();
  }
}

class PeerInfo {
  final String peerId;
  final String os;
  final String hostname;
  final bool online;

  PeerInfo({required this.peerId, required this.os, required this.hostname, required this.online});

  factory PeerInfo.fromJson(Map<String, dynamic> json) => PeerInfo(
    peerId: json['peer_id'] ?? '',
    os: json['os'] ?? '',
    hostname: json['hostname'] ?? '',
    online: json['online'] == true,
  );
}

class ConnectionResponseEvent {
  final String fromPeer;
  final bool accepted;
  ConnectionResponseEvent({required this.fromPeer, required this.accepted});
}

class RelayInfoEvent {
  final String sessionId;
  final String relayHost;
  final int relayPort;
  RelayInfoEvent({required this.sessionId, required this.relayHost, required this.relayPort});
}
