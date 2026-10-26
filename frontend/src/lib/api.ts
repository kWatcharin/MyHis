// fetch wrapper กลาง — เรียก backend ผ่าน /api เสมอ (same-origin เพราะ dev proxy ของ Vite และ
// reverse proxy ของ Caddy ฝั่ง frontend forward /api/* ไปหา backend ให้แล้ว ดู vite.config.ts,
// frontend/Caddyfile) credentials: 'include' ให้ browser แนบ/รับ session cookie ตามปกติ
const BASE = '/api'

export class ApiError extends Error {
  status: number
  retryAfterSeconds: number | null
  // body JSON ทั้งก้อนของ error (เช่น remainingAttempts ของ login ไม่ผ่าน) — null ถ้าไม่มี/อ่านไม่ได้
  body: unknown

  constructor(
    status: number,
    message: string,
    retryAfterSeconds: number | null = null,
    body: unknown = null,
  ) {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.retryAfterSeconds = retryAfterSeconds
    this.body = body
  }
}

// ค่าที่ต่อเข้า path ของ API ต้องผ่านฟังก์ชันนี้เสมอ — id ที่มาจาก URL ของหน้าเว็บ (useParams) ผู้อื่นกำหนดได้ และ react-router
// ถอดรหัส %2F / %3F ให้ จึงมี '/', '..', '?' ได้ ถ้าต่อตรง ๆ คำขอจะหลุดไป endpoint อื่นของ API ด้วยคุกกี้ของผู้ใช้
// '.' / '..' encode แล้วก็ไม่รอด (encodeURIComponent ไม่แตะจุด และเบราว์เซอร์ถือ %2e เป็น dot segment ด้วย) จึงปฏิเสธทั้งหมด —
// หน้าที่รับ id จาก URL ตรวจ isUuid (lib/uuid.ts) ก่อนเรียก API อยู่แล้ว นี่คือด่านสุดท้ายถ้าหลุดมา
export const seg = (value: string): string => {
  if (value === '' || value === '.' || value === '..') throw new Error('invalid path segment')
  return encodeURIComponent(value)
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const response = await fetch(`${BASE}${path}`, {
    credentials: 'include',
    ...init,
    headers: {
      ...(init.body ? { 'Content-Type': 'application/json' } : {}),
      ...init.headers,
    },
  })

  // body ว่าง (เช่น POST /auth/logout) ก็ผ่าน .catch นี้ได้ปกติ กลายเป็น null
  const body: unknown = await response.json().catch(() => null)

  if (!response.ok) {
    const message =
      (body as { error?: string } | null)?.error ?? response.statusText
    const retryAfter = response.headers.get('Retry-After')
    throw new ApiError(response.status, message, retryAfter ? Number(retryAfter) : null, body)
  }

  return body as T
}

export const api = {
  get: <T>(path: string) => request<T>(path),
  post: <T>(path: string, data?: unknown) =>
    request<T>(path, {
      method: 'POST',
      body: data !== undefined ? JSON.stringify(data) : undefined,
    }),
  put: <T>(path: string, data?: unknown) =>
    request<T>(path, {
      method: 'PUT',
      body: data !== undefined ? JSON.stringify(data) : undefined,
    }),
  patch: <T>(path: string, data?: unknown) =>
    request<T>(path, {
      method: 'PATCH',
      body: data !== undefined ? JSON.stringify(data) : undefined,
    }),
  delete: <T>(path: string) => request<T>(path, { method: 'DELETE' }),
}
