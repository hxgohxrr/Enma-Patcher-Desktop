import type { ReactNode } from "react";
import { motion } from "motion/react";

export function Enter(props: {
  play: boolean;
  delay?: number;
  y?: number;
  className?: string;
  children: ReactNode;
}) {
  const y = props.y ?? 10;
  return (
    <motion.div
      className={props.className}
      initial={
        props.play
          ? { opacity: 0, transform: `translateY(${y}px) scale(0.99)` }
          : false
      }
      animate={{ opacity: 1, transform: "translateY(0px) scale(1)" }}
      transition={{
        type: "spring",
        duration: 0.45,
        bounce: 0.2,
        delay: props.play ? props.delay ?? 0 : 0,
      }}
    >
      {props.children}
    </motion.div>
  );
}
