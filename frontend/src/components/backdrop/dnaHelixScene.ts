// ฉาก DNA helix ของแผงหน้า "ทางเข้า" (login, register) — Three.js ล้วน ไม่มี React
// โหลดผ่าน dynamic import จาก MedicalBackdrop.tsx เท่านั้น เพื่อให้ three อยู่ใน chunk แยก ไม่เข้า bundle หลัก
// แนวทางรวม (สี, งบประสิทธิภาพ, การนำไปใช้หน้าอื่น) อยู่ใน docs/ui-3d-backdrop.md
//
// แบบจำลองเกลียวคู่แบบ "ภาพประกอบการแพทย์" (ผู้ใช้ให้ภาพอ้างอิง 2026-11-24): เส้นหลัก (sugar-phosphate backbone) เป็นท่อแก้วโปร่งแสง (แกนเล็กกลาง + เปลือกใส ขอบเรือง fresnel ไล่สีตามความยาว)
// สองเส้น, คู่เบสเป็นแท่งแก้วสีตามชนิดเบส (A/T/C/G) ปลายแท่งเป็นหัวหกเหลี่ยมมีตัวอักษร, กลางคู่เบสเป็นพันธะไฮโดรเจนเส้นประ
// (A–T 2 เส้น, C–G 3 เส้น) สองสายห่างกันเชิงมุม ~0.8π จึงเห็นร่องใหญ่/ร่องเล็กชัด เกลียวใหญ่เต็มแผง เห็นราว 2–3 รอบ
// geometry สร้างตามขนาดแผง (สร้างใหม่ตอน resize เท่านั้น) ต่อเฟรมแค่หมุนกลุ่ม + เอียงตามเมาส์ + อัปเดตการมองเห็นตัวอักษร

import {
  BackSide,
  BufferAttribute,
  BufferGeometry,
  CanvasTexture,
  Color,
  Curve,
  CylinderGeometry,
  DirectionalLight,
  FrontSide,
  Group,
  HemisphereLight,
  InstancedMesh,
  Matrix4,
  Mesh,
  MeshBasicMaterial,
  MeshStandardMaterial,
  PerspectiveCamera,
  Quaternion,
  Raycaster,
  Scene,
  SRGBColorSpace,
  Sprite,
  SpriteMaterial,
  TubeGeometry,
  Vector2,
  Vector3,
  WebGLRenderer,
} from 'three'

import type { Rgb } from '@/lib/cssColor'

// สี sRGB 0–1 จาก token ของธีม (MedicalBackdrop.readPalette) — ใส่ THREE.Color ด้วย SRGBColorSpace ให้ three แปลงเป็น linear
// สำหรับการคำนวณแสงเอง (วัตถุมีแสงเงา สีบนจอจึงสว่าง/มืดตามมุมแสง ไม่ใช่ค่า token เป๊ะ ๆ ทุกจุด)
export interface HelixPalette {
  // เส้นหลักไล่สีตามความยาวจาก backbone → backbone2 (เส้นที่สองไล่ย้อนกลับ) ขอบแก้วเรืองสี backbone2
  backbone: Rgb
  backbone2: Rgb
  baseA: Rgb
  baseT: Rgb
  baseC: Rgb
  baseG: Rgb
  bond: Rgb
  rim: Rgb
  dark: boolean
}

export interface HelixSceneOptions {
  reducedMotion: boolean
  palette: HelixPalette
}

export interface HelixScene {
  setPalette: (palette: HelixPalette) => void
  // พิกัด normalized device (-1..1, y ขึ้นบน) ของเมาส์/นิ้ว เทียบกับกรอบ canvas
  setPointer: (ndcX: number, ndcY: number) => void
  clearPointer: () => void
  // คลิก/แตะในแผง: หมุนเร็วขึ้นชั่วคราวแล้วค่อย ๆ ช้าลง
  impulse: () => void
  resize: (width: number, height: number) => void
  setReducedMotion: (reduced: boolean) => void
  // false = หยุด rAF (แท็บถูกซ่อน) — ไม่ปล่อย resource
  setActive: (active: boolean) => void
  dispose: () => void
}

