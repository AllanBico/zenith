"use client";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Trade } from "@/types/zenith";
import { format } from "date-fns";
import { Badge } from "@/components/ui/badge";

function calculatePnl(trade: Trade): number {
    const entryPrice = parseFloat(trade.entry_execution.price);
    const exitPrice = parseFloat(trade.exit_execution.price);
    const qty = parseFloat(trade.entry_execution.quantity);
    
    // Check for valid numbers
    if (isNaN(entryPrice) || isNaN(exitPrice) || isNaN(qty)) {
        return 0;
    }
    
    if (trade.entry_execution.side === "Buy") {
        return (exitPrice - entryPrice) * qty;
    } else {
        return (entryPrice - exitPrice) * qty;
    }
}

function formatTimestamp(timestamp: string): string {
    try {
        const date = new Date(timestamp);
        if (isNaN(date.getTime())) {
            return "Invalid Date";
        }
        return format(date, "yyyy-MM-dd HH:mm");
    } catch (error) {
        return "Invalid Date";
    }
}

function safeParseFloat(value: string | number | undefined): number {
    if (typeof value === 'number') return value;
    if (typeof value === 'string') {
        const parsed = parseFloat(value);
        return isNaN(parsed) ? 0 : parsed;
    }
    return 0;
}

export function TradeHistoryTable({ data }: { data: Trade[] }) {
  if (!data || data.length === 0) {
    return <div className="text-center p-4 text-muted-foreground">No trade data available</div>;
  }

  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Side</TableHead>
          <TableHead>Entry Time</TableHead>
          <TableHead>Exit Time</TableHead>
          <TableHead className="text-right">Entry Price/Unit</TableHead>
          <TableHead className="text-right">Exit Price/Unit</TableHead>
          <TableHead className="text-right">Entry Cost</TableHead>
          <TableHead className="text-right">Exit Value</TableHead>
          <TableHead className="text-right">Quantity</TableHead>
          <TableHead className="text-right">P&L</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {data.map((trade, index) => {
            const pnl = calculatePnl(trade);
            const side = trade.entry_execution.side;
            const entryPrice = safeParseFloat(trade.entry_execution.price);
            const exitPrice = safeParseFloat(trade.exit_execution.price);
            const qty = safeParseFloat(trade.entry_execution.quantity);
            
            // Calculate actual costs
            const entryCost = entryPrice * qty;
            const exitValue = exitPrice * qty;
            
            return (
                <TableRow key={trade.trade_id || index}>
                    <TableCell>
                        <Badge variant={side === "Buy" ? "default" : "secondary"}>
                            {side.toUpperCase()}
                        </Badge>
                    </TableCell>
                    <TableCell>{formatTimestamp(trade.entry_execution.timestamp)}</TableCell>
                    <TableCell>{formatTimestamp(trade.exit_execution.timestamp)}</TableCell>
                    <TableCell className="text-right font-mono">${entryPrice.toFixed(2)}</TableCell>
                    <TableCell className="text-right font-mono">${exitPrice.toFixed(2)}</TableCell>
                    <TableCell className="text-right font-mono">${entryCost.toFixed(2)}</TableCell>
                    <TableCell className="text-right font-mono">${exitValue.toFixed(2)}</TableCell>
                    <TableCell className="text-right font-mono">{qty.toFixed(6)}</TableCell>
                    <TableCell className={`text-right font-mono ${pnl >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                        ${pnl.toFixed(2)}
                    </TableCell>
                </TableRow>
            )
        })}
      </TableBody>
    </Table>
  );
}