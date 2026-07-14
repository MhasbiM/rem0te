import { useState } from 'react';
import ConnectView from './pages/ConnectView';
import RemoteView from './pages/RemoteView';
import FileTransferView from './pages/FileTransferView';
import { Monitor, FolderOpen, Settings, Menu } from 'lucide-react';

type View = 'connect' | 'remote' | 'files' | 'settings';

interface ConnectionInfo {
  peerId: string;
  hostname: string;
  os: string;
}

export default function App() {
  const [currentView, setCurrentView] = useState<View>('connect');
  const [activeConnection, setActiveConnection] = useState<ConnectionInfo | null>(null);

  const navItems: { view: View; icon: typeof Monitor; label: string }[] = [
    { view: 'connect', icon: Monitor, label: 'Connect' },
    { view: 'files', icon: FolderOpen, label: 'Files' },
  ];

  return (
    <div className="h-screen flex flex-col">
      {/* Header */}
      <header className="bg-dark-900 border-b border-dark-700 px-4 py-3 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="w-7 h-7 bg-primary-600 rounded-lg flex items-center justify-center">
            <Monitor className="w-4 h-4 text-white" />
          </div>
          <h1 className="font-bold text-white">rem0te</h1>
          {activeConnection && (
            <span className="text-xs text-dark-200 ml-2">
              Connected to {activeConnection.hostname}
            </span>
          )}
        </div>
      </header>

      {/* Main content */}
      <div className="flex-1 flex overflow-hidden">
        {/* Sidebar nav */}
        <nav className="w-16 bg-dark-900 border-r border-dark-700 flex flex-col items-center py-4 gap-2">
          {navItems.map(({ view, icon: Icon, label }) => (
            <button
              key={view}
              onClick={() => setCurrentView(view)}
              className={`p-2.5 rounded-lg transition-colors group relative ${
                currentView === view
                  ? 'bg-primary-600/20 text-primary-500'
                  : 'text-dark-200 hover:text-white hover:bg-dark-800'
              }`}
              title={label}
            >
              <Icon className="w-5 h-5" />
              <span className="absolute left-14 bg-dark-800 text-white text-xs px-2 py-1 rounded opacity-0 group-hover:opacity-100 pointer-events-none whitespace-nowrap transition-opacity z-50">
                {label}
              </span>
            </button>
          ))}
        </nav>

        {/* View content */}
        <div className="flex-1 overflow-auto">
          {currentView === 'connect' && (
            <ConnectView
              onConnected={(info) => {
                setActiveConnection(info);
                setCurrentView('remote');
              }}
            />
          )}
          {currentView === 'remote' && activeConnection && (
            <RemoteView
              connection={activeConnection}
              onDisconnect={() => {
                setActiveConnection(null);
                setCurrentView('connect');
              }}
            />
          )}
          {currentView === 'files' && (
            <FileTransferView connection={activeConnection} />
          )}
        </div>
      </div>
    </div>
  );
}
