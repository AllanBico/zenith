"use client";

import { useEffect, useState } from "react";
import { getWfoJobs } from "@/services/api";
import { WfoJob } from "@/types/zenith";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";

export default function WfoJobsPage() {
    const [jobs, setJobs] = useState<WfoJob[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        const fetchJobs = async () => {
            try {
                const data = await getWfoJobs();
                setJobs(data);
            } catch (err) {
                setError(err instanceof Error ? err.message : "Failed to fetch WFO jobs");
            } finally {
                setLoading(false);
            }
        };

        fetchJobs();
    }, []);

    if (loading) {
        return (
            <div className="p-6">
                <h1 className="text-2xl font-bold mb-6">WFO Jobs</h1>
                <div className="flex items-center justify-center h-64">
                    <p>Loading...</p>
                </div>
            </div>
        );
    }

    if (error) {
        return (
            <div className="p-6">
                <h1 className="text-2xl font-bold mb-6">WFO Jobs</h1>
                <Card>
                    <CardContent className="p-6">
                        <p className="text-red-600">Error: {error}</p>
                    </CardContent>
                </Card>
            </div>
        );
    }

    const getStatusBadgeVariant = (status: string) => {
        switch (status.toLowerCase()) {
            case 'completed':
                return 'default';
            case 'running':
                return 'secondary';
            case 'failed':
                return 'destructive';
            default:
                return 'outline';
        }
    };

    return (
        <div className="p-6">
            <h1 className="text-2xl font-bold mb-6">WFO Jobs</h1>
            
            {jobs.length === 0 ? (
                <Card>
                    <CardContent className="p-6">
                        <p className="text-muted-foreground">No WFO jobs found.</p>
                    </CardContent>
                </Card>
            ) : (
                <Card>
                    <CardHeader>
                        <CardTitle>Walk-Forward Optimization Jobs</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <TableHead>Job ID</TableHead>
                                    <TableHead>Strategy</TableHead>
                                    <TableHead>Symbol</TableHead>
                                    <TableHead>In-Sample Period</TableHead>
                                    <TableHead>Out-of-Sample Period</TableHead>
                                    <TableHead>Status</TableHead>
                                    <TableHead>Created</TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {jobs.map((job) => (
                                    <TableRow key={job.wfo_job_id}>
                                        <TableCell className="font-mono text-sm">
                                            {job.wfo_job_id.slice(0, 8)}...
                                        </TableCell>
                                        <TableCell>
                                            <Badge variant="outline">
                                                {job.strategy_id}
                                            </Badge>
                                        </TableCell>
                                        <TableCell>
                                            {job.symbol}
                                        </TableCell>
                                        <TableCell>
                                            {job.in_sample_period_months} months
                                        </TableCell>
                                        <TableCell>
                                            {job.out_of_sample_period_months} months
                                        </TableCell>
                                        <TableCell>
                                            <Badge variant={getStatusBadgeVariant(job.wfo_status)}>
                                                {job.wfo_status}
                                            </Badge>
                                        </TableCell>
                                        <TableCell>
                                            {new Date(job.created_at).toLocaleDateString()}
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