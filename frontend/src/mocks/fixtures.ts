// mock ตัวเลขจำนวนคนต่อเวรในสัปดาห์ — สำหรับกราฟหน้า dashboard เท่านั้น ไม่ใช่ข้อมูลจริง
// (ยังไม่มีฟีเจอร์ตารางเวรจริงใน Milestone นี้ — roles/pages ย้ายไปต่อ API จริงแล้ว ดู
// features/admin/rolesApi.ts, pagesApi.ts)
export const mockShiftChartData = [
  { day: 'จ', morning: 4, afternoon: 3, night: 2 },
  { day: 'อ', morning: 5, afternoon: 3, night: 2 },
  { day: 'พ', morning: 4, afternoon: 4, night: 3 },
  { day: 'พฤ', morning: 6, afternoon: 3, night: 2 },
  { day: 'ศ', morning: 5, afternoon: 4, night: 3 },
  { day: 'ส', morning: 3, afternoon: 2, night: 2 },
  { day: 'อา', morning: 3, afternoon: 2, night: 1 },
]
