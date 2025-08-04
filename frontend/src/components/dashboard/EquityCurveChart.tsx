"use client";
import { Area, AreaChart, CartesianGrid, XAxis, YAxis, Tooltip } from "recharts";
import { ChartContainer, ChartTooltip, ChartTooltipContent } from "@/components/ui/chart";
import { EquityDataPoint } from "@/types/zenith";
import { format } from "date-fns";

export function EquityCurveChart({ data }: { data: EquityDataPoint[] }) {
    const formatDate = (date: Date | string) => {
        try {
            const dateObj = typeof date === 'string' ? new Date(date) : date;
            if (isNaN(dateObj.getTime())) {
                return "Invalid Date";
            }
            return format(dateObj, 'yyyy-MM-dd HH:mm');
        } catch (error) {
            return "Invalid Date";
        }
    };

    const chartData = data.map(d => {
        const date = new Date(d.timestamp);
        return {
            date: isNaN(date.getTime()) ? new Date() : date, // Fallback to current date if invalid
            equity: parseFloat(d.equity),
        };
    });

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
                    tickFormatter={(tick) => formatDate(tick)}
                    />
                <YAxis dataKey="equity" domain={['auto', 'auto']} />
                <Tooltip 
                    content={<ChartTooltipContent 
                        formatter={(value, name) => [`$${(value as number).toFixed(2)}`, 'Equity']}
                        labelFormatter={(label) => formatDate(label)}
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