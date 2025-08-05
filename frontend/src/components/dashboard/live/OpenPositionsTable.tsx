"use client";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { useLiveStore } from "@/store/live";
import { Badge } from "@/components/ui/badge";
import { useMemo } from "react";

export function OpenPositionsTable() {
  const portfolioState = useLiveStore((state) => state.portfolioState);
  const positions = useMemo(() => portfolioState?.positions || [], [portfolioState?.positions]);

  return (
    <Card className="h-full">
      <CardHeader>
        <CardTitle>Open Positions</CardTitle>
      </CardHeader>
      <CardContent>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Symbol</TableHead>
              <TableHead>Side</TableHead>
              <TableHead className="text-right">Size</TableHead>
              <TableHead className="text-right">Entry Price</TableHead>
              <TableHead className="text-right">Unrealized P&L</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {positions.length > 0 ? (
              positions.map((pos) => (
                <TableRow key={pos.position_id}>
                  <TableCell className="font-medium">{pos.symbol}</TableCell>
                  <TableCell>
                    <Badge variant={pos.side === "Buy" ? "default" : "secondary"}>
                      {pos.side.toUpperCase()}
                    </Badge>
                  </TableCell>
                  <TableCell className="text-right font-mono">{parseFloat(pos.quantity).toFixed(4)}</TableCell>
                  <TableCell className="text-right font-mono">{parseFloat(pos.entry_price).toFixed(2)}</TableCell>
                  <TableCell className={`text-right font-mono ${parseFloat(pos.unrealized_pnl) >= 0 ? 'text-green-500' : 'text-red-500'}`}>
                    ${parseFloat(pos.unrealized_pnl).toFixed(2)}
                  </TableCell>
                </TableRow>
              ))
            ) : (
              <TableRow>
                <TableCell colSpan={5} className="text-center text-muted-foreground">No open positions.</TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}