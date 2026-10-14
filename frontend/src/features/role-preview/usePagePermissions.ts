import { useRolePreview } from './RolePreviewContext'

// สิทธิ์ของผู้ใช้ (หรือ role ที่กำลัง preview) ต่อเพจหนึ่ง เช่น usePagePermissions('patient') — ใช้ซ่อน/แสดงปุ่ม
// เพิ่ม/แก้ไข/ลบ/ส่งออก เป็นแค่ความสะดวกของหน้าเว็บ backend ตรวจสิทธิ์ราย action ซ้ำทุกครั้ง
export function usePagePermissions(resource: string) {
  const { hasEffectivePermission } = useRolePreview()
  return {
    canView: hasEffectivePermission(`${resource}.view`),
    canCreate: hasEffectivePermission(`${resource}.create`),
    canUpdate: hasEffectivePermission(`${resource}.update`),
    canDelete: hasEffectivePermission(`${resource}.delete`),
    canExport: hasEffectivePermission(`${resource}.export`),
  }
}
