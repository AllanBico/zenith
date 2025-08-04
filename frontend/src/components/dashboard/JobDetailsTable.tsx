"use client";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { RankedReport } from "@/types/zenith";
import Link from "next/link";

export function JobDetailsTable({ data }: { data: RankedReport[] }) {
    // Sort by score descending
    const sortedData = [...data].sort((a, b) => parseFloat(b.score) - parseFloat(a.score));

    return (
        <Table>
            <TableHeader>
                <TableRow>
                    <TableHead>Rank</TableHead>
                    <TableHead>Score</TableHead>
                    <TableHead>Net Profit %</TableHead>
                    <TableHead>Drawdown %</TableHead>
                    <TableHead>Trades</TableHead>
                    <TableHead>Parameters</TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {sortedData.map((item, index) => (
                    <TableRow key={item.report.run_id}>
                         <TableCell>
                            <Link href={`/admin/runs/${item.report.run_id}`} className="text-primary hover:underline">
                                #{index + 1}
                            </Link>
                        </TableCell>
                        <TableCell>{parseFloat(item.score).toFixed(4)}</TableCell>
                        <TableCell>{parseFloat(item.report.total_return_pct).toFixed(2)}%</TableCell>
                        <TableCell>{parseFloat(item.report.max_drawdown_pct).toFixed(2)}%</TableCell>
                        <TableCell>{item.report.total_trades}</TableCell>
                        <TableCell><pre className="text-xs bg-muted p-2 rounded">{JSON.stringify(item.parameters, null, 2)}</pre></TableCell>
                    </TableRow>
                ))}
            </TableBody>
        </Table>
    );
}