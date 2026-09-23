import { Bar, BarChart, CartesianGrid, XAxis } from 'recharts'

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

// กราฟ mockup ล้วน ๆ ตัวเลขคงที่จาก mocks/fixtures.ts ไม่ได้ผูกกับข้อมูลจริง เพราะยังไม่มีฟีเจอร์
// ตารางเวรจริงใน Milestone นี้ (auth/สิทธิ์เท่านั้น) — รอสลับเป็นของจริงตอนฟีเจอร์นั้นมาถึง
export function DashboardPage() {
  const { user } = useAuth()

  return (
    <div className="flex flex-col gap-4">
      <div>
        <h1 className="text-lg font-medium">สวัสดี {user?.displayName ?? user?.username}</h1>
        <p className="text-sm text-muted-foreground">
          ภาพรวมคร่าว ๆ — mockup, ยังไม่ผูกกับข้อมูลจริง
        </p>
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
