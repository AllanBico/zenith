"use client";
import { JobDetailsTable } from "@/components/dashboard/JobDetailsTable";
import { useJobDetails } from "@/hooks/useZenithApi";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

export default function JobDetailsPage({ params }: { params: { jobId: string } }) {
  const { data: rankedReports, isLoading, error } = useJobDetails(params.jobId);

  return (
    <Card>
      <CardHeader>
        <CardTitle>Job Details: {params.jobId.substring(0, 8)}...</CardTitle>
      </CardHeader>
      <CardContent>
        {isLoading && <p>Loading details...</p>}
        {error && <p className="text-red-500">Error: {error.message}</p>}
        {rankedReports && <JobDetailsTable data={rankedReports} />}
      </CardContent>
    </Card>
  );
}