// สัดส่วนทั้งหมดเทียบกับรัศมีเกลียว R (ตามภาพอ้างอิง: ระยะเกลียวต่อรอบ ≈ 1.7 เท่าของเส้นผ่านศูนย์กลาง เหมือน B-DNA จริง)
const PITCH = 3.4
const PAIRS_PER_TURN = 10
const STRAND_OFFSET = Math.PI * 0.8
const BACKBONE_RADIUS = 0.2
const STICK_RADIUS = 0.085
const CAP_RADIUS = 0.17
const CAP_DEPTH = 0.1
const DASH_RADIUS = 0.022
const BOND_GAP = 0.2 // สัดส่วนช่องว่างกลางคู่เบส (พันธะไฮโดรเจน) ต่อความยาวทั้งคู่
const LETTER_SIZE = 0.24

const CAMERA_DISTANCE = 30
const CAMERA_FOV = 35
const IDLE_SPIN = 0.12 // rad/s
const MAX_IMPULSE = 5
const DPR_CAP = 2

type Base = 'A' | 'T' | 'C' | 'G'
const PARTNER: Record<Base, Base> = { A: 'T', T: 'A', C: 'G', G: 'C' }
const BASES: Base[] = ['A', 'T', 'C', 'G']

// ลำดับเบสคงที่ (ไม่สุ่มทุกครั้งที่โหลด) แต่ดูไม่ซ้ำเป็นแพทเทิร์น
function baseAt(i: number): Base {
  return BASES[(i * 7 + ((i * i) % 5) + (i >> 2)) % 4]
}

class HelixCurve extends Curve<Vector3> {
  length: number
  radius: number
  phase: number

  constructor(length: number, radius: number, phase: number) {
    super()
    this.length = length
    this.radius = radius
    this.phase = phase
  }

  getPoint(t: number, target = new Vector3()): Vector3 {
    const x = (t - 0.5) * this.length
    const a = (x / (PITCH * this.radius)) * Math.PI * 2 + this.phase
    return target.set(x, this.radius * Math.cos(a), this.radius * Math.sin(a))
  }
}

function letterTexture(letter: Base): CanvasTexture {
  const size = 128
  const canvas = document.createElement('canvas')
  canvas.width = size
  canvas.height = size
  const ctx = canvas.getContext('2d')
  if (ctx) {
    ctx.font = `700 ${size * 0.62}px system-ui, -apple-system, "Segoe UI", sans-serif`
    ctx.textAlign = 'center'
    ctx.textBaseline = 'middle'
    ctx.lineJoin = 'round'
    ctx.lineWidth = size * 0.1
    ctx.strokeStyle = 'rgba(8, 12, 28, 0.85)'
    ctx.strokeText(letter, size / 2, size / 2 + size * 0.04)
    ctx.fillStyle = '#ffffff'
    ctx.fillText(letter, size / 2, size / 2 + size * 0.04)
  }
  const texture = new CanvasTexture(canvas)
  texture.colorSpace = SRGBColorSpace
  return texture
}

const UP = new Vector3(0, 1, 0)
const X_AXIS = new Vector3(1, 0, 0)

interface PairInfo {
  x: number
  bases: [Base, Base]
}

// แกนทึบกลางท่อ เทียบรัศมีท่อ; ความโปร่งของเปลือก (ตรงกลาง — ขอบเพิ่มด้วย fresnel ใน glassMaterial)
const CORE_RATIO = 0.42
const SHELL_BACK_OPACITY = 0.28
const SHELL_FRONT_OPACITY = 0.34
// คู่เบส: ความโปร่งของเปลือกแท่ง/หัว และขนาดแกนทึบข้างในเทียบเปลือก
const STICK_OPACITY = 0.42
const CAP_OPACITY = 0.5
const STICK_CORE_RATIO = 0.45
const CAP_CORE_RATIO = 0.6

