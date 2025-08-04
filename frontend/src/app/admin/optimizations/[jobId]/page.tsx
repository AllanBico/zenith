"use client";
import { JobDetailsTable } from "@/components/dashboard/JobDetailsTable";
import { useJobDetails } from "@/hooks/useZenithApi";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { use } from "react";

export default function JobDetailsPage({ params }: { params: Promise<{ jobId: string }> }) {
  const { jobId } = use(params);
  const { data: rankedReports, isLoading, error } = useJobDetails(jobId);

  // Debug logging
  console.log('JobDetailsPage - jobId:', jobId);
  console.log('JobDetailsPage - isLoading:', isLoading);
  console.log('JobDetailsPage - error:', error);
  console.log('JobDetailsPage - rankedReports:', rankedReports);

  return (
    <Card>
      <CardHeader>
        <CardTitle>Job Details: {jobId.substring(0, 8)}...</CardTitle>
      </CardHeader>
      <CardContent>
        {isLoading && <p>Loading details...</p>}
        {error && <p className="text-red-500">Error: {error.message}</p>}
        {!isLoading && !error && !rankedReports && <p>No data found for this job.</p>}
        {rankedReports && rankedReports.length === 0 && <p>No optimization results found.</p>}
        {rankedReports && rankedReports.length > 0 && (
          <>
            <p className="text-sm text-muted-foreground mb-4">Found {rankedReports.length} optimization results</p>
            <JobDetailsTable data={rankedReports} />
          </>
        )}
      </CardContent>
    </Card>
  );
}