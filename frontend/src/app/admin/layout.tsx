import React from "react";

// This layout is now much simpler. It just passes its children through,
// as the main layout in the parent directory already provides the sidebar and main content area.
export default function AdminLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return <>{children}</>;
}