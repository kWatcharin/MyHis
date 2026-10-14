import type { ComponentType, SVGProps } from 'react'
import { AlertTriangle, TrendingUp, Users } from 'lucide-react'
import { Bar, BarChart, CartesianGrid, XAxis } from 'recharts'

import { PageHeader } from '@/components/layout/PageHeader'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { ChartContainer, ChartTooltip, ChartTooltipContent } from '@/components/ui/chart'
import type { ChartConfig } from '@/components/ui/chart'
import { useAuth } from '@/features/auth/AuthContext'
import { mockShiftChartData } from '@/mocks/fixtures'

const chartConfig = {
  morning: { label: 'เวรเช้า', color: 'var(--chart-1)' },
  afternoon: { label: 'เวรบ่าย', color: 'var(--chart-2)' },
  night: { label: 'เวรดึก', color: 'var(--chart-3)' },
} satisfies ChartConfig

// ตัวเลข mock ล้วน ๆ (ไม่ใช่ข้อมูลผู้ป่วย/ข้อมูลจริงใด ๆ — แค่ตัวเลขสรุปการเข้าเวรระดับภาพรวม
// ตามขอบเขตข้อมูลของ CLAUDE.md) แสดงเป็นตัวอย่างว่าหน้า dashboard จริงจะมีโครงหน้าตาแบบนี้
const stats: {
  label: string
  value: string
  hint: string
  icon: ComponentType<SVGProps<SVGSVGElement>>
}[] = [
  { label: 'พยาบาลเข้าเวรวันนี้', value: '12', hint: 'จากทั้งหมด 15 คน', icon: Users },
  { label: 'เวรที่ยังไม่มีคนคุม', value: '2', hint: 'ในสัปดาห์นี้', icon: AlertTriangle },
  { label: 'อัตราเข้าเวรเฉลี่ย', value: '94%', hint: '7 วันล่าสุด', icon: TrendingUp },
]

// กราฟ + stat card mockup ล้วน ๆ ตัวเลขคงที่จาก mocks/fixtures.ts ไม่ได้ผูกกับข้อมูลจริง เพราะยังไม่มี
// ฟีเจอร์ตารางเวรจริงใน Milestone นี้ (auth/สิทธิ์เท่านั้น) — รอสลับเป็นของจริงตอนฟีเจอร์นั้นมาถึง
export function DashboardPage() {
  const { user } = useAuth()

  return (
    <div className="flex flex-col gap-6">
      {/* ชื่อหน้ามาจากการตั้งค่าเพจเหมือนหน้าอื่น — คำทักทายย้ายมาอยู่ในคำอธิบาย */}
      <PageHeader
        fallbackTitle="Dashboard"
        description={`สวัสดี ${user?.displayName ?? user?.username ?? ''} — ภาพรวมคร่าว ๆ (mockup ยังไม่ผูกกับข้อมูลจริง)`}
      />

      <div className="grid gap-4 sm:grid-cols-3">
        {stats.map(({ label, value, hint, icon: Icon }) => (
          <Card key={label}>
            <CardContent className="flex items-start justify-between gap-3">
              <div className="flex flex-col gap-1">
                <p className="text-sm text-muted-foreground">{label}</p>
                <p className="text-2xl font-semibold tracking-tight">{value}</p>
                <p className="text-xs text-muted-foreground">{hint}</p>
              </div>
              <div className="rounded-lg bg-primary/10 p-2 text-primary">
                <Icon className="size-4" />
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      <Card>
        <CardHeader>
          <CardTitle>จำนวนคนต่อเวรในสัปดาห์นี้</CardTitle>
          <CardDescription>mockup — ยังไม่ผูกกับข้อมูลจริง</CardDescription>
        </CardHeader>
        <CardContent>
          <ChartContainer config={chartConfig} className="max-h-[300px] w-full">
            <BarChart data={mockShiftChartData}>
              <CartesianGrid vertical={false} />
              <XAxis dataKey="day" tickLine={false} axisLine={false} />
              <ChartTooltip content={<ChartTooltipContent />} />
              <Bar dataKey="morning" fill="var(--color-morning)" radius={4} />
              <Bar dataKey="afternoon" fill="var(--color-afternoon)" radius={4} />
              <Bar dataKey="night" fill="var(--color-night)" radius={4} />
            </BarChart>
          </ChartContainer>
        </CardContent>
      </Card>
    </div>
  )
}
