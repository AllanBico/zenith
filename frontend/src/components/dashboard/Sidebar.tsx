"use client";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { cn } from "@/lib/utils";

const navItems = [
    { href: "/admin/optimizations", label: "Optimizations" },
    { href: "/admin/single-runs", label: "Single Runs" },
    { href: "/admin/wfo-jobs", label: "WFO Jobs" },
];

export function Sidebar() {
    const pathname = usePathname();
    return (
        <nav className="w-64 border-r p-4 flex flex-col">
            <h1 className="text-2xl font-bold mb-4">Zenith</h1>
            <ul className="space-y-2">
                {navItems.map((item) => (
                    <li key={item.href}>
                        <Link href={item.href}
                            className={cn(
                                "block p-2 rounded-md text-lg hover:bg-muted",
                                pathname === item.href && "bg-primary text-primary-foreground"
                            )}>
                            {item.label}
                        </Link>
                    </li>
                ))}
            </ul>
            <div className="mt-auto">
              <p className="text-xs text-muted-foreground">© 2024 Zenith Trading</p>
            </div>
        </nav>
    );
}