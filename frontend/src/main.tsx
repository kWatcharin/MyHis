import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter } from 'react-router'

import { Toaster } from '@/components/ui/sonner'
import { TooltipProvider } from '@/components/ui/tooltip'
import { AuthProvider } from '@/features/auth/AuthContext'
import { RolePreviewProvider } from '@/features/role-preview/RolePreviewContext'
import { ThemeProvider } from '@/features/theme/ThemeContext'

import App from './App.tsx'
import './index.css'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <BrowserRouter>
      <ThemeProvider>
        <AuthProvider>
          <RolePreviewProvider>
            <TooltipProvider>
              <App />
              <Toaster />
            </TooltipProvider>
          </RolePreviewProvider>
        </AuthProvider>
      </ThemeProvider>
    </BrowserRouter>
  </StrictMode>,
)
