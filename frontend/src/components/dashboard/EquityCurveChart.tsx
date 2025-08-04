"use client";
import { Area, AreaChart, CartesianGrid, XAxis, YAxis, Tooltip } from "recharts";
import { ChartContainer, ChartTooltip, ChartTooltipContent } from "@/components/ui/chart";
import { EquityDataPoint } from "@/types/zenith";
import { format } from "date-fns";

export function EquityCurveChart({ data }: { data: EquityDataPoint[] }) {
    const chartData = data.map(d => ({
        date: new Date(d.timestamp),
        equity: parseFloat(d.equity),
    }));

    return (
        <ChartContainer config={{}} className="w-full h-full">
            <AreaChart data={chartData}>
                <defs>
                    <linearGradient id="colorEquity" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="5%" stopColor="var(--color-equity)" stopOpacity={0.8} />
                        <stop offset="95%" stopColor="var(--color-equity)" stopOpacity={0.1} />
                    </linearGradient>
                </defs>
                <CartesianGrid vertical={false} />
                <XAxis 
                    dataKey="date" 
                    tickFormatter={(tick) => format(tick, 'yyyy-MM-dd')}
                    />
                <YAxis dataKey="equity" domain={['auto', 'auto']} />
                <Tooltip 
                    content={<ChartTooltipContent 
                        formatter={(value, name) => [`$${(value as number).toFixed(2)}`, 'Equity']}
                        labelFormatter={(label) => format(label, 'yyyy-MM-dd HH:mm')}
                        />}
                    />
                <Area
                    type="monotone"
                    dataKey="equity"
                    stroke="var(--color-equity)"
                    fill="url(#colorEquity)"
                />
            </AreaChart>
        </ChartContainer>
    );
}