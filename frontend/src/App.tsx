import { Navigate, Route, Routes } from 'react-router'

import { AppShell } from '@/components/layout/AppShell'
import { BudgetYearsSettingsPage } from '@/features/admin/BudgetYearsSettingsPage'
import { DiseasesSettingsPage } from '@/features/admin/DiseasesSettingsPage'
import { LabsSettingsPage } from '@/features/admin/LabsSettingsPage'
import { NationalitiesSettingsPage } from '@/features/admin/NationalitiesSettingsPage'
import { PagesSettingsPage } from '@/features/admin/PagesSettingsPage'
import { PatientGroupsSettingsPage } from '@/features/admin/PatientGroupsSettingsPage'
import { PrenamesSettingsPage } from '@/features/admin/PrenamesSettingsPage'
import { RolesSettingsPage } from '@/features/admin/RolesSettingsPage'
import { RequireAuth } from '@/features/auth/RequireAuth'
import { LoginPage } from '@/features/auth/LoginPage'
import { DashboardPage } from '@/features/dashboard/DashboardPage'
import { NotFoundPage } from '@/features/errors/NotFoundPage'
import { LabVisitGroupDetailPage } from '@/features/lab-visits/LabVisitGroupDetailPage'
import { LabVisitGroupsPage } from '@/features/lab-visits/LabVisitGroupsPage'
import { LabVisitPrintPage } from '@/features/lab-visits/LabVisitPrintPage'
import { PatientsPage } from '@/features/patients/PatientsPage'
import { RegisterPage } from '@/features/registration/RegisterPage'
import { UsersPage } from '@/features/users/UsersPage'

// หน้าตั้งค่า master ที่สร้างจาก MasterSettingsPage + config (ดู features/admin/master/) — ทุกหน้าต้อง
// มี user.manage เหมือนหน้าตั้งค่าอื่น ๆ (path ต้องตรงกับ route ของหน้าที่ seed ใน migration)
const MASTER_SETTINGS_ROUTES = [
  { path: '/settings/prenames', element: <PrenamesSettingsPage /> },
  { path: '/settings/nationalities', element: <NationalitiesSettingsPage /> },
  { path: '/settings/patient-groups', element: <PatientGroupsSettingsPage /> },
  { path: '/settings/labs', element: <LabsSettingsPage /> },
]

function App() {
  return (
    <Routes>
      <Route path="/login" element={<LoginPage />} />
      {/* ลงทะเบียน/ตั้งรหัสผ่านใหม่จากลิงก์ที่แอดมินออกให้ (/register#<token>) — สาธารณะ ไม่ต้อง login */}
      <Route path="/register" element={<RegisterPage />} />
      {/* หน้าเอกสารพร้อมพิมพ์ (ส่งออก PDF) — นอกโครงหน้าหลัก (ไม่มี sidebar) แต่ยังต้อง login + patient.read */}
      <Route
        path="/lab/appointments/:id/print"
        element={
          <RequireAuth permission="patient.read">
            <LabVisitPrintPage />
          </RequireAuth>
        }
      />
      <Route
        element={
          <RequireAuth>
            <AppShell />
          </RequireAuth>
        }
      >
        <Route path="/dashboard" element={<DashboardPage />} />
        <Route
          path="/patients"
          element={
            <RequireAuth permission="patient.read">
              <PatientsPage />
            </RequireAuth>
          }
        />
        <Route
          path="/lab/appointments"
          element={
            <RequireAuth permission="patient.read">
              <LabVisitGroupsPage />
            </RequireAuth>
          }
        />
        <Route
          path="/lab/appointments/:id"
          element={
            <RequireAuth permission="patient.read">
              <LabVisitGroupDetailPage />
            </RequireAuth>
          }
        />
        {/* จัดการบัญชีผู้ใช้ (กลุ่มเมนู "ลงทะเบียน") — API /users ต้อง user.manage เหมือนกัน */}
        <Route
          path="/users"
          element={
            <RequireAuth permission="user.manage">
              <UsersPage />
            </RequireAuth>
          }
        />
        <Route
          path="/settings/roles"
          element={
            <RequireAuth permission="user.manage">
              <RolesSettingsPage />
            </RequireAuth>
          }
        />
        <Route
          path="/settings/pages"
          element={
            <RequireAuth permission="user.manage">
              <PagesSettingsPage />
            </RequireAuth>
          }
        />
        <Route
          path="/settings/budget-years"
          element={
            <RequireAuth permission="user.manage">
              <BudgetYearsSettingsPage />
            </RequireAuth>
          }
        />
        <Route
          path="/settings/diseases"
          element={
            <RequireAuth permission="user.manage">
              <DiseasesSettingsPage />
            </RequireAuth>
          }
        />
        {MASTER_SETTINGS_ROUTES.map(({ path, element }) => (
          <Route
            key={path}
            path={path}
            element={<RequireAuth permission="user.manage">{element}</RequireAuth>}
          />
        ))}
        {/* path ที่ไม่มีอยู่จริง: แสดงหน้า not found ในโครงหน้าหลัก พร้อมปุ่มกลับ Dashboard */}
        <Route path="*" element={<NotFoundPage />} />
      </Route>
      <Route path="/" element={<Navigate to="/dashboard" replace />} />
    </Routes>
  )
}

export default App
