"use client";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { RankedReport, FullReport } from "@/types/zenith";
import Link from "next/link";
import { format } from "date-fns";
import { Badge } from "@/components/ui/badge";

export function SingleRunsTable({ data }: { data: (RankedReport | FullReport)[] }) {
  const formatDate = (dateString: string) => {
    try {
      const date = new Date(dateString);
      if (isNaN(date.getTime())) {
        return "Invalid Date";
      }
      return format(date, "yyyy-MM-dd HH:mm");
    } catch (error) {
      return "Invalid Date";
    }
  };

  const getRunData = (item: RankedReport | FullReport): FullReport | null => {
    // Check if it's a RankedReport (has report property)
    if ('report' in item && item.report) {
      return item.report;
    }
    // Check if it's a FullReport directly
    if ('run_id' in item && 'total_return_pct' in item) {
      return item as FullReport;
    }
    return null;
  };

  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Run ID</TableHead>
          <TableHead>Run Date</TableHead>
          <TableHead>Net Profit %</TableHead>
          <TableHead>Max Drawdown %</TableHead>
          <TableHead>Profit Factor</TableHead>
          <TableHead>Trades</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {data.map((item, index) => {
          const run = getRunData(item);
          
          if (!run) {
            console.warn(`Invalid data item at index ${index}:`, item);
            return null;
          }

          return (
            <TableRow key={run.run_id}>
              <TableCell>
                <Link href={`/admin/runs/${run.run_id}`} className="text-primary hover:underline font-mono">
                  {run.run_id.substring(0, 8)}...
                </Link>
              </TableCell>
              <TableCell>{formatDate(run.report_id)}</TableCell>
              <TableCell>
                  <Badge variant={parseFloat(run.total_return_pct) >= 0 ? "default" : "destructive"}>
                      {parseFloat(run.total_return_pct).toFixed(2)}%
                  </Badge>
              </TableCell>
              <TableCell>{parseFloat(run.max_drawdown_pct).toFixed(2)}%</TableCell>
              <TableCell>{parseFloat(run.profit_factor || '0').toFixed(2)}</TableCell>
              <TableCell>{run.total_trades}</TableCell>
            </TableRow>
          );
        })}
      </TableBody>
    </Table>
  );
}