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
import { ActivityHistoryPage } from '@/features/audit/ActivityHistoryPage'
import { AuthHistoryPage } from '@/features/audit/AuthHistoryPage'
import { RequireAuth } from '@/features/auth/RequireAuth'
import { LoginPage } from '@/features/auth/LoginPage'
import { DashboardPage } from '@/features/dashboard/DashboardPage'
import { NotFoundPage } from '@/features/errors/NotFoundPage'
import { LabVisitGroupDetailPage } from '@/features/lab-visits/LabVisitGroupDetailPage'
import { LabVisitGroupsPage } from '@/features/lab-visits/LabVisitGroupsPage'
import { LabVisitPrintPage } from '@/features/lab-visits/LabVisitPrintPage'
import { PatientsPage } from '@/features/patients/PatientsPage'
import { RegisterPage } from '@/features/registration/RegisterPage'
import { SqlConsolePage } from '@/features/sql-console/SqlConsolePage'
import { UsersPage } from '@/features/users/UsersPage'

// หน้าตั้งค่า master ที่สร้างจาก MasterSettingsPage + config (ดู features/admin/master/) — เข้าหน้าได้ด้วยสิทธิ์
// "ดู" ของหน้านั้น (path ต้องตรงกับ route ของหน้าที่ seed ใน migration)
const MASTER_SETTINGS_ROUTES = [
  { path: '/settings/prenames', permission: 'prename.view', element: <PrenamesSettingsPage /> },
  { path: '/settings/nationalities', permission: 'nationality.view', element: <NationalitiesSettingsPage /> },
  { path: '/settings/patient-groups', permission: 'patient_group.view', element: <PatientGroupsSettingsPage /> },
  { path: '/settings/labs', permission: 'lab.view', element: <LabsSettingsPage /> },
]

function App() {
  return (
    <Routes>
      <Route path="/login" element={<LoginPage />} />
      {/* ลงทะเบียน/ตั้งรหัสผ่านใหม่จากลิงก์ที่แอดมินออกให้ (/register#<token>) — สาธารณะ ไม่ต้อง login */}
      <Route path="/register" element={<RegisterPage />} />
      {/* หน้าเอกสารพร้อมพิมพ์ (ส่งออก PDF) — นอกโครงหน้าหลัก (ไม่มี sidebar) แต่ยังต้อง login + สิทธิ์ส่งออก (lab_visit.export) */}
      <Route
        path="/lab/appointments/:id/print"
        element={
          <RequireAuth permission="lab_visit.export">
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
            <RequireAuth permission="patient.view">
              <PatientsPage />
            </RequireAuth>
          }
        />
        <Route
          path="/lab/appointments"
          element={
            <RequireAuth permission="lab_visit.view">
              <LabVisitGroupsPage />
            </RequireAuth>
          }
        />
        <Route
          path="/lab/appointments/:id"
          element={
            <RequireAuth permission="lab_visit.view">
              <LabVisitGroupDetailPage />
            </RequireAuth>
          }
        />
        {/* จัดการบัญชีผู้ใช้ (กลุ่มเมนู "ลงทะเบียน") — API /users ต้อง user.view (แก้/สร้างตรวจราย action) */}
        <Route
          path="/users"
          element={
            <RequireAuth permission="user.view">
              <UsersPage />
            </RequireAuth>
          }
        />
        <Route
          path="/settings/roles"
          element={
            <RequireAuth permission="role.view">
              <RolesSettingsPage />
            </RequireAuth>
          }
        />
        <Route
          path="/settings/pages"
          element={
            <RequireAuth permission="page.view">
              <PagesSettingsPage />
            </RequireAuth>
          }
        />
        <Route
          path="/settings/budget-years"
          element={
            <RequireAuth permission="budget_year.view">
              <BudgetYearsSettingsPage />
            </RequireAuth>
          }
        />
        <Route
          path="/settings/diseases"
          element={
            <RequireAuth permission="disease.view">
              <DiseasesSettingsPage />
            </RequireAuth>
          }
        />
        {MASTER_SETTINGS_ROUTES.map(({ path, permission, element }) => (
          <Route
            key={path}
            path={path}
            element={<RequireAuth permission={permission}>{element}</RequireAuth>}
          />
        ))}
        {/* ประวัติการเข้าออก (เพจย่อยของ "ประวัติ") — login/logout ของผู้ใช้ทุกคน */}
        <Route
          path="/audit/auth"
          element={
            <RequireAuth permission="audit.view">
              <AuthHistoryPage />
            </RequireAuth>
          }
        />
        {/* ประวัติการใช้งาน (เพจย่อยของ "ประวัติ") — เพิ่ม/แก้ไข/ลบ/ส่งออก ของทุกหน้า */}
        <Route
          path="/audit/activities"
          element={
            <RequireAuth permission="audit.view">
              <ActivityHistoryPage />
            </RequireAuth>
          }
        />
        {/* หน้า SQL (เพจ "SQL" ใต้กลุ่ม "ข้อมูล") — รัน SQL อ่านอย่างเดียว ทุกครั้งถูกบันทึกในประวัติการใช้งาน */}
        <Route
          path="/data/script"
          element={
            <RequireAuth permission="sql.view">
              <SqlConsolePage />
            </RequireAuth>
          }
        />
        {/* path ที่ไม่มีอยู่จริง: แสดงหน้า not found ในโครงหน้าหลัก พร้อมปุ่มกลับ Dashboard */}
        <Route path="*" element={<NotFoundPage />} />
      </Route>
      <Route path="/" element={<Navigate to="/dashboard" replace />} />
    </Routes>
  )
}

export default App
