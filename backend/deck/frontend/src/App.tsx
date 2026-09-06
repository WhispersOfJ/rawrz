import { NavLink, Route, Routes } from "react-router-dom";
import {
  Dashboard, Containers, Catalog, Credentials, Env, Unstick,
  Library, Watchable, Plex, Nzbdav, Host, Settings,
} from "./pages";

const NAV = [
  { to: "/", label: "Dashboard", el: <Dashboard /> },
  { to: "/containers", label: "Containers", el: <Containers /> },
  { to: "/catalog", label: "Catalog", el: <Catalog /> },
  { to: "/credentials", label: "Credentials", el: <Credentials /> },
  { to: "/env", label: "Env", el: <Env /> },
  { to: "/unstick", label: "Unstick", el: <Unstick /> },
  { to: "/library", label: "Library", el: <Library /> },
  { to: "/watchable", label: "Watchable", el: <Watchable /> },
  { to: "/plex", label: "Plex", el: <Plex /> },
  { to: "/nzbdav", label: "nzbdav", el: <Nzbdav /> },
  { to: "/host", label: "Host", el: <Host /> },
  { to: "/settings", label: "Settings", el: <Settings /> },
];

export default function App() {
  return (
    <div className="cd-layout">
      <nav className="cd-sidebar" aria-label="Primary">
        <NavLink to="/" end>Dashboard</NavLink>
        {NAV.slice(1).map((n) => (
          <NavLink key={n.to} to={n.to}>{n.label}</NavLink>
        ))}
      </nav>
      <main className="cd-main">
        <Routes>
          {NAV.map((n) => (
            <Route key={n.to} path={n.to} element={n.el} />
          ))}
        </Routes>
      </main>
    </div>
  );
}