// วัสดุ "เรืองในตัว": เพิ่มแสงจากสีของพื้นผิวเอง (รวมสีต่อจุด/ต่ออินสแตนซ์) ให้สีสดไม่หม่นตามมุมแสง
// glow = สัดส่วนสีที่ส่องออกเอง (0 = ขึ้นกับแสงล้วน ๆ)
function litMaterial(glow: number, vertexColors: boolean): MeshStandardMaterial {
  const material = new MeshStandardMaterial({ roughness: 0.28, metalness: 0.05, vertexColors })
  material.onBeforeCompile = (shader) => {
    shader.uniforms.uGlow = { value: glow }
    shader.fragmentShader = shader.fragmentShader
      .replace('#include <common>', `#include <common>
      uniform float uGlow;`)
      .replace(
        '#include <emissivemap_fragment>',
        `#include <emissivemap_fragment>
      totalEmissiveRadiance += diffuseColor.rgb * uGlow;`,
      )
  }
  material.customProgramCacheKey = () => 'dna-lit'
  return material
}

interface GlassOptions {
  // สีขอบเรือง (fresnel) ที่ใช้ร่วมกันและเปลี่ยนตามธีมได้; rimMix 1 = ขอบเป็นสีนี้ทั้งหมด, 0 = ขอบเป็นสีอ่อนของพื้นผิวเอง
  rim: Color
  rimMix: number
  glow: number
  vertexColors: boolean
}

// วัสดุแก้วโปร่งแสง: lit + fresnel ที่ขอบ (มุมเฉียงทึบขึ้นและเรืองสี) ตรงกลางใสให้เห็นของข้างใน
function glassMaterial(side: typeof BackSide | typeof FrontSide, opacity: number, options: GlassOptions): MeshStandardMaterial {
  const material = new MeshStandardMaterial({
    roughness: 0.18,
    metalness: 0,
    transparent: true,
    opacity,
    depthWrite: false,
    side,
    vertexColors: options.vertexColors,
  })
  material.onBeforeCompile = (shader) => {
    shader.uniforms.uGlow = { value: options.glow }
    shader.uniforms.uRim = { value: options.rim }
    shader.uniforms.uRimMix = { value: options.rimMix }
    shader.fragmentShader = shader.fragmentShader
      .replace(
        '#include <common>',
        `#include <common>
      uniform float uGlow;
      uniform vec3 uRim;
      uniform float uRimMix;`,
      )
      .replace(
        '#include <emissivemap_fragment>',
        `#include <emissivemap_fragment>
      totalEmissiveRadiance += diffuseColor.rgb * uGlow;`,
      )
      .replace(
        '#include <opaque_fragment>',
        `float fresnel = pow(1.0 - clamp(abs(dot(normalize(normal), normalize(vViewPosition))), 0.0, 1.0), 2.0);
      vec3 rimTone = mix(mix(diffuseColor.rgb, vec3(1.0), 0.5), uRim, uRimMix);
      outgoingLight = mix(outgoingLight, rimTone * 1.3, fresnel * 0.85);
      diffuseColor.a = clamp(diffuseColor.a + fresnel * 0.6, 0.0, 1.0);
      #include <opaque_fragment>`,
      )
  }
  material.customProgramCacheKey = () => 'dna-glass'
  return material
}

