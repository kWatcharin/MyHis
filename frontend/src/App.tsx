import { Navigate, Route, Routes } from 'react-router'

import { AppShell } from '@/components/layout/AppShell'
import { PagesSettingsPage } from '@/features/admin/PagesSettingsPage'
import { RolesSettingsPage } from '@/features/admin/RolesSettingsPage'
import { RequireAuth } from '@/features/auth/RequireAuth'
import { LoginPage } from '@/features/auth/LoginPage'
import { DashboardPage } from '@/features/dashboard/DashboardPage'

function App() {
  return (
    <Routes>
      <Route path="/login" element={<LoginPage />} />
      <Route
        element={
          <RequireAuth>
            <AppShell />
          </RequireAuth>
        }
      >
        <Route path="/dashboard" element={<DashboardPage />} />
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
      </Route>
      <Route path="/" element={<Navigate to="/dashboard" replace />} />
      <Route path="*" element={<Navigate to="/dashboard" replace />} />
    </Routes>
  )
}

export default App
