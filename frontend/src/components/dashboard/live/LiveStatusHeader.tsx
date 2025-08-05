"use client";
import { Badge } from "@/components/ui/badge";
import { useLiveStore } from "@/store/live";

export function LiveStatusHeader() {
  const status = useLiveStore((state) => state.status);

  const getStatusVariant = () => {
    switch (status) {
      case "Connected": return "default";
      case "Connecting": return "secondary";
      case "Disconnected": return "destructive";
      default: return "secondary";
    }
  };

  return (
    <div className="flex items-center justify-between p-2 border-b">
      <h1 className="text-xl font-bold">Live Dashboard</h1>
      <div className="flex items-center gap-2">
        <span>Status:</span>
        <Badge variant={getStatusVariant()}>{status}</Badge>
      </div>
    </div>
  );
}