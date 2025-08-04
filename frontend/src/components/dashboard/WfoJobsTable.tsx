"use client";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { WfoJob } from "@/types/zenith"; // We need to add WfoJob to types
import { Badge } from "@/components/ui/badge";
import Link from "next/link";
import { format } from "date-fns";

function formatTimestamp(timestamp: string): string {
    try {
        const date = new Date(timestamp);
        if (isNaN(date.getTime())) {
            return "Invalid Date";
        }
        return format(date, "yyyy-MM-dd HH:mm");
    } catch (error) {
        return "Invalid Date";
    }
}

export function WfoJobsTable({ data }: { data: WfoJob[] }) {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>WFO Job ID</TableHead>
          <TableHead>Strategy</TableHead>
          <TableHead>Symbol</TableHead>
          <TableHead>IS / OOS Period</TableHead>
          <TableHead>Status</TableHead>
          <TableHead>Created At</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {data.map((job) => (
          <TableRow key={job.wfo_job_id}>
            <TableCell>
              <Link href={`/admin/wfo-jobs/${job.wfo_job_id}`} className="text-primary hover:underline font-mono">
                {job.wfo_job_id.substring(0, 8)}...
              </Link>
            </TableCell>
            <TableCell>{job.strategy_id}</TableCell>
            <TableCell>{job.symbol}</TableCell>
            <TableCell>{job.in_sample_period_months}m / {job.out_of_sample_period_months}m</TableCell>
            <TableCell><Badge>{job.wfo_status}</Badge></TableCell>
            <TableCell>{formatTimestamp(job.created_at)}</TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}