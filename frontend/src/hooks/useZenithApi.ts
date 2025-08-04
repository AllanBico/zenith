"use client"; // This file contains client-side hooks

import { useQuery } from "@tanstack/react-query";
import * as api from "@/services/api";

// A unique key for caching and refetching optimization jobs
const jobsQueryKey = ["optimizationJobs"];
const singleRunsQueryKey = ["singleRuns"];

export const useOptimizationJobs = () => {
  return useQuery({
    queryKey: jobsQueryKey,
    queryFn: api.getOptimizationJobs,
  });
};

export const useJobDetails = (jobId: string) => {
    return useQuery({
        queryKey: ["jobDetails", jobId],
        queryFn: () => api.getJobDetails(jobId),
        enabled: !!jobId, // The query will not run until jobId is available
    });
};

export const useRunDetails = (runId: string) => {
    return useQuery({
        queryKey: ["runDetails", runId],
        queryFn: () => api.getRunDetails(runId),
        enabled: !!runId,
    });
};

export const useSingleRuns = () => {
    return useQuery({
        queryKey: singleRunsQueryKey,
        queryFn: api.getSingleRuns,
    });
};
// Add this hook to your react-query hooks file
export const useWfoJobs = () => {
    return useQuery({
        queryKey: ["wfoJobs"],
        queryFn: api.getWfoJobs,
    });
};