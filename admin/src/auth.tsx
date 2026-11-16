import { createContext, useContext, useState, type ReactNode } from 'react';
import { api, clearAuth, getSavedUser, getToken, saveAuth } from './api/client';
import type { UserDto } from './api/types/UserDto';
import type { LoginResponse } from './api/types/LoginResponse';

interface AuthState {
  user: UserDto | null;
  login: (login: string, password: string) => Promise<void>;
  logout: () => void;
}

const AuthCtx = createContext<AuthState>(null!);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<UserDto | null>(() => (getToken() ? getSavedUser() : null));

  const login = async (l: string, p: string) => {
    const r = await api.post<LoginResponse>('/api/auth/login', { login: l, password: p });
    if (r.user.role !== 'admin') throw new Error('Доступ только для администраторов');
    saveAuth(r.token, r.user);
    setUser(r.user);
  };
  const logout = () => {
    clearAuth();
    setUser(null);
  };
  return <AuthCtx.Provider value={{ user, login, logout }}>{children}</AuthCtx.Provider>;
}

export const useAuth = () => useContext(AuthCtx);
