"use client";

import {
  CheckCircle2,
  CircleAlert,
  Cpu,
  HardDrive,
  MemoryStick,
  Monitor,
} from "lucide-react";
import { useEffect, useState } from "react";

interface Device {
  id: number;
  device_name: string;
  hostname: string;
  operating_system: string;
  employee_id: number | null;
  status: string;
  cpu_usage: number;
  memory_usage: number;
  storage_usage: number;
  last_seen: string | null;
}

interface EmployeeDeviceProps {
  employeeId: string;
}

export default function EmployeeDevice({
  employeeId,
}: EmployeeDeviceProps) {
  const [device, setDevice] = useState<Device | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;

    async function loadDevice() {
      try {
        setLoading(true);

        const response = await fetch(
          "https://gbos-backend-production.up.railway.app/devices",
          {
            cache: "no-store",
          }
        );

        if (!response.ok) {
          throw new Error("Failed to load devices");
        }

        const devices: Device[] = await response.json();

        const assignedDevice =
          devices.find(
            (item) =>
              item.employee_id === Number(employeeId)
          ) ?? null;

        if (!cancelled) {
          setDevice(assignedDevice);
        }
      } catch (error) {
        console.error(
          "Failed to load employee device:",
          error
        );

        if (!cancelled) {
          setDevice(null);
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    loadDevice();

    return () => {
      cancelled = true;
    };
  }, [employeeId]);

  const isOnline =
    device?.status.toLowerCase() === "online";

  return (
    <div className="rounded-2xl border border-zinc-200 bg-white">
      <div className="flex items-center justify-between border-b border-zinc-100 px-6 py-5">
        <div>
          <h2 className="text-sm font-semibold text-zinc-950">
            Assigned Device
          </h2>

          <p className="mt-1 text-xs text-zinc-500">
            Computer currently assigned to this employee
          </p>
        </div>

        {!loading && device && (
          <span
            className={`flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[10px] font-medium ${
              isOnline
                ? "bg-emerald-50 text-emerald-600"
                : "bg-zinc-100 text-zinc-500"
            }`}
          >
            {isOnline ? (
              <CheckCircle2 size={12} />
            ) : (
              <CircleAlert size={12} />
            )}

            {isOnline ? "Online" : "Offline"}
          </span>
        )}
      </div>

      <div className="p-6">
        {loading ? (
          <div>
            <p className="text-sm font-medium text-zinc-900">
              Loading device...
            </p>

            <p className="mt-1 text-xs text-zinc-500">
              Finding the device assigned to this employee.
            </p>
          </div>
        ) : !device ? (
          <div>
            <p className="text-sm font-medium text-zinc-900">
              No device assigned
            </p>

            <p className="mt-1 text-xs text-zinc-500">
              This employee does not currently have an
              assigned device.
            </p>
          </div>
        ) : (
          <>
            <div className="flex items-center gap-4">
              <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-zinc-950 text-white">
                <Monitor size={20} />
              </div>

              <div>
                <p className="text-sm font-semibold text-zinc-900">
                  {device.device_name}
                </p>

                <p className="mt-1 text-xs text-zinc-500">
                  {device.hostname} · {device.operating_system}
                </p>
              </div>
            </div>

            <div className="mt-6 grid grid-cols-3 gap-4 border-t border-zinc-100 pt-5">
              <div>
                <div className="flex items-center gap-1.5 text-xs text-zinc-400">
                  <Cpu size={13} />
                  CPU
                </div>

                <p className="mt-2 text-sm font-medium text-zinc-800">
                  {device.cpu_usage.toFixed(1)}%
                </p>
              </div>

              <div>
                <div className="flex items-center gap-1.5 text-xs text-zinc-400">
                  <MemoryStick size={13} />
                  Memory
                </div>

                <p className="mt-2 text-sm font-medium text-zinc-800">
                  {device.memory_usage.toFixed(1)}%
                </p>
              </div>

              <div>
                <div className="flex items-center gap-1.5 text-xs text-zinc-400">
                  <HardDrive size={13} />
                  Storage
                </div>

                <p className="mt-2 text-sm font-medium text-zinc-800">
                  {device.storage_usage.toFixed(1)}%
                </p>
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
