"use client";
import { Area, AreaChart, CartesianGrid, XAxis, YAxis, Tooltip } from "recharts";
import { ChartContainer, ChartTooltip, ChartTooltipContent } from "@/components/ui/chart";
import { EquityDataPoint } from "@/types/zenith";
import { format } from "date-fns";

// This function calculates the drawdown series from an equity curve.
function calculateDrawdown(data: EquityDataPoint[]) {
  let peak = 0;
  return data.map(point => {
    const equity = parseFloat(point.equity);
    if (equity > peak) {
      peak = equity;
    }
    const drawdown = ((peak - equity) / peak) * 100;
    const date = new Date(point.timestamp);
    return {
      date: isNaN(date.getTime()) ? new Date() : date, // Fallback to current date if invalid
      drawdown: drawdown > 0 ? -drawdown : 0, // Show drawdown as a negative percentage
    };
  });
}

export function DrawdownChart({ data }: { data: EquityDataPoint[] }) {
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

  const chartData = calculateDrawdown(data);

  return (
    <ChartContainer config={{}} className="w-full h-full">
      <AreaChart data={chartData} margin={{ top: 5, right: 20, left: 20, bottom: 5 }}>
        <defs>
            <linearGradient id="colorDrawdown" x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor="var(--color-destructive)" stopOpacity={0.8} />
                <stop offset="95%" stopColor="var(--color-destructive)" stopOpacity={0.1} />
            </linearGradient>
        </defs>
        <CartesianGrid vertical={false} />
        <XAxis 
            dataKey="date" 
            tickFormatter={(tick) => formatDate(tick)}
        />
        <YAxis 
            dataKey="drawdown" 
            tickFormatter={(tick) => `${tick.toFixed(1)}%`}
        />
        <Tooltip 
            content={<ChartTooltipContent 
                formatter={(value) => [`${(value as number).toFixed(2)}%`, 'Drawdown']}
                labelFormatter={(label) => formatDate(label)}
            />}
        />
        <Area
            type="monotone"
            dataKey="drawdown"
            stroke="var(--color-destructive)"
            fill="url(#colorDrawdown)"
        />
      </AreaChart>
    </ChartContainer>
  );
}