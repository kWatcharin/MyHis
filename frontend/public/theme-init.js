// ตั้ง class ธีมก่อน React mount กันการกระพริบเห็นธีมผิดตอนโหลดหน้าแรก — color-scheme จริง
// (scrollbar, ช่อง date ฯลฯ) กำหนดใน src/index.css ตาม class ธีม
// ต้อง sync กับ THEME_CLASS ใน src/features/theme/ThemeContext.tsx
// แยกเป็นไฟล์ (ไม่ inline ใน index.html) เพราะ Content-Security-Policy `script-src 'self'` บล็อก inline script
;(function () {
  try {
    var stored = localStorage.getItem('my-his-theme')
    var classes = { light: 'theme-light', ocean: 'theme-ocean' }
    document.documentElement.classList.add(classes[stored] || 'dark')
  } catch {
    document.documentElement.classList.add('dark')
  }
})()
