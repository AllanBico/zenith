"use client";
import { use } from "react";
import { useRunDetails } from "@/hooks/useZenithApi";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { EquityCurveChart } from "@/components/dashboard/EquityCurveChart";
import { DrawdownChart } from "@/components/dashboard/DrawdownChart";
import { PnlPerTradeChart } from "@/components/dashboard/PnlPerTradeChart";
import { FullMetricsTable } from "@/components/dashboard/FullMetricsTable";
import { TradeHistoryTable } from "@/components/dashboard/TradeHistoryTable"; // <-- IMPORT
import { KpiCard } from "@/components/dashboard/KpiCard";

export default function RunDetailsPage({ params }: { params: Promise<{ runId: string }> }) {
  const { runId } = use(params);
  const { data: details, isLoading, error } = useRunDetails(runId);

  if (isLoading) return <div className="text-center p-8">Loading run details...</div>;
  if (error) return <div className="text-red-500 text-center p-8">Error: {error.message}</div>;
  if (!details) return <div className="text-center p-8">No data found for this run.</div>;
  
  const { report, equity_curve, trades } = details;

  return (
    <div className="space-y-4">
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-5">
          <KpiCard title="Total Return" value={`${parseFloat(report.total_return_pct).toFixed(2)}%`} />
          <KpiCard title="Max Drawdown" value={`${parseFloat(report.max_drawdown_pct).toFixed(2)}%`} isNegative />
          <KpiCard title="Profit Factor" value={parseFloat(report.profit_factor || '0').toFixed(2)} />
          <KpiCard title="Calmar Ratio" value={parseFloat(report.calmar_ratio || '0').toFixed(2)} />
          <KpiCard title="Total Trades" value={report.total_trades.toString()} />
      </div>
      
      <Tabs defaultValue="overview">
        <TabsList className="grid w-full grid-cols-3">
          <TabsTrigger value="overview">Overview</TabsTrigger>
          <TabsTrigger value="all-metrics">All Metrics</TabsTrigger>
          <TabsTrigger value="trade-history">Trade History</TabsTrigger>
        </TabsList>

        <TabsContent value="overview" className="space-y-4 mt-4">
          <Card>
            <CardHeader><CardTitle>Equity Curve</CardTitle></CardHeader>
            <CardContent className="h-[350px]"><EquityCurveChart data={equity_curve} /></CardContent>
          </Card>
          <div className="grid gap-4 md:grid-cols-2">
            <Card>
              <CardHeader><CardTitle>Drawdown Profile</CardTitle></CardHeader>
              <CardContent className="h-[250px]"><DrawdownChart data={equity_curve} /></CardContent>
            </Card>
            <Card>
              <CardHeader><CardTitle>P&L per Trade</CardTitle></CardHeader>
              <CardContent className="h-[250px]"><PnlPerTradeChart data={trades} /></CardContent>
            </Card>
          </div>
        </TabsContent>

        <TabsContent value="all-metrics" className="mt-4">
            <FullMetricsTable report={report} />
        </TabsContent>

        <TabsContent value="trade-history" className="mt-4">
            <Card>
              <CardHeader><CardTitle>All Trades ({trades.length})</CardTitle></CardHeader>
              <CardContent>
                  {/* --- POPULATE THE TAB --- */}
                  <TradeHistoryTable data={trades} />
              </CardContent>
            </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}