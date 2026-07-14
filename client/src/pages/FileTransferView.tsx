import { useState, useCallback } from 'react';
import {
  FolderOpen,
  Upload,
  Download,
  File,
  Folder,
  ChevronRight,
  Home,
  RefreshCw,
  X,
  ArrowRight,
} from 'lucide-react';

interface ConnectionInfo {
  peerId: string;
  hostname: string;
  os: string;
}

interface Props {
  connection: ConnectionInfo | null;
}

interface FileEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  modified: string;
}

export default function FileTransferView({ connection }: Props) {
  const [localPath, setLocalPath] = useState('/home/user');
  const [remotePath, setRemotePath] = useState('/home/user');
  const [localFiles, setLocalFiles] = useState<FileEntry[]>([]);
  const [remoteFiles, setRemoteFiles] = useState<FileEntry[]>([]);
  const [selectedLocal, setSelectedLocal] = useState<string[]>([]);
  const [selectedRemote, setSelectedRemote] = useState<string[]>([]);
  const [transfers, setTransfers] = useState<Array<{
    id: string;
    name: string;
    direction: 'upload' | 'download';
    progress: number;
    status: 'pending' | 'transferring' | 'done' | 'error';
  }>>([]);

  // Simulated file listing - in production, calls Tauri backend
  const refreshLocal = useCallback(() => {
    setLocalFiles([
      { name: 'Documents', path: `${localPath}/Documents`, is_dir: true, size: 0, modified: '2024-01-15' },
      { name: 'Downloads', path: `${localPath}/Downloads`, is_dir: true, size: 0, modified: '2024-01-14' },
      { name: 'Desktop', path: `${localPath}/Desktop`, is_dir: true, size: 0, modified: '2024-01-13' },
      { name: 'readme.md', path: `${localPath}/readme.md`, is_dir: false, size: 2048, modified: '2024-01-10' },
      { name: 'config.json', path: `${localPath}/config.json`, is_dir: false, size: 512, modified: '2024-01-09' },
    ]);
  }, [localPath]);

  const refreshRemote = useCallback(() => {
    if (!connection) return;
    setRemoteFiles([
      { name: 'Projects', path: `${remotePath}/Projects`, is_dir: true, size: 0, modified: '2024-01-15' },
      { name: 'logs', path: `${remotePath}/logs`, is_dir: true, size: 0, modified: '2024-01-14' },
      { name: 'app.log', path: `${remotePath}/app.log`, is_dir: false, size: 4096, modified: '2024-01-15' },
      { name: 'data.db', path: `${remotePath}/data.db`, is_dir: false, size: 1048576, modified: '2024-01-13' },
    ]);
  }, [remotePath, connection]);

  const handleUpload = () => {
    if (selectedLocal.length === 0) return;
    const newTransfers = selectedLocal.map((path, i) => ({
      id: `upload-${Date.now()}-${i}`,
      name: path.split('/').pop() || path,
      direction: 'upload' as const,
      progress: 0,
      status: 'pending' as const,
    }));
    setTransfers((prev) => [...prev, ...newTransfers]);
    setSelectedLocal([]);
  };

  const handleDownload = () => {
    if (selectedRemote.length === 0) return;
    const newTransfers = selectedRemote.map((path, i) => ({
      id: `download-${Date.now()}-${i}`,
      name: path.split('/').pop() || path,
      direction: 'download' as const,
      progress: 0,
      status: 'pending' as const,
    }));
    setTransfers((prev) => [...prev, ...newTransfers]);
    setSelectedRemote([]);
  };

  const formatSize = (bytes: number) => {
    if (bytes === 0) return '-';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="px-6 py-4 border-b border-dark-700">
        <h2 className="text-lg font-bold text-white">File Transfer</h2>
        {connection ? (
          <p className="text-sm text-dark-200">Connected to {connection.hostname}</p>
        ) : (
          <p className="text-sm text-yellow-400">Not connected - file browsing only available on local machine</p>
        )}
      </div>

      {/* Dual pane file browser */}
      <div className="flex-1 flex overflow-hidden">
        {/* Local files */}
        <div className="flex-1 border-r border-dark-700 flex flex-col">
          <div className="px-4 py-3 border-b border-dark-700 flex items-center justify-between bg-dark-900">
            <div className="flex items-center gap-2">
              <Home className="w-4 h-4 text-dark-200" />
              <span className="text-sm font-medium text-white">Local</span>
            </div>
            <div className="flex gap-1">
              <button
                onClick={refreshLocal}
                className="p-1.5 text-dark-200 hover:text-white rounded hover:bg-dark-800"
              >
                <RefreshCw className="w-3.5 h-3.5" />
              </button>
              <button
                onClick={handleUpload}
                disabled={selectedLocal.length === 0 || !connection}
                className="p-1.5 text-dark-200 hover:text-primary-500 rounded hover:bg-dark-800 disabled:opacity-30"
                title="Upload to remote"
              >
                <Upload className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>
          <div className="px-3 py-2 border-b border-dark-700">
            <input
              type="text"
              value={localPath}
              onChange={(e) => setLocalPath(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && refreshLocal()}
              className="w-full bg-dark-800 border border-dark-700 rounded px-3 py-1.5 text-xs text-white font-mono focus:outline-none focus:border-primary-500"
            />
          </div>
          <div className="flex-1 overflow-auto p-2">
            {localFiles.map((file) => (
              <div
                key={file.path}
                className={`flex items-center gap-2 px-3 py-2 rounded-lg cursor-pointer transition-colors ${
                  selectedLocal.includes(file.path) ? 'bg-primary-600/20' : 'hover:bg-dark-800'
                }`}
                onClick={() => {
                  if (file.is_dir) {
                    setLocalPath(file.path);
                    refreshLocal();
                  } else {
                    setSelectedLocal((prev) =>
                      prev.includes(file.path)
                        ? prev.filter((p) => p !== file.path)
                        : [...prev, file.path]
                    );
                  }
                }}
              >
                {file.is_dir ? (
                  <Folder className="w-4 h-4 text-yellow-400 flex-shrink-0" />
                ) : (
                  <File className="w-4 h-4 text-dark-200 flex-shrink-0" />
                )}
                <span className="text-sm text-white truncate">{file.name}</span>
                <span className="text-xs text-dark-200 ml-auto flex-shrink-0">{formatSize(file.size)}</span>
              </div>
            ))}
          </div>
        </div>

        {/* Transfer arrows */}
        <div className="flex flex-col items-center justify-center px-2 gap-2">
          <button
            onClick={handleUpload}
            disabled={!connection || selectedLocal.length === 0}
            className="p-2 bg-primary-600 hover:bg-primary-700 disabled:opacity-30 rounded-lg transition-colors"
            title="Upload →"
          >
            <ArrowRight className="w-4 h-4 text-white" />
          </button>
          <button
            onClick={handleDownload}
            disabled={!connection || selectedRemote.length === 0}
            className="p-2 bg-dark-800 hover:bg-dark-700 disabled:opacity-30 rounded-lg transition-colors"
            title="← Download"
          >
            <ArrowRight className="w-4 h-4 text-white rotate-180" />
          </button>
        </div>

        {/* Remote files */}
        <div className="flex-1 border-l border-dark-700 flex flex-col">
          <div className="px-4 py-3 border-b border-dark-700 flex items-center justify-between bg-dark-900">
            <div className="flex items-center gap-2">
              <GlobeRemote className="w-4 h-4 text-dark-200" />
              <span className="text-sm font-medium text-white">Remote</span>
              {connection && (
                <span className="text-xs text-dark-200 ml-1">({connection.hostname})</span>
              )}
            </div>
            <div className="flex gap-1">
              <button
                onClick={refreshRemote}
                disabled={!connection}
                className="p-1.5 text-dark-200 hover:text-white rounded hover:bg-dark-800 disabled:opacity-30"
              >
                <RefreshCw className="w-3.5 h-3.5" />
              </button>
              <button
                onClick={handleDownload}
                disabled={selectedRemote.length === 0 || !connection}
                className="p-1.5 text-dark-200 hover:text-primary-500 rounded hover:bg-dark-800 disabled:opacity-30"
                title="Download from remote"
              >
                <Download className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>
          <div className="px-3 py-2 border-b border-dark-700">
            <input
              type="text"
              value={remotePath}
              onChange={(e) => setRemotePath(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && refreshRemote()}
              disabled={!connection}
              className="w-full bg-dark-800 border border-dark-700 rounded px-3 py-1.5 text-xs text-white font-mono focus:outline-none focus:border-primary-500 disabled:opacity-50"
            />
          </div>
          <div className="flex-1 overflow-auto p-2">
            {!connection ? (
              <div className="flex flex-col items-center justify-center h-full text-dark-200">
                <FolderOpen className="w-10 h-10 text-dark-700 mb-2" />
                <p className="text-sm">Connect to a remote machine to browse files</p>
              </div>
            ) : (
              remoteFiles.map((file) => (
                <div
                  key={file.path}
                  className={`flex items-center gap-2 px-3 py-2 rounded-lg cursor-pointer transition-colors ${
                    selectedRemote.includes(file.path) ? 'bg-primary-600/20' : 'hover:bg-dark-800'
                  }`}
                  onClick={() => {
                    if (file.is_dir) {
                      setRemotePath(file.path);
                      refreshRemote();
                    } else {
                      setSelectedRemote((prev) =>
                        prev.includes(file.path)
                          ? prev.filter((p) => p !== file.path)
                          : [...prev, file.path]
                      );
                    }
                  }}
                >
                  {file.is_dir ? (
                    <Folder className="w-4 h-4 text-yellow-400 flex-shrink-0" />
                  ) : (
                    <File className="w-4 h-4 text-dark-200 flex-shrink-0" />
                  )}
                  <span className="text-sm text-white truncate">{file.name}</span>
                  <span className="text-xs text-dark-200 ml-auto flex-shrink-0">{formatSize(file.size)}</span>
                </div>
              ))
            )}
          </div>
        </div>
      </div>

      {/* Transfer queue */}
      {transfers.length > 0 && (
        <div className="border-t border-dark-700 bg-dark-900">
          <div className="px-4 py-2 border-b border-dark-700 flex items-center justify-between">
            <span className="text-xs font-medium text-white">Transfers ({transfers.length})</span>
          </div>
          <div className="max-h-32 overflow-auto">
            {transfers.map((t) => (
              <div key={t.id} className="px-4 py-2 flex items-center gap-3 border-b border-dark-800">
                <span className="text-xs text-dark-200">
                  {t.direction === 'upload' ? <Upload className="w-3 h-3" /> : <Download className="w-3 h-3" />}
                </span>
                <span className="text-sm text-white flex-1 truncate">{t.name}</span>
                <span className="text-xs text-dark-200">
                  {t.status === 'done' ? '✓ Done' : t.status === 'error' ? '✗ Error' : `${t.progress}%`}
                </span>
                <button className="p-0.5 text-dark-200 hover:text-red-400">
                  <X className="w-3 h-3" />
                </button>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// Custom icon component for remote globe
function GlobeRemote({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="10" />
      <line x1="2" y1="12" x2="22" y2="12" />
      <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
    </svg>
  );
}
