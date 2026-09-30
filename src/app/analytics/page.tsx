
import Sidebar from "@/components/layout/Sidebar";
import Topbar from "@/components/layout/Topbar";
import AnalyticsOverview from "@/components/analytics/AnalyticsOverview";

export default function AnalyticsPage() {
  return (
    <div className="min-h-screen bg-zinc-50">
      <Sidebar />
      <Topbar />

      <main className="ml-64 pt-20">
        <div className="space-y-6 p-8">
          <div>
            <p className="text-sm font-medium text-zinc-500">
              Performance Insights
            </p>

            <h1 className="mt-1 text-2xl font-semibold tracking-tight text-zinc-950">
              Analytics
            </h1>

            <p className="mt-2 text-sm text-zinc-500">
              Understand device performance, employee activity,
              and application usage across your organization.
            </p>
          </div>

          <AnalyticsOverview />
        </div>
      </main>
    </div>
  );
}
