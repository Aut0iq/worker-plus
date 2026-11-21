import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from 'react';
import * as SecureStore from 'expo-secure-store';
import { useQueryClient } from '@tanstack/react-query';
import { api, loadServerUrl, setOnUnauthorized, setToken } from './api/client';
import { getRegisteredPushToken, registerPush } from './push';
import type { UserDto } from './api/types/UserDto';
import type { LoginResponse } from './api/types/LoginResponse';

export type AppConfig = { timezone: string; max_session_hours: number };

type AuthCtx = {
  user: UserDto | null;
  config: AppConfig | null;
  ready: boolean;
  login: (login: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
  refreshMe: () => Promise<void>;
  /** replace the stored token (the server issues a new one after a password change) */
  updateToken: (t: string) => Promise<void>;
};

const Ctx = createContext<AuthCtx | null>(null);
export const useAuth = () => useContext(Ctx)!;

const TOKEN_KEY = 'jwt';

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<UserDto | null>(null);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [ready, setReady] = useState(false);
  const queryClient = useQueryClient();

  const logout = useCallback(async () => {
    const pt = getRegisteredPushToken();
    if (pt) {
      try {
        await api.del('/api/me/push-token', { token: pt });
      } catch {
        // best effort
      }
    }
    try {
      await SecureStore.deleteItemAsync(TOKEN_KEY);
    } catch {
      // ignore
    }
    setToken(null);
    setUser(null);
    queryClient.clear();
  }, [queryClient]);

  useEffect(() => {
    setOnUnauthorized(() => {
      void logout();
    });
    return () => setOnUnauthorized(null);
  }, [logout]);

  useEffect(() => {
    (async () => {
      await loadServerUrl();
      try {
        const t = await SecureStore.getItemAsync(TOKEN_KEY);
        if (t) {
          setToken(t);
          setUser(await api.get<UserDto>('/api/me'));
        }
      } catch {
        setToken(null);
      }
      try {
        setConfig(await api.get<AppConfig>('/api/config'));
      } catch {
        // config stays null; screens fall back to defaults
      }
      setReady(true);
    })();
  }, []);

  const login = useCallback(async (loginName: string, password: string) => {
    const r = await api.post<LoginResponse>('/api/auth/login', {
      login: loginName,
      password,
    });
    setToken(r.token);
    try {
      await SecureStore.setItemAsync(TOKEN_KEY, r.token);
    } catch {
      // ignore
    }
    setUser(r.user);
    void registerPush().catch(() => {});
  }, []);

  const refreshMe = useCallback(async () => {
    setUser(await api.get<UserDto>('/api/me'));
  }, []);

  const updateToken = useCallback(async (t: string) => {
    setToken(t);
    try {
      await SecureStore.setItemAsync(TOKEN_KEY, t);
    } catch {
      // ignore
    }
  }, []);

  return (
    <Ctx.Provider value={{ user, config, ready, login, logout, refreshMe, updateToken }}>
      {children}
    </Ctx.Provider>
  );
}
