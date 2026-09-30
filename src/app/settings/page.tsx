
import Sidebar from "@/components/layout/Sidebar";
import Topbar from "@/components/layout/Topbar";
import SettingsPanel from "@/components/settings/SettingsPanel";

export default function SettingsPage() {
  return (
    <div className="min-h-screen bg-zinc-50">
      <Sidebar />
      <Topbar />

      <main className="ml-64 pt-20">
        <div className="space-y-6 p-8">
          <div>
            <p className="text-sm font-medium text-zinc-500">
              Platform Configuration
            </p>

            <h1 className="mt-1 text-2xl font-semibold tracking-tight text-zinc-950">
              Settings
            </h1>

            <p className="mt-2 text-sm text-zinc-500">
              Manage your organization, monitoring preferences,
              alerts, and security settings.
            </p>
          </div>

          <SettingsPanel />
        </div>
      </main>
    </div>
  );
}

