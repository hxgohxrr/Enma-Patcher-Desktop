import type { ComponentType } from "react";

export type MarkProps = { size?: number; className?: string };
export type Mark = ComponentType<MarkProps>;

export function AndroidMark(props: MarkProps) {
  const s = props.size ?? 17;
  return (
    <svg width={s} height={s} viewBox="0 0 24 24" className={props.className} aria-hidden="true">
      <g stroke="currentColor" strokeWidth={1.9} strokeLinecap="round" fill="none">
        <path d="M8.4 3.9 9.9 5.7" />
        <path d="M15.6 3.9 14.1 5.7" />
        <path d="M4.9 12.6v3.1" />
        <path d="M19.1 12.6v3.1" />
        <path d="M10 18.4v2.1" />
        <path d="M14 18.4v2.1" />
      </g>
      <path
        fill="currentColor"
        fillRule="evenodd"
        d="M7 11V8a5 5 0 0 1 10 0v3H7zM9.3 8a1 1 0 1 0 2 0 1 1 0 1 0-2 0zM12.7 8a1 1 0 1 0 2 0 1 1 0 1 0-2 0z"
      />
      <rect x="7.5" y="12" width="9" height="5.2" rx="2.6" fill="currentColor" />
    </svg>
  );
}

export function AppleMark(props: MarkProps) {
  const s = props.size ?? 16;
  return (
    <svg width={s} height={s} viewBox="0 0 384 512" fill="currentColor" className={props.className} aria-hidden="true">
      <path d="M318.7 268.7c-.2-36.7 16.4-64.4 50-84.8-18.8-26.9-47.2-41.7-84.7-44.6-35.5-2.8-74.3 20.7-88.5 20.7-15 0-49.4-19.7-76.4-19.7C63.3 141.2 4 184.8 4 273.5q0 39.3 14.4 81.2c12.8 36.7 59 126.7 107.2 125.2 25.2-.6 43-17.9 75.8-17.9 31.8 0 48.3 17.9 76.4 17.9 48.6-.7 90.4-82.5 102.6-119.3-65.2-30.7-61.7-90-61.7-91.9zm-56.6-164.2c27.3-32.4 24.8-61.9 24-72.5-24.1 1.4-52 16.4-67.9 34.9-17.5 19.8-27.8 44.3-25.6 71.9 26.1 2 49.9-11.4 69.5-34.3z" />
    </svg>
  );
}
