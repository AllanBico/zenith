"use client";
import { Bar, BarChart, CartesianGrid, XAxis, YAxis, Tooltip } from "recharts";
import { ChartContainer, ChartTooltip, ChartTooltipContent } from "@/components/ui/chart";
import { Trade } from "@/types/zenith";

// CORRECTED: This function now correctly calculates P&L for both long and short trades.
function calculatePnl(trade: Trade): number {
    const entryPrice = parseFloat(trade.entry_execution.price);
    const exitPrice = parseFloat(trade.exit_execution.price);
    const qty = parseFloat(trade.entry_execution.quantity);

    if (trade.entry_execution.side === "Buy") { // Assuming side is "Buy" for long, "Sell" for short
        return (exitPrice - entryPrice) * qty;
    } else {
        return (entryPrice - exitPrice) * qty;
    }
}

export function PnlPerTradeChart({ data }: { data: Trade[] }) {
  const chartData = data.map((trade, index) => {
    const pnl = calculatePnl(trade);
    return {
      trade: index + 1,
      pnl,
      fill: pnl >= 0 ? 'var(--color-positive)' : 'var(--color-destructive)',
    };
  });

  return (
    <ChartContainer config={{}} className="w-full h-full">
      <BarChart data={chartData} margin={{ top: 5, right: 20, left: 20, bottom: 5 }}>
        <CartesianGrid vertical={false} />
        <XAxis dataKey="trade" name="Trade #" />
        <YAxis tickFormatter={(tick) => `$${tick}`} />
        <Tooltip 
            cursor={{ fill: 'hsl(var(--muted))' }}
            content={<ChartTooltipContent 
                formatter={(value) => [`$${(value as number).toFixed(2)}`, 'P&L']}
                labelFormatter={(label) => `Trade #${label}`}
            />}
        />
        <Bar dataKey="pnl" fill="var(--color-fill)" radius={2} />
      </BarChart>
    </ChartContainer>
  );
}