export function createDnaHelixScene(canvas: HTMLCanvasElement, options: HelixSceneOptions): HelixScene {
  // สร้างไม่ได้ (ไม่มี WebGL) → throw ให้ผู้เรียกถอยไปใช้แผงพื้นเรียบ
  const renderer = new WebGLRenderer({ canvas, alpha: true, antialias: true, powerPreference: 'low-power' })
  renderer.setClearColor(0x000000, 0)

  const scene = new Scene()
  const camera = new PerspectiveCamera(CAMERA_FOV, 1, 0.1, 100)
  camera.position.set(0, 0, CAMERA_DISTANCE)

  // แสง: ฟ้า/พื้นนุ่ม ๆ + แสงหลักจากซ้ายบนหน้า (ให้ท่อมีมิติ) + แสงขอบสี primary จากด้านหลัง (ขอบท่อเรืองตามธีม)
  const hemi = new HemisphereLight(0xffffff, 0x223044, 1.1)
  const key = new DirectionalLight(0xffffff, 1.6)
  key.position.set(-6, 9, 12)
  const rim = new DirectionalLight(0xffffff, 1.1)
  rim.position.set(7, -5, -10)
  scene.add(hemi, key, rim)

  // tilt (เอียงตามเมาส์) ⊃ diag (เฉียงตามแผง) ⊃ spin (หมุนรอบแกนเกลียว) ⊃ ชิ้นส่วนเกลียว
  const tilt = new Group()
  const diag = new Group()
  const spin = new Group()
  tilt.add(diag)
  diag.add(spin)
  scene.add(tilt)

  // ท่อเส้นหลัก = แก้วโปร่งแสง 3 ชั้น: แกนทึบเส้นเล็กอยู่กลาง (ให้เห็นเส้นสายสีเข้ม) + เปลือกหลังกับเปลือกหน้าโปร่งแสง
  // ขอบเปลือกเรืองตาม fresnel (ตรงกลางใส ขอบทึบ/สว่าง) ให้เห็นพื้นผิวโค้งและความหนาแทนผิวเรียบทึบ
  // เปลือกไม่เขียน depth เพื่อให้เห็นแท่งเบสทะลุผ่านท่อ เหมือนภาพประกอบแบบใส
  const rimColor = new Color()
  const gradFrom = new Color()
  const gradTo = new Color()
  const backboneMaterial = litMaterial(0.55, true)
  const shellOptions: GlassOptions = { rim: rimColor, rimMix: 1, glow: 0.3, vertexColors: true }
  const shellBackMaterial = glassMaterial(BackSide, SHELL_BACK_OPACITY, shellOptions)
  const shellFrontMaterial = glassMaterial(FrontSide, SHELL_FRONT_OPACITY, shellOptions)
  const pairOptions: GlassOptions = { rim: rimColor, rimMix: 0, glow: 0.4, vertexColors: false }
  const stickMaterial = glassMaterial(FrontSide, STICK_OPACITY, pairOptions)
  const capMaterial = glassMaterial(FrontSide, CAP_OPACITY, pairOptions)
  const coreMaterial = litMaterial(0.55, false)
  const bondMaterial = new MeshBasicMaterial({ transparent: true, opacity: 0.85 })
  const letterTextures = Object.fromEntries(BASES.map((b) => [b, letterTexture(b)])) as Record<Base, CanvasTexture>
  const letterMaterials = Object.fromEntries(
    BASES.map((b) => [b, new SpriteMaterial({ map: letterTextures[b], depthWrite: false, transparent: true })]),
  ) as Record<Base, SpriteMaterial>

  const baseColor: Record<Base, Color> = { A: new Color(), T: new Color(), C: new Color(), G: new Color() }

  // ชิ้นส่วนที่สร้างตามขนาดแผง
  let built: {
    radius: number
    length: number
    tubes: Mesh[]
    sticks: InstancedMesh
    caps: InstancedMesh
    stickCores: InstancedMesh
    capCores: InstancedMesh
    bonds: InstancedMesh
    letters: Sprite[]
    // ตำแหน่งกลางหัวเบสในพิกัดของกลุ่ม spin (ตัวอักษรถูกยกออกมาหน้าหัวทุกเฟรม ดู updateLetters)
    capCenters: Vector3[]
    pairs: PairInfo[]
  } | null = null

  let reducedMotion = options.reducedMotion
  let active = true
  let frame = 0
  let lastTime = 0
  let spinAngle = 0
  let impulse = 0
  const pointerNdc = new Vector2()
  let pointerActive = false
  let pointerDirty = false
  let hoveredPair = -1
  const raycaster = new Raycaster()

  const tmpMatrix = new Matrix4()
  const tmpQuat = new Quaternion()
  const tmpVec = new Vector3()
  const tmpAxis = new Vector3()
  const tmpColor = new Color()

  function applyPalette(next: HelixPalette) {
    const set = (c: Color, rgb: Rgb) => c.setRGB(rgb[0], rgb[1], rgb[2], SRGBColorSpace)
    set(gradFrom, next.backbone)
    set(gradTo, next.backbone2)
    set(rimColor, next.backbone2)
    set(baseColor.A, next.baseA)
    set(baseColor.T, next.baseT)
    set(baseColor.C, next.baseC)
    set(baseColor.G, next.baseG)
    set(bondMaterial.color, next.bond)
    set(rim.color, next.rim)
    // แสงสะท้อนจากพื้น: ธีมมืดเข้ม (เงาลึก), ธีมสว่างเทาอ่อน (วัตถุไม่ดำทึบบนพื้นขาว)
    if (next.dark) hemi.groundColor.setRGB(0.08, 0.1, 0.16, SRGBColorSpace)
    else hemi.groundColor.setRGB(0.55, 0.58, 0.65, SRGBColorSpace)
    hemi.intensity = next.dark ? 1.1 : 1.6
    paintInstances()
    paintTubes()
  }

  // เส้นหลักไล่สีตามความยาว: เส้นแรก from→to เส้นที่สองไล่ย้อน (สองเส้นเป็นสีคู่ตรงข้ามกันทุกตำแหน่ง)
  function paintTubes() {
    if (!built) return
    const done = new Set<BufferGeometry>()
    for (const tube of built.tubes) {
      const geometry = tube.geometry
      if (done.has(geometry)) continue
      done.add(geometry)
      const position = geometry.getAttribute('position')
      let color = geometry.getAttribute('color')
      if (!color) {
        color = new BufferAttribute(new Float32Array(position.count * 3), 3)
        geometry.setAttribute('color', color)
      }
      const reverse = tube.userData.strand === 1
      for (let v = 0; v < position.count; v++) {
        const t = Math.min(Math.max(position.getX(v) / built.length + 0.5, 0), 1)
        tmpColor.lerpColors(reverse ? gradTo : gradFrom, reverse ? gradFrom : gradTo, t)
        color.setXYZ(v, tmpColor.r, tmpColor.g, tmpColor.b)
      }
      color.needsUpdate = true
    }
  }

  // สีต่ออินสแตนซ์: แท่ง/หัวของคู่เบสตามชนิดเบส คู่ที่เมาส์ชี้อยู่สว่างขึ้น (ผสมขาว)
  function paintInstances() {
    if (!built) return
    const { sticks, caps, stickCores, capCores, pairs } = built
    pairs.forEach((pair, i) => {
      for (let side = 0; side < 2; side++) {
        tmpColor.copy(baseColor[pair.bases[side]])
        if (i === hoveredPair) tmpColor.lerp(new Color(1, 1, 1), 0.5)
        sticks.setColorAt(i * 2 + side, tmpColor)
        caps.setColorAt(i * 2 + side, tmpColor)
        stickCores.setColorAt(i * 2 + side, tmpColor)
        capCores.setColorAt(i * 2 + side, tmpColor)
      }
    })
    if (sticks.instanceColor) sticks.instanceColor.needsUpdate = true
    if (caps.instanceColor) caps.instanceColor.needsUpdate = true
    if (stickCores.instanceColor) stickCores.instanceColor.needsUpdate = true
    if (capCores.instanceColor) capCores.instanceColor.needsUpdate = true
    built.letters.forEach((sprite, k) => {
      const s = (Math.floor(k / 2) === hoveredPair ? 1.45 : 1) * LETTER_SIZE * built!.radius
      sprite.scale.set(s, s, 1)
    })
  }

  function disposeBuilt() {
    if (!built) return
    for (const tube of built.tubes) {
      tube.geometry.dispose()
      spin.remove(tube)
    }
    for (const mesh of [built.sticks, built.caps, built.stickCores, built.capCores, built.bonds]) {
      mesh.geometry.dispose()
      mesh.dispose()
      spin.remove(mesh)
    }
    for (const sprite of built.letters) spin.remove(sprite)
    built = null
  }

  function build(radius: number, length: number) {
    disposeBuilt()
    const R = radius

    const tubes: Mesh[] = []
    for (const phase of [0, STRAND_OFFSET]) {
      const strand = phase === 0 ? 0 : 1
      const curve = new HelixCurve(length, R, phase)
      const segments = Math.ceil((length / (PITCH * R)) * 64)
      const shellGeometry = new TubeGeometry(curve, segments, BACKBONE_RADIUS * R, 24, false)
      const core = new Mesh(new TubeGeometry(curve, segments, BACKBONE_RADIUS * CORE_RATIO * R, 12, false), backboneMaterial)
      const shellBack = new Mesh(shellGeometry, shellBackMaterial)
      const shellFront = new Mesh(shellGeometry, shellFrontMaterial)
      // วาดทึบก่อน (แกน/แท่ง/หัว) แล้วเปลือกหลัง → เปลือกหน้า → ตัวอักษร
      shellBack.renderOrder = 1
      shellFront.renderOrder = 2
      for (const m of [core, shellBack, shellFront]) m.userData.strand = strand
      spin.add(core, shellBack, shellFront)
      tubes.push(core, shellBack, shellFront)
    }

    const spacing = (PITCH * R) / PAIRS_PER_TURN
    const count = Math.floor(length / spacing)
    const pairs: PairInfo[] = []
    for (let i = 0; i < count; i++) {
      const base = baseAt(i)
      pairs.push({ x: -length / 2 + (i + 0.5) * spacing, bases: [base, PARTNER[base]] })
    }

    const sticks = new InstancedMesh(new CylinderGeometry(STICK_RADIUS * R, STICK_RADIUS * R, 1, 12), stickMaterial, count * 2)
    const caps = new InstancedMesh(new CylinderGeometry(CAP_RADIUS * R, CAP_RADIUS * R, CAP_DEPTH * R, 6), capMaterial, count * 2)
    // แกนทึบใช้ geometry ร่วมกับเปลือก (ย่อด้วยเมทริกซ์)
    const stickCores = new InstancedMesh(sticks.geometry, coreMaterial, count * 2)
    const capCores = new InstancedMesh(caps.geometry, coreMaterial, count * 2)
    sticks.renderOrder = 2
    caps.renderOrder = 2
    const bonds = new InstancedMesh(new CylinderGeometry(DASH_RADIUS * R, DASH_RADIUS * R, 1, 6), bondMaterial, count * 9)
    const letters: Sprite[] = []
    const capCenters: Vector3[] = []

    const pa = new Vector3()
    const pb = new Vector3()
    const dir = new Vector3()
    const out = new Vector3()
    let bondIndex = 0
    pairs.forEach((pair, i) => {
      const a = (pair.x / (PITCH * R)) * Math.PI * 2
      pa.set(pair.x, R * Math.cos(a), R * Math.sin(a))
      pb.set(pair.x, R * Math.cos(a + STRAND_OFFSET), R * Math.sin(a + STRAND_OFFSET))
      dir.subVectors(pb, pa)
      const span = dir.length()
      dir.normalize()
      // แกนของหัวหกเหลี่ยม = ตั้งฉากกับแท่งและแกนเกลียว (หน้าแบนหันออกจากระนาบคู่เบส เหมือนภาพประกอบ)
      out.crossVectors(dir, X_AXIS).normalize()
      const half = (span * (1 - BOND_GAP)) / 2

      for (let side = 0; side < 2; side++) {
        const from = side === 0 ? pa : pb
        const sign = side === 0 ? 1 : -1
        // แท่ง: จากแกนท่อเส้นหลักเข้าหากลาง
        tmpAxis.copy(dir).multiplyScalar(sign)
        tmpQuat.setFromUnitVectors(UP, tmpAxis)
        tmpVec.copy(from).addScaledVector(tmpAxis, half / 2)
        tmpMatrix.compose(tmpVec, tmpQuat, new Vector3(1, half, 1))
        sticks.setMatrixAt(i * 2 + side, tmpMatrix)
        tmpMatrix.compose(tmpVec, tmpQuat, new Vector3(STICK_CORE_RATIO, half, STICK_CORE_RATIO))
        stickCores.setMatrixAt(i * 2 + side, tmpMatrix)
        // หัว: ที่ปลายแท่งด้านใน
        tmpVec.copy(from).addScaledVector(tmpAxis, half)
        tmpQuat.setFromUnitVectors(UP, out)
        tmpMatrix.compose(tmpVec, tmpQuat, new Vector3(1, 1, 1))
        caps.setMatrixAt(i * 2 + side, tmpMatrix)
        tmpMatrix.compose(tmpVec, tmpQuat, new Vector3(CAP_CORE_RATIO, 1, CAP_CORE_RATIO))
        capCores.setMatrixAt(i * 2 + side, tmpMatrix)
        const sprite = new Sprite(letterMaterials[pair.bases[side]])
        sprite.position.copy(tmpVec)
        sprite.renderOrder = 10
        spin.add(sprite)
        letters.push(sprite)
        capCenters.push(tmpVec.clone())
      }

      // พันธะไฮโดรเจน: เส้นประขนานกัน 2 เส้น (A–T) หรือ 3 เส้น (C–G) ระหว่างหัวสองข้าง เส้นละ 3 ขีด
      const lines = pair.bases[0] === 'A' || pair.bases[0] === 'T' ? 2 : 3
      const gapStart = half + CAP_RADIUS * R * 0.6
      const gapEnd = span - gapStart
      const gapLen = Math.max(gapEnd - gapStart, 0.001)
      const dashLen = gapLen / 5
      tmpQuat.setFromUnitVectors(UP, dir)
      for (let l = 0; l < lines; l++) {
        const offset = (l - (lines - 1) / 2) * 0.09 * R
        for (let d = 0; d < 3; d++) {
          tmpVec
            .copy(pa)
            .addScaledVector(dir, gapStart + dashLen * (0.5 + d * 2))
            .addScaledVector(X_AXIS, offset)
          tmpMatrix.compose(tmpVec, tmpQuat, new Vector3(1, dashLen, 1))
          bonds.setMatrixAt(bondIndex++, tmpMatrix)
        }
      }
    })
    bonds.count = bondIndex
    for (const mesh of [sticks, caps, stickCores, capCores, bonds]) {
      mesh.instanceMatrix.needsUpdate = true
      mesh.computeBoundingSphere()
      spin.add(mesh)
    }

    built = { radius: R, length, tubes, sticks, caps, stickCores, capCores, bonds, letters, capCenters, pairs }
    paintInstances()
    paintTubes()
  }

  // ตัวอักษรบนหัวเบส: ยกออกมาหน้าหัวไปทางกล้องเล็กน้อยทุกเฟรม (sprite ทดสอบความลึกตามปกติ — ท่อเส้นหลักที่อยู่หน้าจึงบัง
  // ตัวอักษรได้ถูกต้อง แต่ตัวหัวเบสเองไม่บัง) และแสดงเฉพาะครึ่งหน้าของเกลียว (ครึ่งหลังมองลอดช่องเห็นแล้วรก)
  const axisPoint = new Vector3()
  const capPoint = new Vector3()
  const toCamera = new Vector3()
  function updateLetters() {
    if (!built) return
    const R = built.radius
    const threshold = R * 0.2
    const lift = (CAP_RADIUS + CAP_DEPTH) * R
    built.letters.forEach((sprite, k) => {
      capPoint.copy(built!.capCenters[k])
      spin.localToWorld(capPoint)
      axisPoint.set(built!.pairs[Math.floor(k / 2)].x, 0, 0)
      spin.localToWorld(axisPoint)
      sprite.visible = capPoint.z - axisPoint.z > threshold
      if (!sprite.visible) return
      toCamera.subVectors(camera.position, capPoint).normalize()
      capPoint.addScaledVector(toCamera, lift)
      sprite.position.copy(spin.worldToLocal(capPoint))
    })
  }

  function updateHover() {
    if (!built || !pointerDirty) return
    pointerDirty = false
    let next = -1
    if (pointerActive) {
      raycaster.setFromCamera(pointerNdc, camera)
      const hit = raycaster.intersectObjects([built.caps, built.sticks], false)[0]
      if (hit?.instanceId !== undefined) next = Math.floor(hit.instanceId / 2)
    }
    if (next !== hoveredPair) {
      hoveredPair = next
      paintInstances()
    }
  }

  function render() {
    tilt.updateMatrixWorld(true)
    updateLetters()
    renderer.render(scene, camera)
  }

  function tick(now: number) {
    frame = requestAnimationFrame(tick)
    const dt = lastTime ? Math.min((now - lastTime) / 1000, 0.05) : 0
    lastTime = now

    impulse *= Math.exp(-dt * 1.4)
    spinAngle += dt * (IDLE_SPIN + impulse)
    spin.rotation.x = spinAngle

    // damping แบบ exponential — ลื่นเท่ากันทุก frame rate
    const ease = 1 - Math.exp(-dt * 3)
    const strength = pointerActive ? 1 : 0
    tilt.rotation.y += (pointerNdc.x * 0.25 * strength - tilt.rotation.y) * ease
    tilt.rotation.x += (-pointerNdc.y * 0.18 * strength - tilt.rotation.x) * ease
    tilt.updateMatrixWorld(true)
    updateHover()
    render()
  }

  function start() {
    if (frame || reducedMotion || !active) return
    lastTime = 0
    frame = requestAnimationFrame(tick)
  }

  function stop() {
    cancelAnimationFrame(frame)
    frame = 0
  }

  // reduced motion = เฟรมนิ่ง: ไม่หมุน ไม่เอียง ไม่ตอบสนองเมาส์ วาดใหม่เฉพาะตอน resize/เปลี่ยนธีม
  function applyMotionMode() {
    if (reducedMotion) {
      stop()
      pointerActive = false
      hoveredPair = -1
      paintInstances()
      tilt.rotation.set(0, 0, 0)
      render()
    } else {
      start()
    }
  }

  const onContextLost = (event: Event) => {
    event.preventDefault()
    stop()
  }
  canvas.addEventListener('webglcontextlost', onContextLost)

  applyPalette(options.palette)
  applyMotionMode()

  return {
    setPalette(next) {
      applyPalette(next)
      if (reducedMotion || !active) render()
    },
    setPointer(ndcX, ndcY) {
      if (reducedMotion) return
      pointerNdc.set(ndcX, ndcY)
      pointerActive = true
      pointerDirty = true
    },
    clearPointer() {
      pointerActive = false
      pointerDirty = true
    },
    impulse() {
      if (reducedMotion) return
      impulse = Math.min(impulse + 3, MAX_IMPULSE)
    },
    resize(width, height) {
      if (width <= 0 || height <= 0) return
      renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, DPR_CAP))
      renderer.setSize(width, height, false)
      camera.aspect = width / height
      camera.updateProjectionMatrix()

      // ขนาดพื้นที่ที่มองเห็นที่ z = 0
      // - แผงแนวตั้ง (ซ้ายของจอกว้าง): เกลียวใหญ่ราว 40% ของความกว้างแผง (เหมือนภาพอ้างอิง) พาดตามเส้นทแยงซ้ายบน→ขวาล่าง
      //   ชื่อ my-his มุมซ้ายล่างจึงว่าง
      // - แผงแนวนอน (แถบบนของมือถือ): เกลียวเล็กลงและเกือบนอน อยู่ครึ่งบนของแถบ ไม่ทับชื่อ/คำโปรยที่ชิดล่าง
      const visibleH = 2 * Math.tan(((CAMERA_FOV / 2) * Math.PI) / 180) * CAMERA_DISTANCE
      const visibleW = visibleH * camera.aspect
      const portrait = camera.aspect < 1
      const radius = portrait ? visibleW * 0.2 : visibleH * 0.15
      const length = Math.hypot(visibleW, visibleH) * 1.25
      diag.rotation.z = portrait ? -Math.atan2(visibleH, visibleW) : -0.2
      diag.position.y = portrait ? 0 : visibleH * 0.2
      if (!built || Math.abs(built.radius - radius) / radius > 0.02 || !built.pairs.length) {
        build(radius, length)
      }
      render()
    },
    setReducedMotion(reduced) {
      reducedMotion = reduced
      applyMotionMode()
    },
    setActive(next) {
      active = next
      if (active) start()
      else stop()
    },
    dispose() {
      stop()
      canvas.removeEventListener('webglcontextlost', onContextLost)
      disposeBuilt()
      backboneMaterial.dispose()
      shellBackMaterial.dispose()
      shellFrontMaterial.dispose()
      stickMaterial.dispose()
      capMaterial.dispose()
      coreMaterial.dispose()
      bondMaterial.dispose()
      for (const b of BASES) {
        letterMaterials[b].dispose()
        letterTextures[b].dispose()
      }
      renderer.dispose()
      // คืน WebGL context ทันที — ไม่รอ GC (StrictMode mount 2 รอบ / เข้า-ออกหน้า login ซ้ำ จะสะสม context จนเบราว์เซอร์ตัดทิ้ง)
      renderer.forceContextLoss()
    },
  }
}
