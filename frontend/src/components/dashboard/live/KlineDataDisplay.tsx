"use client";
import { useLiveStore } from '@/store/live';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { useEffect } from 'react';

export const KlineDataDisplay = () => {
  const { klineData } = useLiveStore();

  // Debug logging to see the actual timestamp format
  useEffect(() => {
    if (klineData.size > 0) {
      const firstKline = Array.from(klineData.values())[0];
      console.log('Kline data received:', firstKline);
      console.log('Open time format:', firstKline.kline.open_time);
      console.log('Close time format:', firstKline.kline.close_time);
    }
  }, [klineData]);

  if (klineData.size === 0) {
    return (
      <Card>
        <CardHeader>
          <CardTitle>Live Kline Data</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-muted-foreground">No kline data received yet...</p>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card className="h-full flex flex-col overflow-hidden">
      <CardHeader>
        <CardTitle>Live Kline Data</CardTitle>
      </CardHeader>
      <CardContent className="flex-1 min-h-0 overflow-auto">
        <div className="space-y-4">
          {Array.from(klineData.values()).map((data) => (
            <div key={data.symbol} className="border rounded-lg p-4">
              <div className="flex items-center justify-between mb-2">
                <h3 className="font-semibold">{data.symbol}</h3>
                <Badge variant="secondary">{data.kline.interval}</Badge>
              </div>
              <div className="grid grid-cols-2 gap-2 text-sm">
                <div>
                  <span className="text-muted-foreground">Open:</span>
                  <span className="ml-2 font-mono">${parseFloat(data.kline.open).toFixed(2)}</span>
                </div>
                <div>
                  <span className="text-muted-foreground">High:</span>
                  <span className="ml-2 font-mono text-green-600">${parseFloat(data.kline.high).toFixed(2)}</span>
                </div>
                <div>
                  <span className="text-muted-foreground">Low:</span>
                  <span className="ml-2 font-mono text-red-600">${parseFloat(data.kline.low).toFixed(2)}</span>
                </div>
                <div>
                  <span className="text-muted-foreground">Close:</span>
                  <span className="ml-2 font-mono">${parseFloat(data.kline.close).toFixed(2)}</span>
                </div>
                <div className="col-span-2">
                  <span className="text-muted-foreground">Volume:</span>
                  <span className="ml-2 font-mono">{parseFloat(data.kline.volume).toFixed(2)}</span>
                </div>
              </div>
              <div className="mt-2 text-xs text-muted-foreground">
                <div>Open: {(() => {
                  try {
                    return new Date(data.kline.open_time).toLocaleTimeString();
                  } catch (error) {
                    return data.kline.open_time;
                  }
                })()}</div>
                <div>Close: {(() => {
                  try {
                    return new Date(data.kline.close_time).toLocaleTimeString();
                  } catch (error) {
                    return data.kline.close_time;
                  }
                })()}</div>
              </div>
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}; 