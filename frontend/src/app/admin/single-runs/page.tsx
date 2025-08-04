"use client";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { useSingleRuns } from "@/hooks/useZenithApi";
import { SingleRunsTable } from "@/components/dashboard/SingleRunsTable";

export default function SingleRunsPage() {
  const { data: runs, isLoading, error } = useSingleRuns();

  return (
    <Card>
      <CardHeader>
        <CardTitle>Single Backtest Runs</CardTitle>
        <CardDescription>
          A list of all backtests executed via the `single-run` command.
        </CardDescription>
      </CardHeader>
      <CardContent>
        {isLoading && <p>Loading runs...</p>}
        {error && <p className="text-destructive">Error: {error.message}</p>}
        {runs && <SingleRunsTable data={runs} />}
      </CardContent>
    </Card>
  );
}