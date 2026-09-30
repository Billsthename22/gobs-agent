"use client";

import {
  CheckCircle2,
  CircleAlert,
  MoreHorizontal,
  Search,
  UserRound,
  UserRoundCheck,
} from "lucide-react";
import Link from "next/link";
import { useEffect, useMemo, useState } from "react";

interface Employee {
  id: number;
  full_name: string;
  email: string;
  department: string | null;
  job_title: string | null;
  phone: string | null;
  status: string;
  is_active: boolean;
  created_at: string;
}

interface EmployeeTableProps {
  refreshKey?: number;
}

export default function EmployeeTable({
  refreshKey = 0,
}: EmployeeTableProps) {
  const [employees, setEmployees] = useState<Employee[]>([]);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState("All");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [openMenuId, setOpenMenuId] = useState<number | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function loadEmployees() {
      try {
        setLoading(true);
        setError("");

        const response = await fetch(
          "https://gbos-backend-production.up.railway.app/employees",
          {
            cache: "no-store",
          }
        );

        if (!response.ok) {
          throw new Error(
            `Failed to load employees (${response.status})`
          );
        }

        const data: Employee[] = await response.json();

        if (!cancelled) {
          setEmployees(data);
        }
      } catch (err) {
        if (!cancelled) {
          setError(
            err instanceof Error
              ? err.message
              : "Failed to load employees."
          );
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    loadEmployees();

    return () => {
      cancelled = true;
    };
  }, [refreshKey]);

  async function toggleEmployeeActive(employee: Employee) {
    try {
      setError("");

      const response = await fetch(
        `https://gbos-backend-production.up.railway.app/employees/${employee.id}`,
        {
          method: "PATCH",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            is_active: !employee.is_active,
            status: employee.is_active ? "inactive" : "active",
          }),
        }
      );

      const data = await response.json();

      if (!response.ok) {
        throw new Error(
          data?.detail || "Failed to update employee."
        );
      }

      setEmployees((current) =>
        current.map((item) =>
          item.id === employee.id ? data : item
        )
      );

      setOpenMenuId(null);
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : "Failed to update employee."
      );
    }
  }

  const filteredEmployees = useMemo(() => {
    const query = search.trim().toLowerCase();

    return employees.filter((employee) => {
      const matchesSearch =
        !query ||
        employee.full_name.toLowerCase().includes(query) ||
        employee.email.toLowerCase().includes(query) ||
        (employee.department ?? "")
          .toLowerCase()
          .includes(query) ||
        (employee.job_title ?? "")
          .toLowerCase()
          .includes(query);

      const employeeStatus =
        employee.is_active &&
        employee.status.toLowerCase() === "active"
          ? "Active"
          : "Offline";

      const matchesStatus =
        status === "All" || employeeStatus === status;

      return matchesSearch && matchesStatus;
    });
  }, [employees, search, status]);

  return (
    <div className="rounded-2xl border border-zinc-200 bg-white">
      {/* Header */}
      <div className="flex flex-col gap-4 border-b border-zinc-100 p-5 lg:flex-row lg:items-center lg:justify-between">
        <div className="relative w-full lg:w-80">
          <Search
            size={17}
            className="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400"
          />

          <input
            type="text"
            placeholder="Search employees..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="h-10 w-full rounded-xl border border-zinc-200 bg-zinc-50 pl-10 pr-4 text-sm outline-none transition focus:border-zinc-400 focus:bg-white"
          />
        </div>

        <div className="flex items-center gap-2">
          {["All", "Active", "Offline"].map((item) => (
            <button
              key={item}
              type="button"
              onClick={() => setStatus(item)}
              className={`rounded-lg px-3 py-2 text-xs font-medium transition ${
                status === item
                  ? "bg-zinc-950 text-white"
                  : "border border-zinc-200 text-zinc-500 hover:bg-zinc-50"
              }`}
            >
              {item}
            </button>
          ))}
        </div>
      </div>

      {/* Loading */}
      {loading && (
        <div className="p-12 text-center">
          <p className="text-sm font-medium text-zinc-900">
            Loading employees...
          </p>

          <p className="mt-1 text-xs text-zinc-500">
            Fetching employees from GBOS.
          </p>
        </div>
      )}

      {/* Error */}
      {!loading && error && (
        <div className="p-12 text-center">
          <p className="text-sm font-medium text-red-600">
            Unable to load employees
          </p>

          <p className="mt-1 text-xs text-zinc-500">
            {error}
          </p>
        </div>
      )}

      {/* Table */}
      {!loading && !error && (
        <div className="overflow-x-auto lg:overflow-visible">
          <table className="w-full min-w-[850px]">
            <thead>
              <tr className="border-b border-zinc-100 text-left">
                <th className="px-6 py-4 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                  Employee
                </th>

                <th className="px-4 py-4 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                  Department
                </th>

                <th className="px-4 py-4 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                  Job Title
                </th>

                <th className="px-4 py-4 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                  Status
                </th>

                <th className="px-4 py-4 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                  Email
                </th>

                <th className="px-4 py-4" />
              </tr>
            </thead>

            <tbody className="divide-y divide-zinc-100">
              {filteredEmployees.map((employee, index) => {
                const employeeStatus =
                  employee.is_active &&
                  employee.status.toLowerCase() === "active"
                    ? "Active"
                    : "Offline";

                const initials = employee.full_name
                  .split(" ")
                  .filter(Boolean)
                  .slice(0, 2)
                  .map((part) => part[0])
                  .join("")
                  .toUpperCase();

                return (
                  <tr
                    key={employee.id}
                    className="group transition hover:bg-zinc-50"
                  >
                    {/* Employee */}
                    <td className="px-6 py-4">
                      <Link
                        href={`/employees/${employee.id}`}
                        className="block"
                      >
                        <div className="flex items-center gap-3">
                          <div className="flex h-10 w-10 items-center justify-center rounded-full bg-zinc-100 transition group-hover:bg-zinc-200">
                            <span className="text-xs font-semibold text-zinc-600">
                              {initials || "?"}
                            </span>
                          </div>

                          <div>
                            <p className="text-sm font-medium text-zinc-900 transition group-hover:text-zinc-950">
                              {employee.full_name}
                            </p>

                            <p className="mt-0.5 text-xs text-zinc-400">
                              EMP-{String(employee.id).padStart(3, "0")}
                            </p>
                          </div>
                        </div>
                      </Link>
                    </td>

                    {/* Department */}
                    <td className="px-4 py-4">
                      <span className="text-sm text-zinc-700">
                        {employee.department || "—"}
                      </span>
                    </td>

                    {/* Job Title */}
                    <td className="px-4 py-4">
                      <span className="text-sm text-zinc-600">
                        {employee.job_title || "—"}
                      </span>
                    </td>

                    {/* Status */}
                    <td className="px-4 py-4">
                      <div className="flex items-center gap-2">
                        {employeeStatus === "Active" ? (
                          <CheckCircle2
                            size={15}
                            className="text-emerald-500"
                          />
                        ) : (
                          <CircleAlert
                            size={15}
                            className="text-zinc-400"
                          />
                        )}

                        <span
                          className={`text-xs font-medium ${
                            employeeStatus === "Active"
                              ? "text-emerald-600"
                              : "text-zinc-500"
                          }`}
                        >
                          {employeeStatus}
                        </span>
                      </div>
                    </td>

                    {/* Email */}
                    <td className="px-4 py-4">
                      <span className="text-sm text-zinc-600">
                        {employee.email}
                      </span>
                    </td>

                    {/* Actions */}
                    <td className="relative px-4 py-4">
                      <button
                        type="button"
                        aria-label={`Actions for ${employee.full_name}`}
                        onClick={() =>
                          setOpenMenuId(
                            openMenuId === employee.id
                              ? null
                              : employee.id
                          )
                        }
                        className="flex h-8 w-8 items-center justify-center rounded-lg text-zinc-400 opacity-0 transition group-hover:opacity-100 hover:bg-zinc-100 hover:text-zinc-900"
                      >
                        <MoreHorizontal size={17} />
                      </button>

                      {openMenuId === employee.id && (
                        <div
                          className={`absolute right-4 z-30 w-48 rounded-xl border border-zinc-200 bg-white p-1.5 shadow-lg ${
                            index === filteredEmployees.length - 1
                              ? "bottom-12"
                              : "top-12"
                          }`}
                        >
                          <Link
                            href={`/employees/${employee.id}`}
                            onClick={() => setOpenMenuId(null)}
                            className="flex items-center gap-2 rounded-lg px-3 py-2 text-sm text-zinc-700 transition hover:bg-zinc-50"
                          >
                            <UserRound size={15} />
                            View employee
                          </Link>

                          <button
                            type="button"
                            onClick={() => {
                              setOpenMenuId(null);
                              window.dispatchEvent(
                                new CustomEvent("gbos:edit-employee", {
                                  detail: employee,
                                })
                              );
                            }}
                            className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left text-sm text-zinc-700 transition hover:bg-zinc-50"
                          >
                            <UserRoundCheck size={15} />
                            Edit employee
                          </button>

                          <div className="my-1 border-t border-zinc-100" />

                          <button
                            type="button"
                            onClick={() =>
                              toggleEmployeeActive(employee)
                            }
                            className="block w-full rounded-lg px-3 py-2 text-left text-sm text-zinc-700 transition hover:bg-zinc-50"
                          >
                            {employee.is_active
                              ? "Deactivate employee"
                              : "Activate employee"}
                          </button>
                        </div>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>

          {/* Empty */}
          {filteredEmployees.length === 0 && (
            <div className="p-12 text-center">
              <p className="text-sm font-medium text-zinc-900">
                No employees found
              </p>

              <p className="mt-1 text-xs text-zinc-500">
                Try changing your search or status filter.
              </p>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
