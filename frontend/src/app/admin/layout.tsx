import React from "react";
import { Sidebar } from "@/components/dashboard/Sidebar";

// This layout is ONLY for the /admin/* routes.
// It renders the persistent sidebar and a main content area for its children.
export default function AdminLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <>
      <Sidebar />
      <main className="flex-1 p-8 overflow-y-auto">
        {children}
      </main>
    </>
  );
}