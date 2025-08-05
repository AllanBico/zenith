"use client";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useLiveStore } from "@/store/live";
import { LogLevel } from "@/types/zenith";
import { cn } from "@/lib/utils";

const logLevelColor: Record<LogLevel, string> = {
    Info: "text-gray-400",
    Warn: "text-yellow-500",
    Error: "text-red-500",
};

export function LiveLogStream() {
  const logs = useLiveStore((state) => state.logs);

  return (
    <Card className="h-full flex flex-col overflow-hidden">
      <CardHeader>
        <CardTitle>Engine Log Stream</CardTitle>
      </CardHeader>
      <CardContent className="flex-1 min-h-0 overflow-y-auto">
        <div className="flex flex-col-reverse">
            {logs.map((log, index) => (
                <div key={index} className="font-mono text-xs p-1 border-b border-dashed border-muted">
                    <span className={cn(logLevelColor[log.level], "font-bold")}>[{log.level.toUpperCase()}]</span>
                    <span className="text-gray-500 ml-2">{new Date(log.timestamp).toLocaleTimeString()}</span>
                    <span className="ml-2">{log.message}</span>
                </div>
            ))}
        </div>
      </CardContent>
    </Card>
  );
}