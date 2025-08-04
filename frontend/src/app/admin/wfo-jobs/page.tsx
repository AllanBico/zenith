"use client";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { useWfoJobs } from "@/hooks/useZenithApi";
import { WfoJobsTable } from "@/components/dashboard/WfoJobsTable";

export default function WfoJobsPage() {
  const { data: jobs, isLoading, error } = useWfoJobs();

  return (
    <Card>
      <CardHeader>
        <CardTitle>Walk-Forward Optimization Jobs</CardTitle>
        <CardDescription>
          A list of all completed WFO validation runs, the gold standard for strategy testing.
        </CardDescription>
      </CardHeader>
      <CardContent>
        {isLoading && <p>Loading WFO jobs...</p>}
        {error && <p className="text-destructive">Error: {error.message}</p>}
        {jobs && <WfoJobsTable data={jobs} />}
      </CardContent>
    </Card>
  );
}