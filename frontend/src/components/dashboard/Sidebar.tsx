"use client";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { 
    BarChart3, 
    TrendingUp, 
    Settings, 
    Building2 
} from "lucide-react";

const navItems = [
    { 
        href: "/admin/optimizations", 
        label: "Optimizations", 
        icon: BarChart3 
    },
    { 
        href: "/admin/single-runs", 
        label: "Single Runs", 
        icon: TrendingUp 
    },
    { 
        href: "/admin/wfo-jobs", 
        label: "WFO Jobs", 
        icon: Settings 
    },
];

export function Sidebar() {
    const pathname = usePathname();
    
    return (
        <nav className="w-64 border-r bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
            <div className="flex h-full flex-col gap-2 p-4">
                <div className="flex items-center gap-2 px-2 py-2">
                    <Building2 className="h-6 w-6" />
                    <h1 className="text-xl font-semibold">Zenith</h1>
                </div>
                
                <div className="flex-1 space-y-1">
                    {navItems.map((item) => {
                        const Icon = item.icon;
                        const isActive = pathname === item.href;
                        
                        return (
                            <Button
                                key={item.href}
                                variant={isActive ? "secondary" : "ghost"}
                                size="sm"
                                className={cn(
                                    "w-full justify-start gap-2",
                                    isActive && "bg-secondary text-secondary-foreground"
                                )}
                                asChild
                            >
                                <Link href={item.href}>
                                    <Icon className="h-4 w-4" />
                                    {item.label}
                                </Link>
                            </Button>
                        );
                    })}
                </div>
                
                <div className="mt-auto border-t pt-4">
                    <p className="text-xs text-muted-foreground px-2">
                        © 2024 Zenith Trading
                    </p>
                </div>
            </div>
        </nav>
    );
}