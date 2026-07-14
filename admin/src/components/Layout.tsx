import { NavLink, useNavigate } from 'react-router-dom';
import { useAuth } from './AuthContext';
import {
  LayoutDashboard,
  Monitor,
  Users,
  FolderOpen,
  Settings,
  LogOut,
  Activity,
} from 'lucide-react';

const navItems = [
  { to: '/', icon: LayoutDashboard, label: 'Dashboard' },
  { to: '/peers', icon: Monitor, label: 'Peers' },
  { to: '/users', icon: Users, label: 'Users' },
  { to: '/files', icon: FolderOpen, label: 'File Transfers' },
  { to: '/settings', icon: Settings, label: 'Settings' },
];

export default function Layout({ children }: { children: React.ReactNode }) {
  const { user, logout } = useAuth();
  const navigate = useNavigate();

  const handleLogout = () => {
    logout();
    navigate('/login');
  };

  return (
    <div className="flex h-screen">
      {/* Sidebar */}
      <aside className="w-64 bg-dark-900 border-r border-dark-700 flex flex-col">
        <div className="p-6 border-b border-dark-700">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 bg-primary-600 rounded-lg flex items-center justify-center">
              <Activity className="w-5 h-5 text-white" />
            </div>
            <div>
              <h1 className="text-lg font-bold text-white">rem0te</h1>
              <p className="text-xs text-dark-200">Admin Panel</p>
            </div>
          </div>
        </div>

        <nav className="flex-1 p-4 space-y-1">
          {navItems.map(({ to, icon: Icon, label }) => (
            <NavLink
              key={to}
              to={to}
              end={to === '/'}
              className={({ isActive }) =>
                `flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm transition-colors ${
                  isActive
                    ? 'bg-primary-600/20 text-primary-500'
                    : 'text-dark-200 hover:text-white hover:bg-dark-800'
                }`
              }
            >
              <Icon className="w-4 h-4" />
              {label}
            </NavLink>
          ))}
        </nav>

        <div className="p-4 border-t border-dark-700">
          <div className="flex items-center justify-between">
            <div className="text-sm">
              <p className="text-white font-medium">{user?.username}</p>
              <p className="text-xs text-dark-200 capitalize">{user?.role}</p>
            </div>
            <button
              onClick={handleLogout}
              className="p-2 text-dark-200 hover:text-red-400 hover:bg-dark-800 rounded-lg transition-colors"
              title="Logout"
            >
              <LogOut className="w-4 h-4" />
            </button>
          </div>
        </div>
      </aside>

      {/* Main content */}
      <main className="flex-1 overflow-auto bg-dark-950">
        <div className="p-8">{children}</div>
      </main>
    </div>
  );
}
