import { useEffect, useRef, useState } from "react";
import logoUrl from "../assets/enma-logo.png";
import "./Intro.css";

export function Intro(props: { onDone: () => void }) {
  const [leaving, setLeaving] = useState(false);
  const doneRef = useRef(props.onDone);
  doneRef.current = props.onDone;

  useEffect(() => {
    const t1 = window.setTimeout(() => setLeaving(true), 900);
    const t2 = window.setTimeout(() => doneRef.current(), 1200);
    return () => {
      window.clearTimeout(t1);
      window.clearTimeout(t2);
    };
  }, []);

  function skip() {
    setLeaving(true);
    window.setTimeout(() => doneRef.current(), 200);
  }

  return (
    <div
      className={[
        "intro2",
        "bg-ink-50 dark:bg-ink-950",
        leaving ? "intro2-leaving" : "",
      ].join(" ")}
      onClick={skip}
      role="presentation"
    >
      <div className="intro2-glow" />
      <img src={logoUrl} alt="Enma Patcher" className="intro2-logo" draggable={false} />
    </div>
  );
}
