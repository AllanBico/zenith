"use client";

import { useEffect, useState } from "react";
import { getSingleRuns } from "@/services/api";
import { RankedReport } from "@/types/zenith";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";

export default function SingleRunsPage() {
    const [runs, setRuns] = useState<RankedReport[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        const fetchRuns = async () => {
            try {
                const data = await getSingleRuns();
                setRuns(data);
            } catch (err) {
                setError(err instanceof Error ? err.message : "Failed to fetch runs");
            } finally {
                setLoading(false);
            }
        };

        fetchRuns();
    }, []);

    if (loading) {
        return (
            <div className="p-6">
                <h1 className="text-2xl font-bold mb-6">Single Runs</h1>
                <div className="flex items-center justify-center h-64">
                    <p>Loading...</p>
                </div>
            </div>
        );
    }

    if (error) {
        return (
            <div className="p-6">
                <h1 className="text-2xl font-bold mb-6">Single Runs</h1>
                <Card>
                    <CardContent className="p-6">
                        <p className="text-red-600">Error: {error}</p>
                    </CardContent>
                </Card>
            </div>
        );
    }

    return (
        <div className="p-6">
            <h1 className="text-2xl font-bold mb-6">Single Runs</h1>
            
            {runs.length === 0 ? (
                <Card>
                    <CardContent className="p-6">
                        <p className="text-muted-foreground">No single runs found.</p>
                    </CardContent>
                </Card>
            ) : (
                <Card>
                    <CardHeader>
                        <CardTitle>Backtest Runs</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <TableHead>Run ID</TableHead>
                                    <TableHead>Job ID</TableHead>
                                    <TableHead>Total Return</TableHead>
                                    <TableHead>Max Drawdown</TableHead>
                                    <TableHead>Sharpe Ratio</TableHead>
                                    <TableHead>Total Trades</TableHead>
                                    <TableHead>Win Rate</TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {runs.map((run) => (
                                    <TableRow key={run.run_id}>
                                        <TableCell className="font-mono text-sm">
                                            {run.run_id.slice(0, 8)}...
                                        </TableCell>
                                        <TableCell className="font-mono text-sm">
                                            {run.job_id.slice(0, 8)}...
                                        </TableCell>
                                        <TableCell>
                                            <Badge variant={run.total_return_pct && !isNaN(parseFloat(run.total_return_pct)) && parseFloat(run.total_return_pct) > 0 ? "default" : "destructive"}>
                                                {run.total_return_pct && !isNaN(parseFloat(run.total_return_pct)) ? `${parseFloat(run.total_return_pct).toFixed(2)}%` : "N/A"}
                                            </Badge>
                                        </TableCell>
                                        <TableCell>
                                            <Badge variant="outline">
                                                {run.max_drawdown_pct && !isNaN(parseFloat(run.max_drawdown_pct)) ? `${parseFloat(run.max_drawdown_pct).toFixed(2)}%` : "N/A"}
                                            </Badge>
                                        </TableCell>
                                        <TableCell>
                                            {run.sharpe_ratio && !isNaN(parseFloat(run.sharpe_ratio)) ? parseFloat(run.sharpe_ratio).toFixed(2) : "N/A"}
                                        </TableCell>
                                        <TableCell>
                                            {run.total_trades || "N/A"}
                                        </TableCell>
                                        <TableCell>
                                            <Badge variant={run.win_rate_pct && !isNaN(parseFloat(run.win_rate_pct)) && parseFloat(run.win_rate_pct) > 50 ? "default" : "secondary"}>
                                                {run.win_rate_pct && !isNaN(parseFloat(run.win_rate_pct)) ? `${parseFloat(run.win_rate_pct).toFixed(1)}%` : "N/A"}
                                            </Badge>
                                        </TableCell>
                                    </TableRow>
                                ))}
                            </TableBody>
                        </Table>
                    </CardContent>
                </Card>
            )}
        </div>
    );
} 