import logo from "../assets/logo.svg";

/** Shared vector mark used by the editor, favicon and desktop icons. */
export function BrandMark({ size = 32 }: { size?: number }) {
  return <img src={logo} width={size} height={size} alt="" draggable={false} className="shrink-0" />;
}
