import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useLiveStore } from "@/store/live";

export function PortfolioVitals() {
  const portfolioState = useLiveStore((state) => state.portfolioState);

  const formatCurrency = (value: string | number) => {
    return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(parseFloat(value.toString()));
  }

  return (
    <div className="grid gap-4 sm:grid-cols-2 md:grid-cols-3">
      <Card>
        <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Total Equity</CardTitle></CardHeader>
        <CardContent>
          <div className="text-2xl font-bold">{portfolioState ? formatCurrency(portfolioState.total_value) : '$--.--'}</div>
        </CardContent>
      </Card>
      <Card>
        <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Cash</CardTitle></CardHeader>
        <CardContent>
          <div className="text-2xl font-bold">{portfolioState ? formatCurrency(portfolioState.cash) : '$--.--'}</div>
        </CardContent>
      </Card>
      <Card>
        <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Session P&L</CardTitle></CardHeader>
        <CardContent>
            {/* This is a simplified P&L. A full implementation would track initial capital. */}
          <div className="text-2xl font-bold text-green-500">{portfolioState ? formatCurrency(parseFloat(portfolioState.total_value) - 10000) : '$--.--'}</div>
        </CardContent>
      </Card>
    </div>
  );
}