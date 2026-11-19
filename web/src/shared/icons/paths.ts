export type IconName =
  | "overview"
  | "assets"
  | "planning"
  | "deliverables"
  | "automation"
  | "settings"
  | "search"
  | "lock"
  | "sun"
  | "moon"
  | "chevron-right"
  | "arrow-right"
  | "check"
  | "alert"
  | "clock"
  | "close"
  | "plus"
  | "command"
  | "shield"
  | "database"
  | "agent"
  | "refresh"
  | "more"
  | "external"
  | "menu"
  | "info";

export const paths: Record<IconName, string[]> = {
  overview: ["M4 4h6v6H4z", "M14 4h6v6h-6z", "M4 14h6v6H4z", "M14 14h6v6h-6z"],
  assets: ["M12 3 3 8l9 5 9-5-9-5z", "M3 13l9 5 9-5", "M3 18l9 5 9-5"],
  planning: ["M4 18h4l3-9 4 6h5", "M4 6v14"],
  deliverables: ["M6 3h8l4 4v14H6z", "M14 3v5h5"],
  automation: ["M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8z", "M19 3v3m0 12v3"],
  settings: ["M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8z", "M4 12h2", "M18 12h2", "M12 4v2", "M12 18v2"],
  search: ["M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14z", "M16 16l4 4"],
  lock: ["M6 10h12v10H6z", "M9 10V7a3 3 0 0 1 6 0v3"],
  sun: ["M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8z", "M12 2v2", "M12 20v2", "M2 12h2", "M20 12h2", "M5 5l1.4 1.4", "M17.6 17.6L19 19", "M19 5l-1.4 1.4", "M6.4 17.6L5 19"],
  moon: ["M20 14.5A8 8 0 0 1 9.5 4 8 8 0 1 0 10.5 16.5"],
  "chevron-right": ["M9 5l7 7-7 7"],
  "arrow-right": ["M5 12h14", "M13 6l6 6-6 6"],
  check: ["M5 12l4 4L19 6"],
  alert: ["M12 3L2 20h20L12 3z", "M12 9v5", "M12 17h.01"],
  clock: ["M12 4a8 8 0 1 0 0 16 8 8 0 0 0 0-16z", "M12 8v4l3 3"],
  close: ["M6 6l12 12", "M18 6L6 18"],
  plus: ["M12 5v14", "M5 12h14"],
  command: ["M8 9a3 3 0 1 0-3-3h14a3 3 0 1 0-3 3v6a3 3 0 1 0 3 3H5a3 3 0 1 0 3-3z"],
  shield: ["M12 3l7 3v6c0 5-3 7-7 9-4-2-7-4-7-9V6z"],
  database: ["M4 6c0-2 4-3 8-3s8 1 8 3-4 3-8 3-8-1-8-3z", "M4 6v6c0 2 4 3 8 3s8-1 8-3V6", "M4 12v6c0 2 4 3 8 3s8-1 8-3v-6"],
  agent: ["M8 4h8v4H8z", "M6 8h12v12H6z", "M9 12h.01", "M15 12h.01", "M9 16h6"],
  refresh: ["M4 12a8 8 0 0 1 13-6l3 3", "M20 12a8 8 0 0 1-13 6l-3-3", "M17 3v6h-6", "M7 21v-6h6"],
  more: ["M5 12h.01", "M12 12h.01", "M19 12h.01"],
  external: ["M14 4h6v6", "M20 4L10 14", "M18 14v6H4V6h6"],
  menu: ["M4 7h16", "M4 12h16", "M4 17h16"],
  info: ["M12 4a8 8 0 1 0 0 16 8 8 0 0 0 0-16z", "M12 10v6", "M12 7h.01"],
};
