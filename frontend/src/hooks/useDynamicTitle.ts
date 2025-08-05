"use client";
import { useEffect } from 'react';
import { useLiveStore } from '@/store/live';

const INITIAL_CAPITAL = 10000; // This should ideally come from config or initial state

const formatCurrency = (value: number) => {
    return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(value);
}

export const useDynamicTitle = () => {
  const portfolioState = useLiveStore((state) => state.portfolioState);

  useEffect(() => {
    if (portfolioState) {
        const pnl = parseFloat(portfolioState.total_value) - INITIAL_CAPITAL;
        const sign = pnl >= 0 ? '+' : '';
        document.title = `${sign}${formatCurrency(pnl)} | Zenith Live`;
    } else {
        document.title = "Zenith Live";
    }
  }, [portfolioState]);
};