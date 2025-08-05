"use client";
import { useLiveSocket } from "@/hooks/useLiveSocket";
import { useDynamicTitle } from "@/hooks/useDynamicTitle";
import { LiveStatusHeader } from "@/components/dashboard/live/LiveStatusHeader";
import { PortfolioVitals } from "@/components/dashboard/live/PortfolioVitals";
import { OpenPositionsTable } from "@/components/dashboard/live/OpenPositionsTable";
import { LiveLogStream } from "@/components/dashboard/live/LiveLogStream";
import { SessionEquityChart } from "@/components/dashboard/live/SessionEquityChart";
import { KlineDataDisplay } from "@/components/dashboard/live/KlineDataDisplay";

export default function LiveDashboardPage() {
  useLiveSocket();
  useDynamicTitle();

  return (
    <div className="flex flex-col h-full gap-4 p-4 sm:p-6 md:p-8">
      <LiveStatusHeader />
      
      <div className="flex-1 grid grid-cols-1 lg:grid-cols-5 gap-4 overflow-hidden">
        {/* Main Column (takes up 3/5 of the space on large screens) */}
        <div className="lg:col-span-3 flex flex-col gap-4">
          <PortfolioVitals />
          <div className="flex-1">
            <OpenPositionsTable />
          </div>
        </div>

        {/* Right Column (takes up 2/5 of the space on large screens) */}
        <div className="lg:col-span-2 flex flex-col gap-4 min-h-0">
            <div className="h-1/3 min-h-0">
                <SessionEquityChart />
            </div>
            <div className="h-1/3 min-h-0">
                <KlineDataDisplay />
            </div>
            <div className="h-1/3 min-h-0">
                <LiveLogStream />
            </div>
        </div>
      </div>
    </div>
  );
}