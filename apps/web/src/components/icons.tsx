/** Small inline icon set — one visual language, no icon-font dependency. */
import type { SVGProps } from 'react';

type P = SVGProps<SVGSVGElement> & { size?: number };

function base({ size = 16, ...rest }: P, children: React.ReactNode) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.9}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      {...rest}
    >
      {children}
    </svg>
  );
}

export const IconPlus = (p: P) => base(p, <><path d="M12 5v14M5 12h14" /></>);
export const IconUndo = (p: P) => base(p, <><path d="M9 14 4 9l5-5" /><path d="M4 9h10a6 6 0 0 1 0 12h-3" /></>);
export const IconRedo = (p: P) => base(p, <><path d="m15 14 5-5-5-5" /><path d="M20 9H10a6 6 0 0 0 0 12h3" /></>);
export const IconEye = (p: P) => base(p, <><path d="M2 12s3.5-6 10-6 10 6 10 6-3.5 6-10 6S2 12 2 12Z" /><circle cx="12" cy="12" r="3" /></>);
export const IconEyeOff = (p: P) => base(p, <><path d="M3 3l18 18" /><path d="M10.6 5.2A10.9 10.9 0 0 1 12 5c6.5 0 10 7 10 7a17 17 0 0 1-3 3.9" /><path d="M6.6 6.6C3.7 8.6 2 12 2 12s3.5 7 10 7a9.7 9.7 0 0 0 4.4-1" /><path d="M9.9 9.9a3 3 0 0 0 4.2 4.2" /></>);
export const IconLock = (p: P) => base(p, <><rect x="5" y="11" width="14" height="10" rx="2" /><path d="M8 11V7a4 4 0 0 1 8 0v4" /></>);
export const IconUnlock = (p: P) => base(p, <><rect x="5" y="11" width="14" height="10" rx="2" /><path d="M8 11V7a4 4 0 0 1 7.5-2" /></>);
export const IconTrash = (p: P) => base(p, <><path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3" /></>);
export const IconCopy = (p: P) => base(p, <><rect x="9" y="9" width="12" height="12" rx="2" /><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" /></>);
export const IconSun = (p: P) => base(p, <><circle cx="12" cy="12" r="4" /><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" /></>);
export const IconMoon = (p: P) => base(p, <><path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8Z" /></>);
export const IconMonitor = (p: P) => base(p, <><rect x="3" y="4" width="18" height="12" rx="2" /><path d="M8 20h8M12 16v4" /></>);
export const IconHelp = (p: P) => base(p, <><circle cx="12" cy="12" r="9" /><path d="M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.7.4-1 .9-1 1.7M12 17h.01" /></>);
export const IconFit = (p: P) => base(p, <><path d="M8 3H5a2 2 0 0 0-2 2v3M16 3h3a2 2 0 0 1 2 2v3M8 21H5a2 2 0 0 1-2-2v-3M16 21h3a2 2 0 0 0 2-2v-3" /><rect x="8" y="9" width="8" height="6" rx="1" /></>);
export const IconGrid = (p: P) => base(p, <><path d="M3 9h18M3 15h18M9 3v18M15 3v18" /></>);
export const IconPlay = (p: P) => base(p, <><path d="M7 4v16l13-8Z" /></>);
export const IconStop = (p: P) => base(p, <><rect x="6" y="6" width="12" height="12" rx="1.5" /></>);
export const IconDownload = (p: P) => base(p, <><path d="M12 3v12M7 10l5 5 5-5M4 21h16" /></>);
export const IconUpload = (p: P) => base(p, <><path d="M12 15V3M7 8l5-5 5 5M4 21h16" /></>);
export const IconFolder = (p: P) => base(p, <><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" /></>);
export const IconSave = (p: P) => base(p, <><path d="M5 3h11l3 3v15H5Z" /><path d="M8 3v6h7V3M8 21v-6h8v6" /></>);
export const IconChevronDown = (p: P) => base(p, <><path d="m6 9 6 6 6-6" /></>);
export const IconChevronRight = (p: P) => base(p, <><path d="m9 6 6 6-6 6" /></>);
export const IconClose = (p: P) => base(p, <><path d="M6 6l12 12M18 6 6 18" /></>);
export const IconPointer = (p: P) => base(p, <><path d="M5 3l14 8-6 2-3 6Z" /></>);
export const IconHand = (p: P) => base(p, <><path d="M8 13V5.5a1.5 1.5 0 0 1 3 0V12M11 11V4.5a1.5 1.5 0 0 1 3 0V12M14 12V6.5a1.5 1.5 0 0 1 3 0V13" /><path d="M17 13v1a6 6 0 0 1-6 6h-1a5 5 0 0 1-4.2-2.3L3.4 14a1.5 1.5 0 0 1 2.4-1.7L8 14" /></>);
export const IconSeed = (p: P) => base(p, <><circle cx="12" cy="12" r="2.5" /><path d="M3 12c3-4 6-4 9 0s6 4 9 0" /></>);
export const IconLayers = (p: P) => base(p, <><path d="m12 3 9 5-9 5-9-5Z" /><path d="m3 13 9 5 9-5M3 17l9 5 9-5" /></>);
export const IconInfo = (p: P) => base(p, <><circle cx="12" cy="12" r="9" /><path d="M12 11v5M12 8h.01" /></>);
export const IconWarning = (p: P) => base(p, <><path d="M12 3 2.5 20h19Z" /><path d="M12 10v4M12 17h.01" /></>);
export const IconCheck = (p: P) => base(p, <><path d="m5 12 5 5L20 7" /></>);
export const IconSidebarLeft = (p: P) => base(p, <><rect x="3" y="4" width="18" height="16" rx="2" /><path d="M9 4v16" /></>);
export const IconSidebarRight = (p: P) => base(p, <><rect x="3" y="4" width="18" height="16" rx="2" /><path d="M15 4v16" /></>);
export const IconPanelBottom = (p: P) => base(p, <><rect x="3" y="4" width="18" height="16" rx="2" /><path d="M3 14h18" /></>);
export const IconChart = (p: P) => base(p, <><path d="M3 20h18M6 17V10M11 17V5M16 17v-7" /></>);
export const IconMore = (p: P) => base(p, <><circle cx="12" cy="5" r="1.2" /><circle cx="12" cy="12" r="1.2" /><circle cx="12" cy="19" r="1.2" /></>);
export const IconRotate = (p: P) => base(p, <><path d="M20 12a8 8 0 1 1-2.3-5.7" /><path d="M20 4v5h-5" /></>);

export const IconBook = (p: P) =>
  base(p, <><path d="M4 5.5A2.5 2.5 0 0 1 6.5 3H20v16H6.5A2.5 2.5 0 0 0 4 21.5z" /><path d="M4 21.5V5.5" /><path d="M9 8h7" /></>);
