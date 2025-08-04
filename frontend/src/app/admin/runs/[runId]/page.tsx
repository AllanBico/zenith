"use client";
import { useRunDetails } from "@/hooks/useZenithApi";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EquityCurveChart } from "@/components/dashboard/EquityCurveChart";
import { KpiCard } from "@/components/dashboard/KpiCard";

export default function RunDetailsPage({ params }: { params: { runId: string } }) {
  const { data: details, isLoading, error } = useRunDetails(params.runId);

  if (isLoading) return <div>Loading run details...</div>;
  if (error) return <div className="text-red-500">Error: {error.message}</div>;
  if (!details) return <div>No data found for this run.</div>;
  
  const { report, equity_curve } = details;

  return (
    <div className="space-y-4">
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
            <KpiCard title="Total Net Profit %" value={`${parseFloat(report.total_return_pct).toFixed(2)}%`} />
            <KpiCard title="Max Drawdown %" value={`${parseFloat(report.max_drawdown_pct).toFixed(2)}%`} isNegative />
            <KpiCard title="Profit Factor" value={parseFloat(report.profit_factor || '0').toFixed(2)} />
            <KpiCard title="Total Trades" value={report.total_trades.toString()} />
        </div>
        <Card>
            <CardHeader>
                <CardTitle>Equity Curve</CardTitle>
            </CardHeader>
            <CardContent className="h-[400px]">
                <EquityCurveChart data={equity_curve} />
            </CardContent>
        </Card>
        {/* We would add the Tabs component with Trade History here */}
    </div>
  );
}