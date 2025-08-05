import { create } from 'zustand';
import { LogMessage, PortfolioState, KlineData } from '@/types/zenith';

export type ConnectionStatus = "Connecting" | "Connected" | "Disconnected";

interface LiveState {
  status: ConnectionStatus;
  portfolioState: PortfolioState | null;
  logs: LogMessage[];
  klineData: Map<string, KlineData>; // Store latest kline data per symbol
}

interface LiveActions {
  setStatus: (status: ConnectionStatus) => void;
  setPortfolioState: (state: PortfolioState) => void;
  addLog: (log: LogMessage) => void;
  updateKlineData: (klineData: KlineData) => void;
}

const MAX_LOGS = 200; // The maximum number of log messages to keep in memory

export const useLiveStore = create<LiveState & LiveActions>((set) => ({
  status: "Connecting",
  portfolioState: null,
  logs: [],
  klineData: new Map(),
  setStatus: (status) => set({ status }),
  setPortfolioState: (state) => set({ portfolioState: state }),
  addLog: (log) =>
    set((state) => ({
      // Add the new log to the beginning of the array and cap the total length.
      logs: [log, ...state.logs].slice(0, MAX_LOGS),
    })),
  updateKlineData: (klineData) =>
    set((state) => {
      const newKlineData = new Map(state.klineData);
      newKlineData.set(klineData.symbol, klineData);
      return { klineData: newKlineData };
    }),
}));