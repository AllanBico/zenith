"use client";
import { JobsDataTable } from "@/components/dashboard/JobsDataTable";
import { useOptimizationJobs } from "@/hooks/useZenithApi";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

export default function OptimizationsPage() {
  const { data: jobs, isLoading, error } = useOptimizationJobs();

  return (
    <Card>
      <CardHeader>
        <CardTitle>Optimization Jobs</CardTitle>
      </CardHeader>
      <CardContent>
        {isLoading && <p>Loading jobs...</p>}
        {error && <p className="text-red-500">Error: {error.message}</p>}
        {jobs && <JobsDataTable data={jobs} />}
      </CardContent>
    </Card>
  );
}