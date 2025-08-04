import { OptimizationJob, RankedReport, BacktestRunDetails, WfoJob, WfoRun } from "@/types/zenith";

// The base URL for our Zenith backend API.
// In a real app, this would come from an environment variable.
const API_BASE_URL = "http://localhost:8080/api";

async function fetcher<T>(url: string): Promise<T> {
  const res = await fetch(url);
  if (!res.ok) {
    const errorBody = await res.json().catch(() => ({ error: "An unknown error occurred" }));
    throw new Error(errorBody.error || `Request failed with status ${res.status}`);
  }
  return res.json();
}

export const getOptimizationJobs = (): Promise<OptimizationJob[]> => {
  return fetcher(`${API_BASE_URL}/optimization-jobs`);
};

export const getJobDetails = (jobId: string): Promise<RankedReport[]> => {
  return fetcher(`${API_BASE_URL}/optimization-jobs/${jobId}`);
};

export const getRunDetails = (runId: string): Promise<BacktestRunDetails> => {
    return fetcher(`${API_BASE_URL}/backtest-runs/${runId}/details`);
};

export const getSingleRuns = (): Promise<RankedReport[]> => {
    return fetcher(`${API_BASE_URL}/single-runs`);
}

export const testCors = (): Promise<string> => {
    return fetcher(`${API_BASE_URL}/cors-test`);
}

export const getWfoJobs = (): Promise<WfoJob[]> => {
    return fetcher(`${API_BASE_URL}/wfo-jobs`);
}

export const getWfoJobDetails = (wfoJobId: string): Promise<WfoRun[]> => {
    return fetcher(`${API_BASE_URL}/wfo-jobs/${wfoJobId}/runs`);
}