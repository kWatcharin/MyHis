// สรุป User-Agent เป็น "เบราว์เซอร์ · ระบบ" แบบคร่าว ๆ (ไม่เพิ่ม dependency) — ลำดับการเช็คสำคัญ: Edge/Opera/Samsung
// มีคำว่า Chrome ใน UA ด้วย ต้องเช็คก่อน Chrome, Chrome/Firefox บน iOS มีคำว่า Safari ต้องเช็คก่อน Safari
// iPad รุ่นใหม่รายงานตัวเป็น Macintosh จึงขึ้นเป็น macOS
const BROWSERS: [RegExp, string][] = [
  [/EdgA?\/|EdgiOS\/|Edge\//, 'Edge'],
  [/OPR\/|Opera/, 'Opera'],
  [/SamsungBrowser\//, 'Samsung Internet'],
  [/CriOS\//, 'Chrome'],
  [/FxiOS\/|Firefox\//, 'Firefox'],
  [/Chrome\//, 'Chrome'],
  [/Safari\//, 'Safari'],
]

const SYSTEMS: [RegExp, string][] = [
  [/Windows/, 'Windows'],
  [/Android/, 'Android'],
  [/iPhone|iPod/, 'iPhone'],
  [/iPad/, 'iPad'],
  [/CrOS/, 'ChromeOS'],
  [/Mac OS X|Macintosh/, 'macOS'],
  [/Linux/, 'Linux'],
]

export function describeUserAgent(userAgent: string | null): string {
  if (!userAgent) return '—'
  const browser = BROWSERS.find(([pattern]) => pattern.test(userAgent))?.[1]
  const system = SYSTEMS.find(([pattern]) => pattern.test(userAgent))?.[1]
  if (!browser && !system) {
    // ไม่ใช่เบราว์เซอร์ทั่วไป (เช่น curl/8.0) — แสดงคำแรกพอให้รู้ว่าเป็นอะไร
    return userAgent.split(' ')[0].slice(0, 40)
  }
  return [browser ?? 'ไม่ทราบเบราว์เซอร์', system ?? 'ไม่ทราบระบบ'].join(' · ')
}
