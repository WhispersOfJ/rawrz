import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import App from "../src/App";

function renderApp(initial: string) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[initial]}>
        <App />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("Cave Deck shell", () => {
  it("renders the sidebar navigation with all 12 areas", () => {
    renderApp("/");
    for (const label of [
      "Dashboard", "Containers", "Catalog", "Credentials", "Env",
      "Unstick", "Library", "Watchable", "Plex", "nzbdav",
      "Host", "Settings",
    ]) {
      // nav link + page heading can share the label — assert presence, not uniqueness
      expect(screen.getAllByText(label).length).toBeGreaterThan(0);
    }
  });

  it("shows the M0 skeleton banner on the dashboard", async () => {
    renderApp("/");
    expect(await screen.findByText(/M0 skeleton/i)).toBeDefined();
  });
});
