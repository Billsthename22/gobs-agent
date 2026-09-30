"use client";

import { useEffect, useState } from "react";
import {
  Activity,
  AlertTriangle,
  CheckCircle2,
  Computer,
} from "lucide-react";

import Sidebar from "@/components/layout/Sidebar";
import Topbar from "@/components/layout/Topbar";
import StatCard from "@/components/dashboard/StatCard";
import DeviceTable from "@/components/devices/DeviceTable";

interface Device {
  id: number;
  device_name: string;
  hostname: string;
  operating_system: string;
  ip_address: string | null;
  employee_id: number | null;
  status: string;
  cpu_usage: number;
  memory_usage: number;
  storage_usage: number;
  last_seen: string | null;
  is_active: boolean;
  created_at: string;
}

export default function DevicesPage() {
  const [devices, setDevices] = useState<Device[]>([]);

  useEffect(() => {
    async function fetchDevices() {
      try {
        const response = await fetch(
          "https://gbos-backend-production.up.railway.app/devices",
          {
            cache: "no-store",
          }
        );

        if (!response.ok) {
          throw new Error("Failed to fetch devices");
        }

        const data: Device[] = await response.json();
        setDevices(data);
      } catch (error) {
        console.error("Failed to load device statistics:", error);
      }
    }

    fetchDevices();

    const interval = setInterval(fetchDevices, 5000);

    return () => clearInterval(interval);
  }, []);

  const totalDevices = devices.length;
  const onlineDevices = devices.filter(
    (device) => device.status === "online"
  ).length;
  const activeDevices = devices.filter(
    (device) => device.is_active
  ).length;
  const issueDevices = devices.filter(
    (device) => device.status === "stale" || device.status === "offline"
  ).length;

  const onlinePercentage =
    totalDevices > 0
      ? Math.round((onlineDevices / totalDevices) * 100)
      : 0;

  return (
    <div className="min-h-screen bg-zinc-50">
      <Sidebar />
      <Topbar />

      <main className="ml-64 pt-20">
        <div className="space-y-6 p-8">
          {/* Header */}
          <div>
            <p className="text-sm font-medium text-zinc-500">
              Device Management
            </p>

            <h1 className="mt-1 text-2xl font-semibold tracking-tight text-zinc-950">
              Company Devices
            </h1>

            <p className="mt-2 text-sm text-zinc-500">
              Monitor and manage all company-owned computers.
            </p>
          </div>

          {/* Stats */}
          <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
            <StatCard
              title="Total Devices"
              value={String(totalDevices)}
              description="registered devices"
              icon={Computer}
            />

            <StatCard
              title="Online"
              value={String(onlineDevices)}
              description="currently connected"
              trend={`${onlinePercentage}%`}
              icon={CheckCircle2}
            />

            <StatCard
              title="Active"
              value={String(activeDevices)}
              description="active devices"
              icon={Activity}
            />

            <StatCard
              title="Issues"
              value={String(issueDevices)}
              description="require attention"
              icon={AlertTriangle}
            />
          </div>

          {/* Device table */}
          <DeviceTable />
        </div>
      </main>
    </div>
  );
}
