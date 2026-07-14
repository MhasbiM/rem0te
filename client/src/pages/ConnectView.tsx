import { useState, useEffect, useRef, type FormEvent } from 'react';
import {
  Monitor,
  Wifi,
  RefreshCw,
  Circle,
  Server,
  Plug,
  Globe,
  Loader2,
  AlertCircle,
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
const CONNECT_TIMEOUT = 15000;

export default function ConnectView({ onConnected }: Props) {
  const [serverAddr, setServerAddr] = useState(() => localStorage.getItem('rem0te_server') || DEFAULT_SERVER);
  const [peers, setPeers] = useState<PeerInfo[]>([]);
  const [connecting, setConnecting] = useState(false);
  const [connectId, setConnectId] = useState('');
  const [error, setError] = useState('');
  const [wsConnected, setWsConnected] = useState(false);
  const [localPeerId, setLocalPeerId] = useState('');
  const [localHostname, setLocalHostname] = useState('');
  const wsRef = useRef<WebSocket | null>(null);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    setLocalHostname(navigator.platform || 'Unknown');
    return () => {
      wsRef.current?.close();
      if (timeoutRef.current) clearTimeout(timeoutRef.current);
    };
  }, []);

  // Connect to WebSocket signaling server
  const connectToServer = () => {
    setError('');
    wsRef.current?.close();
    const ws = new WebSocket(`ws://${serverAddr}`);
    wsRef.current = ws;

    ws.onopen = () => {
      setWsConnected(true);
      localStorage.setItem('rem0te_server', serverAddr);
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
      try { handleSignalingMessage(JSON.parse(event.data)); }
      catch { console.error('Invalid signaling message'); }
    };

    ws.onerror = () => { setError('Cannot reach signaling server'); setWsConnected(false); };
    ws.onclose = () => setWsConnected(false);
  };

  const clearTimeout = () => {
    if (timeoutRef.current) { clearTimeout(timeoutRef.current); timeoutRef.current = null; }
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
        const p = msg.payload.peer;
        setPeers((prev) => [...prev.filter((x) => x.peer_id !== p.peer_id), { ...p, online: true }]);
        break;
      }
      case 'PeerOffline':
        setPeers((prev) => prev.map((x) => (x.peer_id === msg.payload.peer_id ? { ...x, online: false } : x)));
        break;
      case 'ConnectionResponse':
        clearTimeout();
        if (msg.payload.accepted) {
          const peer = peers.find((p) => p.peer_id === msg.payload.from_peer);
          onConnected({ peerId: msg.payload.from_peer, hostname: peer?.hostname || 'Remote', os: peer?.os || 'Unknown' });
        } else {
          setError('Connection rejected by peer');
          setConnecting(false);
        }
        break;
      case 'RequestConnection':
        // Auto-accept incoming connection, respond back to requestor
        wsRef.current?.send(JSON.stringify({
          type: 'ConnectionResponse',
          payload: {
            from_peer: localPeerId,
            to_peer: msg.payload.from_peer,
            accepted: true,
            sdp: null,
          },
        }));
        break;
      case 'Error':
        clearTimeout();
        setConnecting(false);
        setError(msg.payload?.message || 'Unknown error');
        break;
    }
  };

  const handleConnect = (peerId: string, hostname: string, os: string) => {
    if (!wsRef.current || wsRef.current.readyState !== WebSocket.OPEN) {
      setError('Not connected to signaling server. Click "Connect" first.');
      return;
    }

    setConnecting(true);
    setError('');
    timeoutRef.current = setTimeout(() => {
      setConnecting(false);
      setError('Connection timed out. Is rem0te running on the remote machine?');
    }, CONNECT_TIMEOUT);

    wsRef.current.send(JSON.stringify({
      type: 'RequestConnection',
      payload: {
        from_peer: localPeerId,
        to_peer: peerId,
        sdp: null,
      },
    }));
  };

  const handleDirectConnect = async (e: FormEvent) => {
    e.preventDefault();
    if (!connectId.trim()) return;

    const peer = peers.find((p) => p.peer_id === connectId.trim());
    if (peer && wsConnected && wsRef.current) {
      setConnecting(true);
      setError('');
      timeoutRef.current = setTimeout(() => {
        setConnecting(false);
        setError('No response. Ensure rem0te is running on the remote machine.');
      }, CONNECT_TIMEOUT);
      wsRef.current.send(JSON.stringify({
        type: 'RequestConnection',
        payload: { from_peer: localPeerId, to_peer: peer.peer_id, sdp: null },
      }));
      return;
    }
    handleConnect(connectId.trim(), connectId.trim(), 'Unknown');
  };

  return (
    <div className="max-w-2xl mx-auto p-8 space-y-8">
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
              wsConnected ? 'bg-green-500/10 text-green-400' : 'bg-dark-700 text-dark-200'
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
          <div className="bg-red-500/10 border border-red-500/30 text-red-400 px-4 py-2 rounded-lg text-sm flex items-center gap-2">
            <AlertCircle className="w-4 h-4 flex-shrink-0" />
            {error}
          </div>
        )}
      </div>

      {/* Direct Connect */}
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
            placeholder="Enter Peer ID or IP:port..."
            className="flex-1 bg-dark-800 border border-dark-700 rounded-lg px-4 py-2.5 text-white placeholder-dark-200 focus:outline-none focus:border-primary-500"
            disabled={connecting}
          />
          <button
            type="submit"
            disabled={connecting || !connectId.trim()}
            className="flex items-center gap-2 px-4 py-2.5 bg-primary-600 hover:bg-primary-700 disabled:opacity-50 rounded-lg text-sm font-medium text-white transition-colors"
          >
            {connecting ? (
              <>
                <Loader2 className="w-4 h-4 animate-spin" />
                Connecting...
              </>
            ) : (
              'Connect'
            )}
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
              onClick={() => wsRef.current?.send(JSON.stringify({ type: 'Refresh' }))}
              className="p-1.5 text-dark-200 hover:text-white rounded-lg hover:bg-dark-800"
            >
              <RefreshCw className="w-4 h-4" />
            </button>
          </div>
          <div className="p-4 space-y-2 max-h-64 overflow-y-auto">
            {peers.filter((p) => p.online).length === 0 ? (
              <p className="text-dark-200 text-sm text-center py-4">Waiting for peers...</p>
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
