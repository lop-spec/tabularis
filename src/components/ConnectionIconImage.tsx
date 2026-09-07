import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { appDataDir, join } from "@tauri-apps/api/path";

interface Props {
  path: string;       // relative path "connection-icons/foo-abcd.png"
  size: number;
  fallback: React.ReactNode;
}

/**
 * Outer wrapper that keys on `path` so React unmounts/remounts the inner
 * component whenever the path changes — cleanly resetting all internal
 * state without calling setState during render or inside an effect body.
 */
export function ConnectionIconImage(props: Props) {
  return <ConnectionIconImageInner key={props.path} {...props} />;
}

function ConnectionIconImageInner({ path, size, fallback }: Props) {
  const [src, setSrc] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const abs = await join(await appDataDir(), path);
        if (!cancelled) setSrc(convertFileSrc(abs));
      } catch (error) {
        console.error("[ConnectionIconImage] Cannot resolve icon; using fallback", error);
        if (!cancelled) setFailed(true);
      }
    })();
    return () => { cancelled = true; };
  }, [path]);

  if (failed) return <>{fallback}</>;
  if (!src) return <>{fallback}</>;
  return (
    <img
      src={src}
      width={size}
      height={size}
      alt=""
      onError={() => {
        console.error("[ConnectionIconImage] Icon load failed; using fallback");
        setFailed(true);
      }}
      style={{ objectFit: "contain", borderRadius: 4 }}
    />
  );
}
