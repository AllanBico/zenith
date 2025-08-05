"use client";
import { useEffect, useRef } from 'react';
import { useLiveStore } from '@/store/live';
import { WsMessage } from '@/types/zenith';

// The URL for our backend WebSocket.
// In a real app, this would come from an environment variable.
const WS_URL = "ws://127.0.0.1:8080/ws";

export const useLiveSocket = () => {
  const { setStatus, setPortfolioState, addLog, updateKlineData } = useLiveStore();
  const socketRef = useRef<WebSocket | null>(null);

  useEffect(() => {
    // This effect runs once on component mount.
    const connect = () => {
      setStatus("Connecting");
      const socket = new WebSocket(WS_URL);
      socketRef.current = socket;

      socket.onopen = () => {
        console.log("WebSocket connection established.");
        setStatus("Connected");
      };

      socket.onmessage = (event) => {
        try {
          const message = JSON.parse(event.data) as WsMessage;
          console.log('WebSocket message received:', message);
          
          // Use a switch on the message type to update the correct part of the store.
          switch (message.type) {
            case "Log":
              addLog(message.payload);
              break;
            case "PortfolioState":
              setPortfolioState(message.payload);
              break;
            case "KlineData":
              console.log('KlineData received:', message.payload);
              updateKlineData(message.payload);
              break;
            case "Connected":
              console.log('WebSocket connection confirmed');
              break;
            default:
              console.log('Unknown message type:', message);
          }
        } catch (error) {
          console.error("Failed to parse WebSocket message:", error);
          console.error("Raw message data:", event.data);
        }
      };

      socket.onclose = (event) => {
        console.warn("WebSocket connection closed. Code:", event.code, "Reason:", event.reason);
        setStatus("Disconnected");
        // Simple exponential backoff for reconnection
        setTimeout(connect, 5000);
      };

      socket.onerror = (error) => {
        console.error("WebSocket error:", error);
        socket.close(); // This will trigger the onclose handler to reconnect
      };
    };

    connect();

    // Add a periodic check to see if the connection is still alive
    const intervalId = setInterval(() => {
      if (socketRef.current && socketRef.current.readyState === WebSocket.OPEN) {
        // Send a ping to keep the connection alive
        socketRef.current.send(JSON.stringify({ type: "ping" }));
      }
    }, 30000); // Every 30 seconds

    // Cleanup function to close the socket when the component unmounts.
    return () => {
      clearInterval(intervalId);
      if (socketRef.current) {
        socketRef.current.close();
      }
    };
  }, []); // Remove dependencies - Zustand store functions are stable
};