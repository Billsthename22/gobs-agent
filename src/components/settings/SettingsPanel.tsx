"use client";

import { useState } from "react";
import {
  Bell,
  Building2,
  Check,
  Lock,
  Monitor,
  Save,
  ShieldCheck,
} from "lucide-react";

function Toggle({
  enabled,
  onChange,
}: {
  enabled: boolean;
  onChange: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onChange}
      className={`relative h-6 w-11 rounded-full transition ${
        enabled ? "bg-zinc-950" : "bg-zinc-200"
      }`}
      aria-label="Toggle setting"
    >
      <span
        className={`absolute top-1 h-4 w-4 rounded-full bg-white transition ${
          enabled ? "left-6" : "left-1"
        }`}
      />
    </button>
  );
}

export default function SettingsPanel() {
  const [monitoring, setMonitoring] = useState(true);
  const [idleTracking, setIdleTracking] = useState(true);
  const [deviceAlerts, setDeviceAlerts] = useState(true);
  const [emailAlerts, setEmailAlerts] = useState(false);
  const [securityAlerts, setSecurityAlerts] = useState(true);

  return (
    <div className="grid gap-6 xl:grid-cols-[220px_1fr]">
      {/* Settings Navigation */}
      <div className="h-fit rounded-2xl border border-zinc-200 bg-white p-2">
        <div className="space-y-1">
          <a
            href="#general"
            className="flex items-center gap-3 rounded-xl bg-zinc-100 px-3 py-2.5 text-sm font-medium text-zinc-950"
          >
            <Building2 size={16} />
            General
          </a>

          <a
            href="#monitoring"
            className="flex items-center gap-3 rounded-xl px-3 py-2.5 text-sm text-zinc-500 transition hover:bg-zinc-50 hover:text-zinc-950"
          >
            <Monitor size={16} />
            Monitoring
          </a>

          <a
            href="#alerts"
            className="flex items-center gap-3 rounded-xl px-3 py-2.5 text-sm text-zinc-500 transition hover:bg-zinc-50 hover:text-zinc-950"
          >
            <Bell size={16} />
            Alerts
          </a>

          <a
            href="#security"
            className="flex items-center gap-3 rounded-xl px-3 py-2.5 text-sm text-zinc-500 transition hover:bg-zinc-50 hover:text-zinc-950"
          >
            <ShieldCheck size={16} />
            Security
          </a>
        </div>
      </div>

      {/* Settings Content */}
      <div className="space-y-6">
        {/* General */}
        <section
          id="general"
          className="rounded-2xl border border-zinc-200 bg-white"
        >
          <div className="border-b border-zinc-100 px-6 py-5">
            <h2 className="text-sm font-semibold text-zinc-950">
              General Settings
            </h2>
            <p className="mt-1 text-xs text-zinc-500">
              Basic information about your organization.
            </p>
          </div>

          <div className="grid gap-5 p-6 md:grid-cols-2">
            <div>
              <label className="text-xs font-medium text-zinc-700">
                Organization Name
              </label>
              <input
                defaultValue="Gbaja Partners"
                className="mt-2 h-10 w-full rounded-xl border border-zinc-200 bg-white px-3 text-sm text-zinc-900 outline-none transition placeholder:text-zinc-400 focus:border-zinc-400"
              />
            </div>

            <div>
              <label className="text-xs font-medium text-zinc-700">
                Organization ID
              </label>
              <input
                defaultValue="GBJ-001"
                disabled
                className="mt-2 h-10 w-full rounded-xl border border-zinc-200 bg-zinc-50 px-3 text-sm text-zinc-500 outline-none"
              />
            </div>

            <div>
              <label className="text-xs font-medium text-zinc-700">
                Admin Email
              </label>
              <input
                type="email"
                defaultValue="admin@gbajapartners.com"
                className="mt-2 h-10 w-full rounded-xl border border-zinc-200 bg-white px-3 text-sm text-zinc-900 outline-none transition focus:border-zinc-400"
              />
            </div>

            <div>
              <label className="text-xs font-medium text-zinc-700">
                Timezone
              </label>

              <select
                defaultValue="Africa/Lagos"
                className="mt-2 h-10 w-full rounded-xl border border-zinc-200 bg-white px-3 text-sm text-zinc-900 outline-none focus:border-zinc-400"
              >
                <option value="Africa/Lagos">
                  Africa/Lagos (WAT)
                </option>
                <option value="UTC">UTC</option>
                <option value="Europe/London">
                  Europe/London
                </option>
              </select>
            </div>
          </div>

          <div className="flex justify-end border-t border-zinc-100 px-6 py-4">
            <button
              type="button"
              className="flex items-center gap-2 rounded-xl bg-zinc-950 px-4 py-2.5 text-xs font-medium text-white transition hover:bg-zinc-800"
            >
              <Save size={14} />
              Save Changes
            </button>
          </div>
        </section>

        {/* Monitoring */}
        <section
          id="monitoring"
          className="rounded-2xl border border-zinc-200 bg-white"
        >
          <div className="border-b border-zinc-100 px-6 py-5">
            <h2 className="text-sm font-semibold text-zinc-950">
              Monitoring
            </h2>
            <p className="mt-1 text-xs text-zinc-500">
              Configure authorized device and workforce telemetry.
            </p>
          </div>

          <div className="divide-y divide-zinc-100">
            <div className="flex items-center justify-between gap-6 px-6 py-5">
              <div>
                <p className="text-sm font-medium text-zinc-900">
                  Device Monitoring
                </p>
                <p className="mt-1 text-xs text-zinc-500">
                  Collect device health and connection status.
                </p>
              </div>

              <Toggle
                enabled={monitoring}
                onChange={() => setMonitoring(!monitoring)}
              />
            </div>

            <div className="flex items-center justify-between gap-6 px-6 py-5">
              <div>
                <p className="text-sm font-medium text-zinc-900">
                  Idle Detection
                </p>
                <p className="mt-1 text-xs text-zinc-500">
                  Track active and idle session states.
                </p>
              </div>

              <Toggle
                enabled={idleTracking}
                onChange={() => setIdleTracking(!idleTracking)}
              />
            </div>

            <div className="px-6 py-5">
              <p className="text-sm font-medium text-zinc-900">
                Heartbeat Interval
              </p>
              <p className="mt-1 text-xs text-zinc-500">
                How frequently managed devices report their status.
              </p>

              <select
                defaultValue="30"
                className="mt-3 h-10 w-full max-w-xs rounded-xl border border-zinc-200 bg-white px-3 text-sm text-zinc-900 outline-none focus:border-zinc-400"
              >
                <option value="15">Every 15 seconds</option>
                <option value="30">Every 30 seconds</option>
                <option value="60">Every 1 minute</option>
                <option value="300">Every 5 minutes</option>
              </select>
            </div>
          </div>
        </section>

        {/* Alerts */}
        <section
          id="alerts"
          className="rounded-2xl border border-zinc-200 bg-white"
        >
          <div className="border-b border-zinc-100 px-6 py-5">
            <h2 className="text-sm font-semibold text-zinc-950">
              Alert Preferences
            </h2>
            <p className="mt-1 text-xs text-zinc-500">
              Choose which events should generate notifications.
            </p>
          </div>

          <div className="divide-y divide-zinc-100">
            <div className="flex items-center justify-between gap-6 px-6 py-5">
              <div>
                <p className="text-sm font-medium text-zinc-900">
                  Device Alerts
                </p>
                <p className="mt-1 text-xs text-zinc-500">
                  Notify when devices go offline or report health issues.
                </p>
              </div>

              <Toggle
                enabled={deviceAlerts}
                onChange={() => setDeviceAlerts(!deviceAlerts)}
              />
            </div>

            <div className="flex items-center justify-between gap-6 px-6 py-5">
              <div>
                <p className="text-sm font-medium text-zinc-900">
                  Email Notifications
                </p>
                <p className="mt-1 text-xs text-zinc-500">
                  Send important alerts to the administrator email.
                </p>
              </div>

              <Toggle
                enabled={emailAlerts}
                onChange={() => setEmailAlerts(!emailAlerts)}
              />
            </div>

            <div className="flex items-center justify-between gap-6 px-6 py-5">
              <div>
                <p className="text-sm font-medium text-zinc-900">
                  Security Alerts
                </p>
                <p className="mt-1 text-xs text-zinc-500">
                  Notify about suspicious authentication and access events.
                </p>
              </div>

              <Toggle
                enabled={securityAlerts}
                onChange={() => setSecurityAlerts(!securityAlerts)}
              />
            </div>
          </div>
        </section>

        {/* Security */}
        <section
          id="security"
          className="rounded-2xl border border-zinc-200 bg-white"
        >
          <div className="border-b border-zinc-100 px-6 py-5">
            <h2 className="text-sm font-semibold text-zinc-950">
              Security
            </h2>
            <p className="mt-1 text-xs text-zinc-500">
              Manage administrator access and session security.
            </p>
          </div>

          <div className="divide-y divide-zinc-100">
            <div className="flex items-center gap-4 px-6 py-5">
              <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-zinc-100">
                <Lock size={17} className="text-zinc-700" />
              </div>

              <div className="flex-1">
                <p className="text-sm font-medium text-zinc-900">
                  Administrator Password
                </p>
                <p className="mt-1 text-xs text-zinc-500">
                  Last changed 30 days ago.
                </p>
              </div>

              <button
                type="button"
                className="rounded-xl border border-zinc-200 px-3 py-2 text-xs font-medium text-zinc-700 transition hover:bg-zinc-50"
              >
                Change
              </button>
            </div>

            <div className="flex items-center gap-4 px-6 py-5">
              <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-emerald-50">
                <Check size={17} className="text-emerald-600" />
              </div>

              <div>
                <p className="text-sm font-medium text-zinc-900">
                  Two-Factor Authentication
                </p>
                <p className="mt-1 text-xs text-emerald-600">
                  Enabled
                </p>
              </div>
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}

