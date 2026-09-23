import path from 'node:path'

import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(import.meta.dirname, './src'),
    },
  },
  server: {
    proxy: {
      // ให้ browser เห็น /api เป็น origin เดียวกับหน้าเว็บ (localhost:5173) ไม่ต้องตั้ง CORS
      // ฝั่ง backend — พอร์ต 8081 มาจาก docker-compose.override.yml (publish เฉพาะ dev)
      // ใช้ 127.0.0.1 ตรง ๆ ไม่ใช่ "localhost" — Node resolve "localhost" เป็น ::1 (IPv6) ก่อน
      // บนเครื่องนี้ แต่ docker publish พอร์ตไว้แค่ฝั่ง IPv4 (127.0.0.1) ทำให้ proxy ต่อไม่ติด (502)
      '/api': 'http://127.0.0.1:8081',
    },
  },
})
