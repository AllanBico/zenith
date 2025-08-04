"use client";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { OptimizationJob } from "@/types/zenith";
import { Badge } from "@/components/ui/badge";
import Link from "next/link";
import { format } from "date-fns";

export function JobsDataTable({ data }: { data: OptimizationJob[] }) {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Job ID</TableHead>
          <TableHead>Strategy</TableHead>
          <TableHead>Symbol</TableHead>
          <TableHead>Status</TableHead>
          <TableHead>Created At</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {data.map((job) => (
          <TableRow key={job.job_id}>
            <TableCell>
              <Link href={`/admin/optimizations/${job.job_id}`} className="text-primary hover:underline">
                {job.job_id.substring(0, 8)}...
              </Link>
            </TableCell>
            <TableCell>{job.strategy_id}</TableCell>
            <TableCell>{job.symbol}</TableCell>
            <TableCell><Badge>{job.job_status}</Badge></TableCell>
            <TableCell>{format(new Date(job.created_at), "yyyy-MM-dd HH:mm")}</TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}