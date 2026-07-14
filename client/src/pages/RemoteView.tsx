import { useState, useRef, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  X, Maximize2, Minimize2, Monitor,
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
  const [frameData, setFrameData] = useState<string | null>(null);
  const [streamUrl, setStreamUrl] = useState<string | null>(null);
  const [fps, setFps] = useState(0);
  const canvasRef = useRef<HTMLDivElement>(null);
  const frameCountRef = useRef(0);
  const lastFpsTime = useRef(Date.now());

  // Listen for incoming frames from Tauri backend
  const [streamUrl, setStreamUrl] = useState<string | null>(null);

  useEffect(() => {
    // Listen for stream-ready event from backend
    const unlisten = listen<string>('stream-ready', (event) => {
      setStreamUrl(event.payload);
    });

    // FPS counter: poll image naturalWidth changes
    const interval = setInterval(() => {
      frameCountRef.current++;
      const now = Date.now();
      if (now - lastFpsTime.current >= 1000) {
        setFps(frameCountRef.current);
        frameCountRef.current = 0;
        lastFpsTime.current = now;
      }
    }, 1000);

    return () => {
      unlisten.then(fn => fn());
      clearInterval(interval);
    };
  }, []);

  const handleDisconnect = () => {
    invoke('disconnect_session').catch(() => {});
    onDisconnect();
  };

  const handleFullscreen = useCallback(() => {
    if (!document.fullscreenElement) {
      canvasRef.current?.requestFullscreen();
      setFullscreen(true);
    } else {
      document.exitFullscreen();
      setFullscreen(false);
    }
  }, []);

  // ── Input forwarding ──────────────────────────────────────────

  const sendInput = useCallback((type_: string, payload: Record<string, unknown>) => {
    const ws = (window as any).__rem0te_ws;
    if (ws && ws.readyState === WebSocket.OPEN && connection) {
      ws.send(JSON.stringify({
        type: 'InputEvent',
        payload: {
          from_peer: '',
          to_peer: connection.peerId,
          event: JSON.stringify({ type: type_, ...payload }),
        },
      }));
      return;
    }
    // Fallback: try relay
    invoke('send_input_event', {
      eventType: type_,
      keyCode: payload.key_code || null,
      x: payload.x || null,
      y: payload.y || null,
      button: payload.button || null,
    }).catch(() => {});
  }, [connection]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    e.preventDefault();
    sendInput('keyDown', { key_code: e.code });
  }, [sendInput]);

  const handleKeyUp = useCallback((e: React.KeyboardEvent) => {
    e.preventDefault();
    sendInput('keyUp', { key_code: e.code });
  }, [sendInput]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (!canvasRef.current) return;
    const rect = canvasRef.current.getBoundingClientRect();
    const x = ((e.clientX - rect.left) / rect.width) * 1920;
    const y = ((e.clientY - rect.top) / rect.height) * 1080;
    sendInput('mouseMove', { x, y });
  }, [sendInput]);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    const btn = e.button === 2 ? 'right' : e.button === 1 ? 'middle' : 'left';
    sendInput('mouseDown', { button: btn });
  }, [sendInput]);

  const handleMouseUp = useCallback((e: React.MouseEvent) => {
    const btn = e.button === 2 ? 'right' : e.button === 1 ? 'middle' : 'left';
    sendInput('mouseUp', { button: btn });
  }, [sendInput]);

  return (
    <div className="h-full flex flex-col bg-dark-950" onKeyDown={handleKeyDown} onKeyUp={handleKeyUp} tabIndex={0}>
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

        <button
          onClick={handleFullscreen}
          className="p-1.5 text-dark-200 hover:text-white hover:bg-dark-800 rounded transition-colors"
          title={fullscreen ? 'Exit fullscreen' : 'Fullscreen'}
        >
          {fullscreen ? <Minimize2 className="w-4 h-4" /> : <Maximize2 className="w-4 h-4" />}
        </button>

        <button
          onClick={handleDisconnect}
          className="p-1.5 text-dark-200 hover:text-red-400 hover:bg-red-500/10 rounded transition-colors"
          title="Disconnect"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* Remote desktop canvas */}
      <div ref={canvasRef} className="flex-1 flex items-center justify-center bg-black p-2"
        onMouseMove={handleMouseMove}
        onMouseDown={handleMouseDown}
        onMouseUp={handleMouseUp}
      >
        {streamUrl ? (
          <img src={streamUrl} alt="Remote" className="max-w-full max-h-full object-contain" />
        ) : (
          <div className="text-dark-200 text-center">
            <Monitor className="w-16 h-16 mx-auto mb-3 text-dark-700" />
            <p>Waiting for stream from {connection.hostname}...</p>
          </div>
        )}
      </div>

      {/* Status bar */}
      <div className="bg-dark-900 border-t border-dark-700 px-4 py-1.5 flex items-center gap-4 text-xs text-dark-200">
        <span>Remote: {connection.hostname}</span>
        <span>OS: {connection.os}</span>
        <span>FPS: {fps}</span>
        <span className="ml-auto">rem0te v0.1.0</span>
      </div>
    </div>
  );
}
