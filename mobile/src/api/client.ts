import { Platform } from 'react-native';
import { SecureStore } from '../storage';

export class ApiError extends Error {
  status: number;
  code: string;
  constructor(status: number, code: string, message: string) {
    super(message);
    this.status = status;
    this.code = code;
  }
}

// the web build is served by the API server itself, so it talks to its own origin
const defaultUrl = Platform.OS === 'web' ? window.location.origin : 'http://185.28.173.203';
let baseUrl = (process.env.EXPO_PUBLIC_API_URL ?? defaultUrl).replace(/\/+$/, '');
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

const TIMEOUT_MS = 20_000;
// photo upload on a slow mobile network takes longer than a JSON call
const UPLOAD_TIMEOUT_MS = 90_000;

export async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers: Record<string, string> = { ...(init.headers as Record<string, string>) };
  const hadToken = !!token;
  if (token) headers['Authorization'] = `Bearer ${token}`;
  const isForm = typeof FormData !== 'undefined' && init.body instanceof FormData;
  if (init.body && !isForm && !headers['Content-Type']) {
    headers['Content-Type'] = 'application/json';
  }
  const timeoutMs = isForm ? UPLOAD_TIMEOUT_MS : TIMEOUT_MS;
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), timeoutMs);
  let resp: Response;
  try {
    resp = await fetch(`${baseUrl}${path}`, { ...init, headers, signal: ctrl.signal });
  } catch (e) {
    // only here is it really "no connection"; HTTP errors are handled below
    if (ctrl.signal.aborted) {
      throw new ApiError(
        0,
        'timeout',
        `Сервер не ответил за ${timeoutMs / 1000} с. Проверьте интернет и попробуйте ещё раз`,
      );
    }
    const detail = e instanceof Error && e.message ? ` (${e.message})` : '';
    throw new ApiError(
      0,
      'network',
      `Нет связи с сервером ${baseUrl}. Проверьте интернет (Wi-Fi или мобильные данные)${detail}`,
    );
  } finally {
    clearTimeout(timer);
  }
  const text = await resp.text().catch(() => '');
  let data: unknown = null;
  try {
    data = text ? JSON.parse(text) : null;
  } catch {
    // non-JSON response (proxy error page etc.)
  }
  const e = (data as { error?: { code?: string; message?: string } } | null)?.error;
  if (resp.status === 401) {
    // a wrong password on the login screen is also 401: keep the server message
    if (hadToken) onUnauthorized?.();
    throw new ApiError(401, e?.code ?? 'unauthorized', e?.message ?? 'Требуется вход');
  }
  if (!resp.ok) {
    const fallback =
      resp.status === 413
        ? 'Файл слишком большой'
        : resp.status >= 500
          ? `Ошибка сервера (${resp.status})`
          : `Ошибка ${resp.status}`;
    throw new ApiError(resp.status, e?.code ?? 'http_error', e?.message ?? fallback);
  }
  return data as T;
}

/** User-facing text for any error, with the server error code for support. */
export function errorText(e: unknown, fallback = 'Ошибка'): string {
  if (e instanceof ApiError) {
    return e.status > 0 ? `${e.message} [${e.status} ${e.code}]` : e.message;
  }
  return e instanceof Error && e.message ? e.message : fallback;
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
