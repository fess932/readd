import { reactive } from 'vue';

export interface User {
  id: number;
  name: string;
  isAdmin: boolean;
}

const TOKEN_KEY = 'readd_token';

/** JWT payloads are base64url-encoded UTF-8, which plain `atob` does not understand. */
function decodePayload(token: string): any {
  const base64 = token.split('.')[1].replace(/-/g, '+').replace(/_/g, '/');
  const bytes = Uint8Array.from(atob(base64), c => c.charCodeAt(0));
  return JSON.parse(new TextDecoder().decode(bytes));
}

function loadFromStorage(): { user: User | null; token: string | null } {
  const token = localStorage.getItem(TOKEN_KEY);
  if (!token) return { user: null, token: null };
  try {
    const payload = decodePayload(token);
    return { token, user: { id: payload.id, name: payload.name, isAdmin: payload.isAdmin } };
  } catch {
    return { user: null, token: null };
  }
}

const initial = loadFromStorage();

export const auth = reactive({
  user: initial.user as User | null,
  token: initial.token as string | null,
});

export function setAuth(token: string, user: User) {
  localStorage.setItem(TOKEN_KEY, token);
  auth.user = user;
  auth.token = token;
}

export function clearAuth() {
  localStorage.removeItem(TOKEN_KEY);
  auth.user = null;
  auth.token = null;
}
