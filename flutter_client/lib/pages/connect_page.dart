import 'package:flutter/material.dart';
import '../services/signaling_service.dart';
import '../services/relay_service.dart';
import 'remote_page.dart';

class ConnectPage extends StatefulWidget {
  const ConnectPage({super.key});
  @override
  State<ConnectPage> createState() => _ConnectPageState();
}

class _ConnectPageState extends State<ConnectPage> {
  final _serverCtrl = TextEditingController(text: '192.168.100.129:21118');
  final _peerCtrl = TextEditingController();
  final _signaling = SignalingService();
  final _relay = RelayService();
  List<PeerInfo> _peers = [];
  bool _busy = false;
  bool _wsOk = false;
  String? _err;

  @override
  void initState() {
    super.initState();
    _signaling.peerList.listen((p) => setState(() => _peers = [..._peers.where((x) => !p.any((n) => n.peerId == x.peerId)), ...p]));
    _signaling.connectionResponse.listen((ev) async {
      if (!ev.accepted) { setState(() { _err = 'Rejected'; _busy = false; }); return; }
      try {
        final sid = await _relay.createSession(_serverCtrl.text);
        _signaling.sendRelayInfo(ev.fromPeer, sid, _serverCtrl.text);
        if (mounted) _goRemote(ev.fromPeer, true);
      } catch (e) { setState(() { _err = 'Relay: $e'; _busy = false; }); }
    });
    _signaling.requestConnection.listen((from) => _signaling.acceptConnection(from));
    _signaling.relayInfo.listen((info) async {
      try { await _relay.joinSession(info.relayHost, info.sessionId); } catch (_) {}
    });
    _signaling.errors.listen((e) => setState(() { _err = e; _busy = false; }));
  }

  Future<void> _connectServer() async {
    setState(() { _busy = true; _err = null; });
    try { await _signaling.connect(_serverCtrl.text); setState(() { _wsOk = true; _busy = false; }); }
    catch (e) { setState(() { _err = 'Cannot connect'; _busy = false; }); }
  }

  void _connectPeer(PeerInfo p) { setState(() { _busy = true; _err = null; }); _signaling.sendConnectionRequest(p.peerId); }
  void _direct() {
    final id = _peerCtrl.text.trim(); if (id.isEmpty) return;
    setState(() { _busy = true; _err = null; });
    _signaling.sendConnectionRequest(id);
  }

  void _goRemote(String peerId, bool viewer) {
    final host = _peers.where((p) => p.peerId == peerId).isNotEmpty ? _peers.firstWhere((p) => p.peerId == peerId).hostname : 'Remote';
    Navigator.push(context, MaterialPageRoute(builder: (_) => RemotePage(signaling: _signaling, relay: _relay, peerId: peerId, hostname: host, isViewer: viewer)));
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('rem0te'), centerTitle: true),
      body: ListView(padding: const EdgeInsets.all(16), children: [
        const Icon(Icons.desktop_windows, size: 56, color: Colors.blue),
        const SizedBox(height: 8),
        Text('Remote Desktop', textAlign: TextAlign.center, style: Theme.of(context).textTheme.titleLarge),
        const SizedBox(height: 20),
        // Server card
        Card(child: Padding(padding: const EdgeInsets.all(12), child: Column(children: [
          Row(children: [const Icon(Icons.dns, size: 16), const SizedBox(width: 8), const Text('Signaling', style: TextStyle(fontWeight: FontWeight.bold)), const Spacer(),
            Chip(label: Text(_wsOk ? 'Connected' : 'Offline', style: TextStyle(fontSize: 11, color: _wsOk ? Colors.green : Colors.grey)))],),
          const SizedBox(height: 8),
          Row(children: [
            Expanded(child: TextField(controller: _serverCtrl, decoration: const InputDecoration(hintText: 'host:21118', border: OutlineInputBorder(), contentPadding: EdgeInsets.symmetric(horizontal: 10, vertical: 8)), style: const TextStyle(fontSize: 13))),
            const SizedBox(width: 8),
            ElevatedButton(onPressed: _busy ? null : _connectServer, child: const Text('Connect')),
          ]),
        ]))),
        if (_err != null) Card(color: Colors.red.shade50, child: Padding(padding: const EdgeInsets.all(10), child: Text(_err!, style: const TextStyle(color: Colors.red, fontSize: 13)))),
        // Direct connect
        Card(child: Padding(padding: const EdgeInsets.all(12), child: Column(children: [
          const Row(children: [Icon(Icons.link, size: 16), SizedBox(width: 8), Text('Direct', style: TextStyle(fontWeight: FontWeight.bold))]),
          const SizedBox(height: 8),
          Row(children: [
            Expanded(child: TextField(controller: _peerCtrl, decoration: const InputDecoration(hintText: 'Peer ID', border: OutlineInputBorder(), contentPadding: EdgeInsets.symmetric(horizontal: 10, vertical: 8)), style: const TextStyle(fontSize: 13))),
            const SizedBox(width: 8),
            ElevatedButton(onPressed: _busy ? null : _direct, child: const Text('Connect')),
          ]),
        ]))),
        if (_wsOk) ...[
          const SizedBox(height: 8),
          Card(child: Padding(padding: const EdgeInsets.all(12), child: Column(children: [
            const Row(children: [Icon(Icons.wifi, size: 16, color: Colors.green), SizedBox(width: 8), Text('Online', style: TextStyle(fontWeight: FontWeight.bold))]),
            if (_peers.where((p) => p.online).isEmpty) const Padding(padding: EdgeInsets.all(16), child: Text('No peers online', style: TextStyle(color: Colors.grey)))
            else ..._peers.where((p) => p.online).map((p) => ListTile(leading: const Icon(Icons.circle, size: 8, color: Colors.green), title: Text(p.hostname, style: const TextStyle(fontSize: 14)), subtitle: Text(p.peerId, style: const TextStyle(fontSize: 10, fontFamily: 'monospace')), trailing: Text(p.os, style: const TextStyle(fontSize: 11, color: Colors.grey)), onTap: () => _connectPeer(p))),
          ]))),
        ],
      ]),
    );
  }

  @override void dispose() { _serverCtrl.dispose(); _peerCtrl.dispose(); super.dispose(); }
}
