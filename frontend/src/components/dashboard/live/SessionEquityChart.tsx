"use client";
import { Area, AreaChart, CartesianGrid, XAxis, YAxis, Tooltip, ResponsiveContainer, ReferenceLine } from "recharts";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ChartContainer, ChartTooltip, ChartTooltipContent } from "@/components/ui/chart";
import { useLiveStore } from "@/store/live";
import { PortfolioState } from "@/types/zenith";
import { format } from "date-fns";
import { useEffect, useState } from "react";

const INITIAL_CAPITAL = 10000;

export function SessionEquityChart() {
  const latestState = useLiveStore((state) => state.portfolioState);
  const [history, setHistory] = useState<PortfolioState[]>([]);

  // Accumulate a history of portfolio states for the chart
  useEffect(() => {
    if (latestState) {
      setHistory(prev => [...prev, latestState]);
    }
  }, [latestState]);

  const chartData = history.map(d => {
    try {
      return {
        date: new Date(d.timestamp),
        equity: parseFloat(d.total_value),
      };
    } catch (error) {
      console.warn('Invalid date in portfolio state:', d.timestamp);
      return {
        date: new Date(),
        equity: parseFloat(d.total_value),
      };
    }
  });

  return (
    <Card className="h-full flex flex-col overflow-hidden">
        <CardHeader><CardTitle>Session Equity</CardTitle></CardHeader>
        <CardContent className="flex-1 min-h-0 overflow-hidden">
            <ChartContainer config={{}} className="w-full h-full min-h-0">
                <ResponsiveContainer width="100%" height="100%">
                    <AreaChart data={chartData}>
                    <defs>
                        <linearGradient id="colorEquity" x1="0" y1="0" x2="0" y2="1">
                            <stop offset="5%" stopColor="var(--color-equity)" stopOpacity={0.8} />
                            <stop offset="95%" stopColor="var(--color-equity)" stopOpacity={0.1} />
                        </linearGradient>
                    </defs>
                    <CartesianGrid vertical={false} strokeDasharray="3 3" />
                    <XAxis 
                        dataKey="date" 
                        tickFormatter={(tick) => {
                          try {
                            return format(tick, 'HH:mm:ss');
                          } catch (error) {
                            return 'Invalid';
                          }
                        }}
                        />
                    <YAxis dataKey="equity" domain={['auto', 'auto']} tickFormatter={(tick) => `$${tick.toLocaleString()}`} />
                    <Tooltip 
                        content={<ChartTooltipContent 
                            formatter={(value) => [`$${(value as number).toFixed(2)}`, 'Equity']}
                            labelFormatter={(label) => {
                              try {
                                return format(label, 'HH:mm:ss');
                              } catch (error) {
                                return 'Invalid';
                              }
                            }}
                        />}
                        />
                    <ReferenceLine y={INITIAL_CAPITAL} label="Start" stroke="hsl(var(--muted-foreground))" strokeDasharray="3 3" />
                    <Area type="monotone" dataKey="equity" stroke="var(--color-equity)" fill="url(#colorEquity)" />
                </AreaChart>
                </ResponsiveContainer>
            </ChartContainer>
        </CardContent>
    </Card>
  );
}