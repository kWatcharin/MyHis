// อ่านสีจาก CSS variable ของธีม (src/index.css) เป็น sRGB 0–1 สำหรับโค้ดที่วาดเองนอก CSS (WebGL/canvas)
// token ของธีมเขียนเป็น oklch()/hex ปนกัน — ให้เบราว์เซอร์ parse ให้ด้วยการวาด 1px ลง canvas 2D แล้วอ่าน pixel กลับ
// (ใช้ได้กับทุกรูปแบบสีที่ CSS รู้จัก ไม่ต้องเขียนตัวแปลง oklch เอง)
// ค่าที่ได้เป็น sRGB ไม่ใช่ linear — shader ที่เขียนสีออก canvas ตรง ๆ ใช้ได้เลยโดยไม่ต้องแปลง

export type Rgb = readonly [r: number, g: number, b: number]

// ค่าที่ CSS color ปกติไม่มีทางเป็น ใช้เช็กว่าการตั้ง fillStyle ด้วยค่าจริงสำเร็จไหม —
// ถ้า parse ไม่ได้ canvas จะเงียบและคงค่าเดิมไว้ (ไม่ throw)
const SENTINEL = '#010203'

let ctx: CanvasRenderingContext2D | null | undefined

function context(): CanvasRenderingContext2D | null {
  if (ctx === undefined) {
    const canvas = document.createElement('canvas')
    canvas.width = 1
    canvas.height = 1
    ctx = canvas.getContext('2d', { willReadFrequently: true })
  }
  return ctx
}

// null = ไม่มีตัวแปรนี้ / parse ไม่ได้ / ไม่มี canvas — ผู้เรียกใช้สีสำรองของตัวเอง
export function readThemeColor(varName: string, element: Element = document.documentElement): Rgb | null {
  const value = getComputedStyle(element).getPropertyValue(varName).trim()
  if (!value) return null
  const c = context()
  if (!c) return null

  c.fillStyle = SENTINEL
  const sentinel = c.fillStyle
  c.fillStyle = value
  if (c.fillStyle === sentinel && value.toLowerCase() !== SENTINEL) return null

  c.clearRect(0, 0, 1, 1)
  c.fillRect(0, 0, 1, 1)
  const [r, g, b] = c.getImageData(0, 0, 1, 1).data
  return [r / 255, g / 255, b / 255]
}
