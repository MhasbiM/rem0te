import { useState, useEffect, type FormEvent } from 'react';
import { useAuth, API_BASE } from '../components/AuthContext';
import { Users as UsersIcon, Plus, Trash2, Shield, User } from 'lucide-react';

interface UserRecord {
  id: string;
  username: string;
  role: string;
  created_at: string;
}

export default function Users() {
  const { token, user: currentUser } = useAuth();
  const [users, setUsers] = useState<UserRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [showForm, setShowForm] = useState(false);
  const [newUsername, setNewUsername] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [newRole, setNewRole] = useState('user');
  const [error, setError] = useState('');
  const [creating, setCreating] = useState(false);

  const fetchUsers = async () => {
    try {
      const res = await fetch(`${API_BASE}/users`, {
        headers: { Authorization: `Bearer ${token}` },
      });
      if (res.ok) {
        const data = await res.json();
        setUsers(data.data ?? []);
      }
    } catch (err) {
      console.error('Failed to fetch users:', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchUsers();
  }, [token]);

  const handleCreate = async (e: FormEvent) => {
    e.preventDefault();
    setError('');
    setCreating(true);
    try {
      const res = await fetch(`${API_BASE}/users`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({ username: newUsername, password: newPassword, role: newRole }),
      });
      if (!res.ok) {
        const data = await res.json();
        throw new Error(data.error || 'Failed to create user');
      }
      setNewUsername('');
      setNewPassword('');
      setShowForm(false);
      fetchUsers();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create');
    } finally {
      setCreating(false);
    }
  };

  const handleDelete = async (username: string) => {
    if (!confirm(`Delete user "${username}"?`)) return;
    try {
      await fetch(`${API_BASE}/users/${username}`, {
        method: 'DELETE',
        headers: { Authorization: `Bearer ${token}` },
      });
      fetchUsers();
    } catch (err) {
      console.error('Failed to delete user:', err);
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold text-white">Users</h2>
          <p className="text-dark-200 mt-1">{users.length} registered users</p>
        </div>
        {currentUser?.role === 'admin' && (
          <button
            onClick={() => setShowForm(!showForm)}
            className="flex items-center gap-2 px-4 py-2 bg-primary-600 hover:bg-primary-700 rounded-lg text-sm font-medium text-white transition-colors"
          >
            <Plus className="w-4 h-4" />
            Add User
          </button>
        )}
      </div>

      {/* Create form */}
      {showForm && (
        <form onSubmit={handleCreate} className="bg-dark-900 border border-dark-700 rounded-xl p-5 space-y-4">
          <h3 className="font-semibold text-white">Create New User</h3>
          {error && (
            <div className="bg-red-500/10 border border-red-500/30 text-red-400 px-4 py-2 rounded-lg text-sm">
              {error}
            </div>
          )}
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <input
              type="text"
              value={newUsername}
              onChange={(e) => setNewUsername(e.target.value)}
              placeholder="Username"
              className="bg-dark-800 border border-dark-700 rounded-lg px-4 py-2.5 text-white placeholder-dark-200 focus:outline-none focus:border-primary-500"
              required
            />
            <input
              type="password"
              value={newPassword}
              onChange={(e) => setNewPassword(e.target.value)}
              placeholder="Password"
              className="bg-dark-800 border border-dark-700 rounded-lg px-4 py-2.5 text-white placeholder-dark-200 focus:outline-none focus:border-primary-500"
              required
            />
            <select
              value={newRole}
              onChange={(e) => setNewRole(e.target.value)}
              className="bg-dark-800 border border-dark-700 rounded-lg px-4 py-2.5 text-white focus:outline-none focus:border-primary-500"
            >
              <option value="user">User</option>
              <option value="admin">Admin</option>
            </select>
          </div>
          <div className="flex gap-2">
            <button
              type="submit"
              disabled={creating}
              className="px-4 py-2 bg-primary-600 hover:bg-primary-700 disabled:opacity-50 rounded-lg text-sm font-medium text-white transition-colors"
            >
              {creating ? 'Creating...' : 'Create'}
            </button>
            <button
              type="button"
              onClick={() => setShowForm(false)}
              className="px-4 py-2 bg-dark-800 hover:bg-dark-700 rounded-lg text-sm text-dark-200 transition-colors"
            >
              Cancel
            </button>
          </div>
        </form>
      )}

      {/* Users table */}
      <div className="bg-dark-900 border border-dark-700 rounded-xl overflow-hidden">
        <table className="w-full">
          <thead>
            <tr className="border-b border-dark-700">
              <th className="text-left px-5 py-3 text-xs font-medium text-dark-200 uppercase">User</th>
              <th className="text-left px-5 py-3 text-xs font-medium text-dark-200 uppercase">Role</th>
              <th className="text-left px-5 py-3 text-xs font-medium text-dark-200 uppercase">Created</th>
              <th className="text-right px-5 py-3 text-xs font-medium text-dark-200 uppercase">Actions</th>
            </tr>
          </thead>
          <tbody>
            {loading ? (
              <tr>
                <td colSpan={4} className="px-5 py-8 text-center text-dark-200">Loading...</td>
              </tr>
            ) : (
              users.map((u) => (
                <tr key={u.id} className="border-b border-dark-800 hover:bg-dark-800/50">
                  <td className="px-5 py-3">
                    <div className="flex items-center gap-2">
                      {u.role === 'admin' ? (
                        <Shield className="w-4 h-4 text-primary-500" />
                      ) : (
                        <User className="w-4 h-4 text-dark-200" />
                      )}
                      <span className="text-sm font-medium text-white">{u.username}</span>
                    </div>
                  </td>
                  <td className="px-5 py-3">
                    <span className={`text-xs px-2 py-1 rounded-full ${
                      u.role === 'admin' ? 'bg-primary-500/10 text-primary-500' : 'bg-dark-700 text-dark-200'
                    }`}>
                      {u.role}
                    </span>
                  </td>
                  <td className="px-5 py-3 text-sm text-dark-200">
                    {new Date(u.created_at).toLocaleDateString()}
                  </td>
                  <td className="px-5 py-3 text-right">
                    {u.username !== 'admin' && currentUser?.role === 'admin' && (
                      <button
                        onClick={() => handleDelete(u.username)}
                        className="p-2 text-dark-200 hover:text-red-400 hover:bg-red-500/10 rounded-lg transition-colors"
                        title="Delete user"
                      >
                        <Trash2 className="w-4 h-4" />
                      </button>
                    )}
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
