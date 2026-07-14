import { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from 'react';

interface User {
  username: string;
  role: string;
}

interface AuthContextType {
  user: User | null;
  token: string | null;
  isAuthenticated: boolean;
  login: (username: string, password: string) => Promise<void>;
  logout: () => void;
}

const AuthContext = createContext<AuthContextType>(null!);

const API_BASE = '/api/v1';

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [token, setToken] = useState<string | null>(() => localStorage.getItem('rem0te_token'));

  const isAuthenticated = !!token && !!user;

  const fetchMe = useCallback(async (t: string) => {
    try {
      const res = await fetch(`${API_BASE}/auth/me`, {
        headers: { Authorization: `Bearer ${t}` },
      });
      if (res.ok) {
        const data = await res.json();
        if (data.success) setUser(data.data);
      } else {
        localStorage.removeItem('rem0te_token');
        setToken(null);
      }
    } catch {
      // Server not available yet
    }
  }, []);

  useEffect(() => {
    if (token) fetchMe(token);
  }, [token, fetchMe]);

  const login = async (username: string, password: string) => {
    const res = await fetch(`${API_BASE}/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password }),
    });
    if (!res.ok) throw new Error('Login failed');
    const data = await res.json();
    if (!data.success) throw new Error(data.error || 'Login failed');
    localStorage.setItem('rem0te_token', data.data.token);
    setToken(data.data.token);
    setUser({ username: data.data.username, role: data.data.role });
  };

  const logout = () => {
    localStorage.removeItem('rem0te_token');
    setToken(null);
    setUser(null);
  };

  return (
    <AuthContext.Provider value={{ user, token, isAuthenticated, login, logout }}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  return useContext(AuthContext);
}

export { API_BASE };
