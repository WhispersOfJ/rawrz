import type { ComponentType } from "react";
import { NavLink, Route, Routes } from "react-router-dom";
import {
  Dashboard, Containers, Catalog, Credentials, Env, Unstick,
  Library, Watchable, Plex, Nzbdav, Host, Settings,
} from "./pages";

const NAV: Array<{ to: string; label: string; icon: string; Page: ComponentType }> = [
  { to: "/", label: "Dashboard", icon: "⌂", Page: Dashboard },
  { to: "/containers", label: "Containers", icon: "▦", Page: Containers },
  { to: "/catalog", label: "Catalog", icon: "◈", Page: Catalog },
  { to: "/credentials", label: "Credentials", icon: "▣", Page: Credentials },
  { to: "/env", label: "Env", icon: "⚙", Page: Env },
  { to: "/unstick", label: "Unstick", icon: "⚠", Page: Unstick },
  { to: "/library", label: "Library", icon: "▤", Page: Library },
  { to: "/watchable", label: "Watchable", icon: "▶", Page: Watchable },
  { to: "/plex", label: "Plex", icon: "●", Page: Plex },
  { to: "/nzbdav", label: "nzbdav", icon: "⇩", Page: Nzbdav },
  { to: "/host", label: "Host", icon: "⌘", Page: Host },
  { to: "/settings", label: "Settings", icon: "⚙", Page: Settings },
];

function Brand() {
  return (
    <div className="cd-brand">
      <div className="cd-brand-mark" aria-hidden="true">B</div>
      <div>
        <strong>Cave Deck</strong>
        <span>Bear Cave control room</span>
      </div>
    </div>
  );
}

function NavItems() {
  return (
    <div className="cd-nav-items">
      {NAV.map((item) => (
        <NavLink
          key={item.to}
          to={item.to}
          end={item.to === "/"}
          className={({ isActive }) => isActive ? "cd-nav-link active" : "cd-nav-link"}
        >
          <span className="cd-nav-icon" aria-hidden="true">{item.icon}</span>
          <span>{item.label}</span>
        </NavLink>
      ))}
    </div>
  );
}

export default function App() {
  return (
    <div className="cd-app">
      <aside className="cd-sidebar" aria-label="Primary">
        <Brand />
        <NavItems />
        <div className="cd-sidebar-footer">
          <span className="cd-live-dot" aria-hidden="true" />
          <span>LAN connection</span>
        </div>
      </aside>
      <div className="cd-content-shell">
        <header className="cd-mobile-header">
          <Brand />
          <span className="cd-live-pill"><span className="cd-live-dot" /> LAN only</span>
        </header>
        <main className="cd-main">
          <Routes>
            {NAV.map((item) => (
              <Route key={item.to} path={item.to} element={<item.Page />} />
            ))}
          </Routes>
        </main>
      </div>
    </div>
  );
}
