import { Server, Key, Cpu, HardDrive } from 'lucide-react';

export default function Settings() {
  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-2xl font-bold text-white">Settings</h2>
        <p className="text-dark-200 mt-1">Server configuration and status</p>
      </div>

      {/* Server Info */}
      <div className="bg-dark-900 border border-dark-700 rounded-xl p-5 space-y-4">
        <div className="flex items-center gap-3">
          <Server className="w-5 h-5 text-primary-500" />
          <h3 className="font-semibold text-white">Server Information</h3>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div className="bg-dark-800 rounded-lg p-4">
            <p className="text-xs text-dark-200 mb-1">Signaling Port (TCP)</p>
            <p className="text-sm font-mono text-white">21116</p>
          </div>
          <div className="bg-dark-800 rounded-lg p-4">
            <p className="text-xs text-dark-200 mb-1">Relay Port (TCP)</p>
            <p className="text-sm font-mono text-white">21117</p>
          </div>
          <div className="bg-dark-800 rounded-lg p-4">
            <p className="text-xs text-dark-200 mb-1">WebSocket Signaling</p>
            <p className="text-sm font-mono text-white">21118</p>
          </div>
          <div className="bg-dark-800 rounded-lg p-4">
            <p className="text-xs text-dark-200 mb-1">API & Admin Port</p>
            <p className="text-sm font-mono text-white">8080</p>
          </div>
        </div>
      </div>

      {/* Security */}
      <div className="bg-dark-900 border border-dark-700 rounded-xl p-5 space-y-4">
        <div className="flex items-center gap-3">
          <Key className="w-5 h-5 text-yellow-400" />
          <h3 className="font-semibold text-white">Security</h3>
        </div>
        <div className="bg-dark-800 rounded-lg p-4">
          <p className="text-xs text-dark-200 mb-1">JWT Authentication</p>
          <p className="text-sm text-green-400">Enabled</p>
        </div>
        <div className="bg-dark-800 rounded-lg p-4">
          <p className="text-xs text-dark-200 mb-1">Password Hashing</p>
          <p className="text-sm text-green-400">bcrypt (12 rounds)</p>
        </div>
        <div className="bg-yellow-500/10 border border-yellow-500/30 rounded-lg p-4">
          <p className="text-sm text-yellow-400">
            ⚠️ Remember to change the default JWT secret and admin password in production!
          </p>
        </div>
      </div>

      {/* System */}
      <div className="bg-dark-900 border border-dark-700 rounded-xl p-5 space-y-4">
        <div className="flex items-center gap-3">
          <Cpu className="w-5 h-5 text-purple-400" />
          <h3 className="font-semibold text-white">System</h3>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div className="bg-dark-800 rounded-lg p-4">
            <p className="text-xs text-dark-200 mb-1">Runtime</p>
            <p className="text-sm font-mono text-white">Rust (Actix-web + Tokio)</p>
          </div>
          <div className="bg-dark-800 rounded-lg p-4">
            <p className="text-xs text-dark-200 mb-1">Storage</p>
            <p className="text-sm text-white">In-memory (DashMap)</p>
          </div>
        </div>
      </div>

      <p className="text-xs text-dark-200 text-center">rem0te server v0.1.0</p>
    </div>
  );
}
