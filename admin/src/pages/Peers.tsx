import { useState, useEffect } from 'react';
import { useAuth, API_BASE } from '../components/AuthContext';
import { Circle, Monitor, RefreshCw, Search } from 'lucide-react';

interface Peer {
  id: string;
  peer_id: string;
  os: string;
  hostname: string;
  online: boolean;
  addr?: string;
}

export default function Peers() {
  const { token } = useAuth();
  const [peers, setPeers] = useState<Peer[]>([]);
  const [search, setSearch] = useState('');
  const [loading, setLoading] = useState(true);

  const fetchPeers = async () => {
    try {
      const res = await fetch(`${API_BASE}/connections`, {
        headers: { Authorization: `Bearer ${token}` },
      });
      if (res.ok) {
        const data = await res.json();
        setPeers(data.data ?? []);
      }
    } catch (err) {
      console.error('Failed to fetch peers:', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchPeers();
    const interval = setInterval(fetchPeers, 10000);
    return () => clearInterval(interval);
  }, [token]);

  const filtered = peers.filter(
    (p) =>
      p.hostname.toLowerCase().includes(search.toLowerCase()) ||
      p.peer_id.toLowerCase().includes(search.toLowerCase()) ||
      p.os.toLowerCase().includes(search.toLowerCase())
  );

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold text-white">Peers</h2>
          <p className="text-dark-200 mt-1">{peers.length} registered peers</p>
        </div>
        <button
          onClick={fetchPeers}
          className="flex items-center gap-2 px-4 py-2 bg-dark-800 hover:bg-dark-700 border border-dark-700 rounded-lg text-sm text-white transition-colors"
        >
          <RefreshCw className="w-4 h-4" />
          Refresh
        </button>
      </div>

      {/* Search */}
      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-dark-200" />
        <input
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search peers..."
          className="w-full bg-dark-900 border border-dark-700 rounded-lg pl-10 pr-4 py-2.5 text-white placeholder-dark-200 focus:outline-none focus:border-primary-500 transition-colors"
        />
      </div>

      {/* Table */}
      <div className="bg-dark-900 border border-dark-700 rounded-xl overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="border-b border-dark-700">
                <th className="text-left px-5 py-3 text-xs font-medium text-dark-200 uppercase">Status</th>
                <th className="text-left px-5 py-3 text-xs font-medium text-dark-200 uppercase">Hostname</th>
                <th className="text-left px-5 py-3 text-xs font-medium text-dark-200 uppercase">Peer ID</th>
                <th className="text-left px-5 py-3 text-xs font-medium text-dark-200 uppercase">OS</th>
                <th className="text-left px-5 py-3 text-xs font-medium text-dark-200 uppercase">Address</th>
              </tr>
            </thead>
            <tbody>
              {loading ? (
                <tr>
                  <td colSpan={5} className="px-5 py-8 text-center text-dark-200">
                    Loading...
                  </td>
                </tr>
              ) : filtered.length === 0 ? (
                <tr>
                  <td colSpan={5} className="px-5 py-8 text-center">
                    <Monitor className="w-10 h-10 text-dark-700 mx-auto mb-2" />
                    <p className="text-dark-200">No peers found</p>
                  </td>
                </tr>
              ) : (
                filtered.map((peer) => (
                  <tr key={peer.id} className="border-b border-dark-800 hover:bg-dark-800/50 transition-colors">
                    <td className="px-5 py-3">
                      <Circle
                        className={`w-2.5 h-2.5 ${
                          peer.online ? 'fill-green-400 text-green-400' : 'fill-dark-200 text-dark-200'
                        }`}
                      />
                    </td>
                    <td className="px-5 py-3 text-sm font-medium text-white">{peer.hostname}</td>
                    <td className="px-5 py-3 text-sm text-dark-200 font-mono">{peer.peer_id}</td>
                    <td className="px-5 py-3 text-sm text-dark-200">{peer.os}</td>
                    <td className="px-5 py-3 text-sm text-dark-200 font-mono">{peer.addr || '-'}</td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
