import * as SecureStore from 'expo-secure-store';

export class ApiError extends Error {
  status: number;
  code: string;
  constructor(status: number, code: string, message: string) {
    super(message);
    this.status = status;
    this.code = code;
  }
}

let baseUrl = (process.env.EXPO_PUBLIC_API_URL ?? 'http://192.168.31.63:8080').replace(/\/+$/, '');
let token: string | null = null;
let onUnauthorized: (() => void) | null = null;

export const getBaseUrl = () => baseUrl;
export const setBaseUrl = (u: string) => {
  baseUrl = u.replace(/\/+$/, '');
};
export const getToken = () => token;
export const setToken = (t: string | null) => {
  token = t;
};
export const setOnUnauthorized = (fn: (() => void) | null) => {
  onUnauthorized = fn;
};

const SERVER_URL_KEY = 'server_url';

export async function loadServerUrl() {
  try {
    const u = await SecureStore.getItemAsync(SERVER_URL_KEY);
    if (u) setBaseUrl(u);
  } catch {
    // SecureStore unavailable (web) — keep default
  }
}

export async function saveServerUrl(u: string | null) {
  try {
    if (u) await SecureStore.setItemAsync(SERVER_URL_KEY, u);
    else await SecureStore.deleteItemAsync(SERVER_URL_KEY);
  } catch {
    // ignore
  }
}

export async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers: Record<string, string> = { ...(init.headers as Record<string, string>) };
  if (token) headers['Authorization'] = `Bearer ${token}`;
  const isForm = typeof FormData !== 'undefined' && init.body instanceof FormData;
  if (init.body && !isForm && !headers['Content-Type']) {
    headers['Content-Type'] = 'application/json';
  }
  let resp: Response;
  try {
    resp = await fetch(`${baseUrl}${path}`, { ...init, headers });
  } catch {
    throw new ApiError(0, 'network', 'Нет связи с сервером');
  }
  if (resp.status === 401) {
    onUnauthorized?.();
    throw new ApiError(401, 'unauthorized', 'Требуется вход');
  }
  const text = await resp.text();
  let data: unknown = null;
  try {
    data = text ? JSON.parse(text) : null;
  } catch {
    // non-JSON response
  }
  if (!resp.ok) {
    const e = (data as { error?: { code?: string; message?: string } } | null)?.error;
    throw new ApiError(resp.status, e?.code ?? 'error', e?.message ?? `Ошибка ${resp.status}`);
  }
  return data as T;
}

export const api = {
  get: <T>(path: string) => request<T>(path),
  post: <T>(path: string, body?: unknown) =>
    request<T>(path, {
      method: 'POST',
      body: body instanceof FormData ? body : JSON.stringify(body ?? {}),
    }),
  del: <T>(path: string, body?: unknown) =>
    request<T>(path, { method: 'DELETE', body: JSON.stringify(body ?? {}) }),
};
