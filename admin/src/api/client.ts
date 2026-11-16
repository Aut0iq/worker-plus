const TOKEN_KEY = 'prila_admin_token';
const USER_KEY = 'prila_admin_user';

export const getToken = () => localStorage.getItem(TOKEN_KEY);
export const getSavedUser = () => {
  const s = localStorage.getItem(USER_KEY);
  return s ? JSON.parse(s) : null;
};
export const saveAuth = (token: string, user: unknown) => {
  localStorage.setItem(TOKEN_KEY, token);
  localStorage.setItem(USER_KEY, JSON.stringify(user));
};
export const clearAuth = () => {
  localStorage.removeItem(TOKEN_KEY);
  localStorage.removeItem(USER_KEY);
};

export class ApiError extends Error {
  code: string;
  status: number;
  constructor(status: number, code: string, message: string) {
    super(message);
    this.status = status;
    this.code = code;
  }
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers: Record<string, string> = { ...(init.headers as Record<string, string>) };
  const token = getToken();
  if (token) headers.authorization = `Bearer ${token}`;
  if (init.body && !(init.body instanceof FormData) && !headers['content-type']) {
    headers['content-type'] = 'application/json';
  }
  const resp = await fetch(path, { ...init, headers });
  if (resp.status === 401) {
    clearAuth();
    if (location.pathname !== '/login') location.href = '/login';
    throw new ApiError(401, 'unauthorized', 'Сессия истекла, войдите снова');
  }
  const text = await resp.text();
  let body: unknown = null;
  try {
    body = text ? JSON.parse(text) : null;
  } catch {
    /* not json */
  }
  if (!resp.ok) {
    const e = (body as { error?: { code?: string; message?: string } })?.error;
    throw new ApiError(resp.status, e?.code ?? 'error', e?.message ?? `Ошибка ${resp.status}`);
  }
  return body as T;
}

export const api = {
  get: <T>(path: string) => request<T>(path),
  post: <T>(path: string, body?: unknown) =>
    request<T>(path, { method: 'POST', body: body instanceof FormData ? body : JSON.stringify(body ?? {}) }),
  patch: <T>(path: string, body: unknown) =>
    request<T>(path, { method: 'PATCH', body: JSON.stringify(body) }),
  del: <T>(path: string) => request<T>(path, { method: 'DELETE' }),
};

/** Authorized file download -> object URL (photos, xlsx, images). */
export async function downloadBlob(path: string): Promise<string> {
  const token = getToken();
  const resp = await fetch(path, { headers: token ? { authorization: `Bearer ${token}` } : {} });
  if (!resp.ok) throw new ApiError(resp.status, 'download', `Ошибка загрузки ${resp.status}`);
  return URL.createObjectURL(await resp.blob());
}

export async function downloadFile(path: string, filename: string) {
  const url = await downloadBlob(path);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 5000);
}
