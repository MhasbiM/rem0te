import { useState, useRef, useCallback } from 'react';
import {
  X,
  Maximize2,
  Minimize2,
  Monitor,
  Upload,
  Download,
  Clipboard,
  Keyboard,
  MousePointer,
} from 'lucide-react';

interface ConnectionInfo {
  peerId: string;
  hostname: string;
  os: string;
}

interface Props {
  connection: ConnectionInfo;
  onDisconnect: () => void;
}

export default function RemoteView({ connection, onDisconnect }: Props) {
  const [fullscreen, setFullscreen] = useState(false);
  const [quality, setQuality] = useState<'low' | 'medium' | 'high'>('medium');
  const [viewMode, setViewMode] = useState<'fit' | 'original' | 'stretch'>('fit');
  const canvasRef = useRef<HTMLDivElement>(null);

  const handleFullscreen = useCallback(() => {
    if (!document.fullscreenElement) {
      canvasRef.current?.requestFullscreen();
      setFullscreen(true);
    } else {
      document.exitFullscreen();
      setFullscreen(false);
    }
  }, []);

  // Keyboard event forwarding
  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    // Forward keyboard events to Tauri backend
    console.log('Key event to forward:', e.key, e.code);
    // In production: invoke('send_key_event', { key: e.key, code: e.code, type: 'down' })
  }, []);

  return (
    <div className="h-full flex flex-col bg-dark-950" onKeyDown={handleKeyDown} tabIndex={0}>
      {/* Toolbar */}
      <div className="bg-dark-900 border-b border-dark-700 px-4 py-2 flex items-center gap-2">
        {/* Connection info */}
        <div className="flex items-center gap-2 mr-4">
          <Monitor className="w-4 h-4 text-green-400" />
          <div>
            <span className="text-sm font-medium text-white">{connection.hostname}</span>
            <span className="text-xs text-dark-200 ml-2">{connection.os}</span>
          </div>
        </div>

        <div className="flex-1" />

        {/* Quality selector */}
        <select
          value={quality}
          onChange={(e) => setQuality(e.target.value as any)}
          className="bg-dark-800 border border-dark-700 rounded px-2 py-1 text-xs text-white"
        >
          <option value="low">Low Quality</option>
          <option value="medium">Medium</option>
          <option value="high">High Quality</option>
        </select>

        {/* View mode */}
        <select
          value={viewMode}
          onChange={(e) => setViewMode(e.target.value as any)}
          className="bg-dark-800 border border-dark-700 rounded px-2 py-1 text-xs text-white"
        >
          <option value="fit">Fit</option>
          <option value="original">Original</option>
          <option value="stretch">Stretch</option>
        </select>

        {/* Actions */}
        <button
          className="p-1.5 text-dark-200 hover:text-white hover:bg-dark-800 rounded transition-colors"
          title="Send Ctrl+Alt+Del"
        >
          <Keyboard className="w-4 h-4" />
        </button>
        <button
          className="p-1.5 text-dark-200 hover:text-white hover:bg-dark-800 rounded transition-colors"
          title="Toggle mouse mode"
        >
          <MousePointer className="w-4 h-4" />
        </button>
        <button
          className="p-1.5 text-dark-200 hover:text-white hover:bg-dark-800 rounded transition-colors"
          title="Clipboard sync"
        >
          <Clipboard className="w-4 h-4" />
        </button>

        <div className="w-px h-6 bg-dark-700 mx-1" />

        <button
          onClick={handleFullscreen}
          className="p-1.5 text-dark-200 hover:text-white hover:bg-dark-800 rounded transition-colors"
          title={fullscreen ? 'Exit fullscreen' : 'Fullscreen'}
        >
          {fullscreen ? <Minimize2 className="w-4 h-4" /> : <Maximize2 className="w-4 h-4" />}
        </button>

        <button
          onClick={onDisconnect}
          className="p-1.5 text-dark-200 hover:text-red-400 hover:bg-red-500/10 rounded transition-colors"
          title="Disconnect"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* Remote desktop canvas */}
      <div
        ref={canvasRef}
        className="flex-1 flex items-center justify-center bg-dark-950 p-4"
      >
        <div className="bg-dark-900 border-2 border-dashed border-dark-700 rounded-xl w-full h-full flex flex-col items-center justify-center">
          <Monitor className="w-16 h-16 text-dark-700 mb-4" />
          <p className="text-dark-200 text-lg font-medium">Connected to {connection.hostname}</p>
          <p className="text-dark-200 text-sm mt-2">
            Remote desktop stream will render here via the Tauri Rust backend
          </p>
          <div className="mt-4 bg-dark-800 rounded-lg px-4 py-2">
            <p className="text-xs text-dark-200 font-mono">
              Peer ID: {connection.peerId}
            </p>
          </div>

          {/* Status indicators */}
          <div className="flex gap-4 mt-6">
            <div className="flex items-center gap-2 text-xs">
              <div className="w-2 h-2 rounded-full bg-green-400" />
              <span className="text-green-400">Connected</span>
            </div>
            <div className="flex items-center gap-2 text-xs">
              <div className="w-2 h-2 rounded-full bg-blue-400 animate-pulse" />
              <span className="text-blue-400">Stream: {quality}</span>
            </div>
            <div className="flex items-center gap-2 text-xs">
              <div className="w-2 h-2 rounded-full bg-dark-200" />
              <span className="text-dark-200">Latency: --</span>
            </div>
          </div>
        </div>
      </div>

      {/* Status bar */}
      <div className="bg-dark-900 border-t border-dark-700 px-4 py-1.5 flex items-center gap-4 text-xs text-dark-200">
        <span>Remote: {connection.peerId}</span>
        <span>OS: {connection.os}</span>
        <span>Resolution: --</span>
        <span>FPS: --</span>
        <span className="ml-auto">rem0te v0.1.0</span>
      </div>
    </div>
  );
}
