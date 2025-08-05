import React from "react";
import { ThemeProvider } from "@/components/theme-provider";

// This layout is for the main dashboard pages. It does NOT include a sidebar.
export default function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <div className="flex-1 p-4 sm:p-6 md:p-8">
      {children}
    </div>
  );
}