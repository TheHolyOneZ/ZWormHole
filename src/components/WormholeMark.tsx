

import logo from "../assets/logo.png";

interface WormholeMarkProps {
  size?: number;
  spin?: boolean;
}

export function WormholeMark({ size = 20, spin = false }: WormholeMarkProps) {
  return (
    <img
      src={logo}
      width={size}
      height={size}
      className={spin ? "wormhole-mark wormhole-mark--spin" : "wormhole-mark"}
      alt=""
      aria-hidden
      draggable={false}
    />
  );
}
