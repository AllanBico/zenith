import { FullReport } from "@/types/zenith";
import { Card, CardContent } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableRow } from "@/components/ui/table";

export function FullMetricsTable({ report }: { report: FullReport }) {
    const metrics = [
        { label: "Total Net Profit", value: parseFloat(report.total_net_profit).toFixed(2) },
        { label: "Total Return %", value: `${parseFloat(report.total_return_pct).toFixed(2)}%` },
        { label: "Max Drawdown %", value: `${parseFloat(report.max_drawdown_pct).toFixed(2)}%` },
        { label: "Profit Factor", value: parseFloat(report.profit_factor || '0').toFixed(2) },
        { label: "Calmar Ratio", value: parseFloat(report.calmar_ratio || '0').toFixed(2) },
        { label: "Sharpe Ratio", value: parseFloat(report.sharpe_ratio || '0').toFixed(2) },
        { label: "Total Trades", value: report.total_trades },
        { label: "Win Rate %", value: `${parseFloat(report.win_rate_pct || '0').toFixed(2)}%` },
        { label: "Payoff Ratio", value: parseFloat(report.payoff_ratio || '0').toFixed(2) },
        { label: "Average Win", value: parseFloat(report.average_win).toFixed(2) },
        { label: "Average Loss", value: parseFloat(report.average_loss).toFixed(2) },
        // Add more metrics as desired
    ];

    return (
        <Card>
            <CardContent className="pt-6">
                <Table>
                    <TableBody>
                        {metrics.map(metric => (
                            <TableRow key={metric.label}>
                                <TableCell className="font-medium">{metric.label}</TableCell>
                                <TableCell className="text-right">{metric.value}</TableCell>
                            </TableRow>
                        ))}
                    </TableBody>
                </Table>
            </CardContent>
        </Card>
    );
}