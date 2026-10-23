export class ApiError extends Error {
  constructor(message: string, public status: number) { super(message) }
}

export async function api<T = any>(path: string, method = 'GET', body?: unknown): Promise<T> {
  const response = await fetch(`/api/console${path}`, {
    method, credentials: 'same-origin', cache: 'no-store',
    headers: body === undefined ? undefined : { 'Content-Type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  const data = response.status === 204 ? null : await response.json().catch(() => null)
  if (!response.ok) throw new ApiError(data?.error || `请求未完成（${response.status}）`, response.status)
  return data as T
}

export interface User { id: number; email: string; role: string; enabled: boolean; dailyLimit: number; usedToday: number; createdAt?: string }
export interface Provider { id: string; name: string; baseUrl: string; models: string[] }
export interface AccessToken { id: number; name: string; prefix: string; createdAt: string; lastUsedAt: string | null; revoked: boolean }
export interface Page<T> { items: T[]; total: number; page: number; pageSize: number }
export interface TokenSecret { id: number; name: string; prefix: string; token: string; ocsConfig: unknown[] }
export interface ModelSettings { providerId: string; model: string; hasKey: boolean; temperature: number; maxTokens: number }
