import { useState, useEffect, type FormEvent } from 'react';
import {
  Monitor,
  Wifi,
  Search,
  RefreshCw,
  Circle,
  Server,
  Plug,
  Globe,
} from 'lucide-react';

interface PeerInfo {
  peer_id: string;
  os: string;
  hostname: string;
  online: boolean;
}

interface ConnectionInfo {
  peerId: string;
  hostname: string;
  os: string;
}

interface Props {
  onConnected: (info: ConnectionInfo) => void;
}

const DEFAULT_SERVER = 'localhost:21118';

export default function ConnectView({ onConnected }: Props) {
  const [serverAddr, setServerAddr] = useState(() => localStorage.getItem('rem0te_server') || DEFAULT_SERVER);
  const [peers, setPeers] = useState<PeerInfo[]>([]);
  const [connecting, setConnecting] = useState(false);
  const [connectId, setConnectId] = useState('');
  const [error, setError] = useState('');
  const [wsConnected, setWsConnected] = useState(false);
  const [localPeerId, setLocalPeerId] = useState('');
  const [localHostname, setLocalHostname] = useState('');

  useEffect(() => {
    // Auto-detect local hostname
    setLocalHostname(navigator.platform || 'Unknown');
  }, []);

  // Connect to WebSocket signaling server
  const connectToServer = () => {
    setError('');
    const ws = new WebSocket(`ws://${serverAddr}`);

    ws.onopen = () => {
      setWsConnected(true);
      localStorage.setItem('rem0te_server', serverAddr);

      // Register
      ws.send(JSON.stringify({
        type: 'Register',
        payload: {
          peer_id: localPeerId || `peer-${Math.random().toString(36).slice(2, 10)}`,
          os: navigator.platform,
          hostname: localHostname || 'Unknown',
        },
      }));
    };

    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data);
        handleSignalingMessage(msg);
      } catch (e) {
        console.error('Invalid message:', e);
      }
    };

    ws.onerror = () => {
      setError('Failed to connect to signaling server');
      setWsConnected(false);
    };

    ws.onclose = () => {
      setWsConnected(false);
    };

    // Store for later use
    (window as any).__rem0te_ws = ws;
  };

  const handleSignalingMessage = (msg: any) => {
    switch (msg.type) {
      case 'Registered':
        setLocalPeerId(msg.payload.assigned_id);
        break;
      case 'PeerList':
        setPeers(msg.payload.peers || []);
        break;
      case 'PeerOnline': {
        const peer = msg.payload.peer;
        setPeers((prev) => {
          const filtered = prev.filter((p) => p.peer_id !== peer.peer_id);
          return [...filtered, { ...peer, online: true }];
        });
        break;
      }
      case 'PeerOffline':
        setPeers((prev) =>
          prev.map((p) => (p.peer_id === msg.payload.peer_id ? { ...p, online: false } : p))
        );
        break;
      case 'ConnectionResponse':
        if (msg.payload.accepted) {
          onConnected({
            peerId: msg.payload.from_peer,
            hostname: peers.find((p) => p.peer_id === msg.payload.from_peer)?.hostname || 'Remote',
            os: peers.find((p) => p.peer_id === msg.payload.from_peer)?.os || 'Unknown',
          });
        } else {
          setError('Connection rejected by peer');
        }
        setConnecting(false);
        break;
    }
  };

  const handleConnect = (peerId: string, hostname: string, os: string) => {
    const ws = (window as any).__rem0te_ws;
    if (!ws || !wsConnected) {
      setError('Not connected to server');
      return;
    }

    setConnecting(true);
    setError('');
    ws.send(JSON.stringify({
      type: 'RequestConnection',
      payload: {
        from_peer: localPeerId,
        to_peer: peerId,
        sdp: null,
      },
    }));
  };

  const handleDirectConnect = (e: FormEvent) => {
    e.preventDefault();
    if (!connectId.trim()) return;
    
    // Find the peer by ID
    const peer = peers.find((p) => p.peer_id === connectId.trim());
    if (peer) {
      handleConnect(peer.peer_id, peer.hostname, peer.os);
    } else {
      // Try direct connection
      onConnected({
        peerId: connectId.trim(),
        hostname: 'Direct Connection',
        os: 'Unknown',
      });
    }
  };

  return (
    <div className="max-w-2xl mx-auto p-8 space-y-8">
      {/* Header */}
      <div className="text-center">
        <div className="inline-flex items-center justify-center w-16 h-16 bg-primary-600 rounded-2xl mb-4">
          <Monitor className="w-8 h-8 text-white" />
        </div>
        <h2 className="text-2xl font-bold text-white">rem0te Desktop</h2>
        <p className="text-dark-200 mt-2">Connect to remote machines securely</p>
      </div>

      {/* Server connection */}
      <div className="bg-dark-900 border border-dark-700 rounded-xl p-5 space-y-4">
        <div className="flex items-center gap-2">
          <Server className="w-4 h-4 text-dark-200" />
          <h3 className="font-semibold text-white">Signaling Server</h3>
          <span
            className={`ml-auto text-xs px-2 py-1 rounded-full ${
              wsConnected
                ? 'bg-green-500/10 text-green-400'
                : 'bg-dark-700 text-dark-200'
            }`}
          >
            {wsConnected ? 'Connected' : 'Disconnected'}
          </span>
        </div>
        <div className="flex gap-2">
          <input
            type="text"
            value={serverAddr}
            onChange={(e) => setServerAddr(e.target.value)}
            placeholder="server:21118"
            className="flex-1 bg-dark-800 border border-dark-700 rounded-lg px-4 py-2.5 text-white placeholder-dark-200 focus:outline-none focus:border-primary-500 font-mono text-sm"
          />
          <button
            onClick={connectToServer}
            className="flex items-center gap-2 px-4 py-2.5 bg-primary-600 hover:bg-primary-700 rounded-lg text-sm font-medium text-white transition-colors"
          >
            <Plug className="w-4 h-4" />
            Connect
          </button>
        </div>
        {error && (
          <div className="bg-red-500/10 border border-red-500/30 text-red-400 px-4 py-2 rounded-lg text-sm">
            {error}
          </div>
        )}
      </div>

      {/* Direct connect */}
      <div className="bg-dark-900 border border-dark-700 rounded-xl p-5 space-y-4">
        <div className="flex items-center gap-2">
          <Globe className="w-4 h-4 text-dark-200" />
          <h3 className="font-semibold text-white">Direct Connect</h3>
        </div>
        <form onSubmit={handleDirectConnect} className="flex gap-2">
          <input
            type="text"
            value={connectId}
            onChange={(e) => setConnectId(e.target.value)}
            placeholder="Enter Peer ID or address..."
            className="flex-1 bg-dark-800 border border-dark-700 rounded-lg px-4 py-2.5 text-white placeholder-dark-200 focus:outline-none focus:border-primary-500"
          />
          <button
            type="submit"
            disabled={connecting || !connectId.trim()}
            className="flex items-center gap-2 px-4 py-2.5 bg-primary-600 hover:bg-primary-700 disabled:opacity-50 rounded-lg text-sm font-medium text-white transition-colors"
          >
            {connecting ? 'Connecting...' : 'Connect'}
          </button>
        </form>
      </div>

      {/* Online peers */}
      {wsConnected && (
        <div className="bg-dark-900 border border-dark-700 rounded-xl">
          <div className="px-5 py-4 border-b border-dark-700 flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Wifi className="w-4 h-4 text-green-400" />
              <h3 className="font-semibold text-white">Online Peers</h3>
            </div>
            <button
              onClick={() => {
                const ws = (window as any).__rem0te_ws;
                if (ws) ws.send(JSON.stringify({ type: 'Refresh' }));
              }}
              className="p-1.5 text-dark-200 hover:text-white rounded-lg hover:bg-dark-800"
            >
              <RefreshCw className="w-4 h-4" />
            </button>
          </div>
          <div className="p-4 space-y-2 max-h-64 overflow-y-auto">
            {peers.length === 0 ? (
              <p className="text-dark-200 text-sm text-center py-4">
                Waiting for peers to come online...
              </p>
            ) : (
              peers
                .filter((p) => p.online)
                .map((peer) => (
                  <div
                    key={peer.peer_id}
                    className="flex items-center justify-between p-3 bg-dark-800 rounded-lg hover:bg-dark-700/50 transition-colors cursor-pointer"
                    onClick={() => handleConnect(peer.peer_id, peer.hostname, peer.os)}
                  >
                    <div className="flex items-center gap-3">
                      <Circle className="w-2.5 h-2.5 fill-green-400 text-green-400" />
                      <div>
                        <p className="text-sm font-medium text-white">{peer.hostname}</p>
                        <p className="text-xs text-dark-200 font-mono">{peer.peer_id}</p>
                      </div>
                    </div>
                    <span className="text-xs text-dark-200">{peer.os}</span>
                  </div>
                ))
            )}
          </div>
        </div>
      )}
    </div>
  );
}
