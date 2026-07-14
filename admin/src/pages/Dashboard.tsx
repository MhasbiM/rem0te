import { useState, useEffect } from 'react';
import { useAuth, API_BASE } from '../components/AuthContext';
import { Monitor, Users, Activity, FolderOpen, Circle, Wifi, WifiOff } from 'lucide-react';

interface Stats {
  totalPeers: number;
  onlinePeers: number;
  totalUsers: number;
  activeSessions: number;
}

interface Peer {
  id: string;
  peer_id: string;
  os: string;
  hostname: string;
  online: boolean;
  addr?: string;
}

export default function Dashboard() {
  const { token } = useAuth();
  const [stats, setStats] = useState<Stats>({ totalPeers: 0, onlinePeers: 0, totalUsers: 0, activeSessions: 0 });
  const [peers, setPeers] = useState<Peer[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const fetchData = async () => {
      try {
        const headers = { Authorization: `Bearer ${token}` };
        const [connRes, usersRes, sessionsRes] = await Promise.all([
          fetch(`${API_BASE}/connections`, { headers }),
          fetch(`${API_BASE}/users`, { headers }),
          fetch(`${API_BASE}/connections/relay-sessions`, { headers }),
        ]);

        const connData = connRes.ok ? await connRes.json() : null;
        const usersData = usersRes.ok ? await usersRes.json() : null;
        const sessionsData = sessionsRes.ok ? await sessionsRes.json() : null;

        const peerList: Peer[] = connData?.data ?? [];
        const online = peerList.filter((p) => p.online).length;

        setPeers(peerList.slice(0, 10));
        setStats({
          totalPeers: peerList.length,
          onlinePeers: online,
          totalUsers: usersData?.data?.length ?? 0,
          activeSessions: sessionsData?.data?.length ?? 0,
        });
      } catch (err) {
        console.error('Failed to fetch dashboard data:', err);
      } finally {
        setLoading(false);
      }
    };
    fetchData();
    const interval = setInterval(fetchData, 10000);
    return () => clearInterval(interval);
  }, [token]);

  const statCards = [
    { label: 'Total Peers', value: stats.totalPeers, icon: Monitor, color: 'text-blue-400', bg: 'bg-blue-500/10' },
    { label: 'Online Now', value: stats.onlinePeers, icon: Wifi, color: 'text-green-400', bg: 'bg-green-500/10' },
    { label: 'Users', value: stats.totalUsers, icon: Users, color: 'text-purple-400', bg: 'bg-purple-500/10' },
    { label: 'Active Sessions', value: stats.activeSessions, icon: Activity, color: 'text-orange-400', bg: 'bg-orange-500/10' },
  ];

  return (
    <div className="space-y-8">
      <div>
        <h2 className="text-2xl font-bold text-white">Dashboard</h2>
        <p className="text-dark-200 mt-1">Overview of your rem0te infrastructure</p>
      </div>

      {/* Stats Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        {statCards.map(({ label, value, icon: Icon, color, bg }) => (
          <div key={label} className="bg-dark-900 border border-dark-700 rounded-xl p-5">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-dark-200 text-sm">{label}</p>
                <p className="text-2xl font-bold text-white mt-1">
                  {loading ? '...' : value}
                </p>
              </div>
              <div className={`p-3 rounded-lg ${bg}`}>
                <Icon className={`w-5 h-5 ${color}`} />
              </div>
            </div>
          </div>
        ))}
      </div>

      {/* Recent Peers */}
      <div className="bg-dark-900 border border-dark-700 rounded-xl">
        <div className="px-5 py-4 border-b border-dark-700">
          <h3 className="font-semibold text-white">Connected Peers</h3>
        </div>
        <div className="p-5">
          {loading ? (
            <p className="text-dark-200 text-sm">Loading...</p>
          ) : peers.length === 0 ? (
            <div className="text-center py-8">
              <Monitor className="w-12 h-12 text-dark-700 mx-auto mb-3" />
              <p className="text-dark-200">No peers connected yet</p>
              <p className="text-dark-200 text-sm mt-1">
                Run the rem0te client on a remote machine to see it here
              </p>
            </div>
          ) : (
            <div className="space-y-2">
              {peers.map((peer) => (
                <div
                  key={peer.id}
                  className="flex items-center justify-between p-3 bg-dark-800 rounded-lg"
                >
                  <div className="flex items-center gap-3">
                    {peer.online ? (
                      <Circle className="w-2.5 h-2.5 fill-green-400 text-green-400" />
                    ) : (
                      <Circle className="w-2.5 h-2.5 fill-dark-200 text-dark-200" />
                    )}
                    <div>
                      <p className="text-sm font-medium text-white">{peer.hostname}</p>
                      <p className="text-xs text-dark-200">
                        {peer.peer_id} · {peer.os}
                      </p>
                    </div>
                  </div>
                  <span
                    className={`text-xs px-2 py-1 rounded-full ${
                      peer.online
                        ? 'bg-green-500/10 text-green-400'
                        : 'bg-dark-700 text-dark-200'
                    }`}
                  >
                    {peer.online ? 'Online' : 'Offline'}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
