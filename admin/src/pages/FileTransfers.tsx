import { useState, useEffect } from 'react';
import { useAuth, API_BASE } from '../components/AuthContext';
import { FolderOpen, Clock } from 'lucide-react';

interface FileSession {
  session_id: string;
  peer_a: string;
  peer_b: string | null;
  created_at: string;
}

export default function FileTransfers() {
  const { token } = useAuth();
  const [sessions, setSessions] = useState<FileSession[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchSessions = async () => {
    try {
      const res = await fetch(`${API_BASE}/files/sessions`, {
        headers: { Authorization: `Bearer ${token}` },
      });
      if (res.ok) {
        const data = await res.json();
        setSessions(data.data ?? []);
      }
    } catch (err) {
      console.error('Failed to fetch file sessions:', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchSessions();
    const interval = setInterval(fetchSessions, 10000);
    return () => clearInterval(interval);
  }, [token]);

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-2xl font-bold text-white">File Transfers</h2>
        <p className="text-dark-200 mt-1">Active and recent file transfer sessions</p>
      </div>

      <div className="bg-dark-900 border border-dark-700 rounded-xl overflow-hidden">
        <table className="w-full">
          <thead>
            <tr className="border-b border-dark-700">
              <th className="text-left px-5 py-3 text-xs font-medium text-dark-200 uppercase">Session ID</th>
              <th className="text-left px-5 py-3 text-xs font-medium text-dark-200 uppercase">From (Peer A)</th>
              <th className="text-left px-5 py-3 text-xs font-medium text-dark-200 uppercase">To (Peer B)</th>
              <th className="text-left px-5 py-3 text-xs font-medium text-dark-200 uppercase">Created</th>
              <th className="text-left px-5 py-3 text-xs font-medium text-dark-200 uppercase">Status</th>
            </tr>
          </thead>
          <tbody>
            {loading ? (
              <tr>
                <td colSpan={5} className="px-5 py-8 text-center text-dark-200">Loading...</td>
              </tr>
            ) : sessions.length === 0 ? (
              <tr>
                <td colSpan={5} className="px-5 py-8 text-center">
                  <FolderOpen className="w-10 h-10 text-dark-700 mx-auto mb-2" />
                  <p className="text-dark-200">No file transfer sessions</p>
                </td>
              </tr>
            ) : (
              sessions.map((s) => (
                <tr key={s.session_id} className="border-b border-dark-800 hover:bg-dark-800/50">
                  <td className="px-5 py-3 text-sm font-mono text-primary-500">{s.session_id.slice(0, 8)}...</td>
                  <td className="px-5 py-3 text-sm text-white">{s.peer_a}</td>
                  <td className="px-5 py-3 text-sm text-white">{s.peer_b || 'Waiting...'}</td>
                  <td className="px-5 py-3 text-sm text-dark-200">
                    <span className="flex items-center gap-1">
                      <Clock className="w-3 h-3" />
                      {new Date(s.created_at).toLocaleString()}
                    </span>
                  </td>
                  <td className="px-5 py-3">
                    <span className={`text-xs px-2 py-1 rounded-full ${
                      s.peer_b
                        ? 'bg-green-500/10 text-green-400'
                        : 'bg-yellow-500/10 text-yellow-400'
                    }`}>
                      {s.peer_b ? 'Active' : 'Waiting'}
                    </span>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
