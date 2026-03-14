"use client";

import { useRef, useState, useEffect, useCallback } from "react";

interface LightboxState {
  src: string;
  alt: string;
  images: { src: string; alt: string }[];
  index: number;
}

export function WikiContent({ html }: { html: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const [lightbox, setLightbox] = useState<LightboxState | null>(null);

  useEffect(() => {
    if (!ref.current) return;
    const imgs = ref.current.querySelectorAll<HTMLImageElement>(
      "img:not([src*='shields.io']):not([src*='badge'])"
    );
    const imageList = Array.from(imgs).map((img) => ({
      src: img.getAttribute("src") ?? img.src,
      alt: img.alt,
    }));

    imgs.forEach((img, i) => {
      img.style.cursor = "zoom-in";
      img.addEventListener("click", () => {
        setLightbox({ src: imageList[i].src, alt: imageList[i].alt, images: imageList, index: i });
      });
    });
  }, [html]);

  const close = useCallback(() => setLightbox(null), []);

  const prev = useCallback(() => {
    setLightbox((lb) => {
      if (!lb || lb.images.length <= 1) return lb;
      const index = (lb.index - 1 + lb.images.length) % lb.images.length;
      return { ...lb, index, src: lb.images[index].src, alt: lb.images[index].alt };
    });
  }, []);

  const next = useCallback(() => {
    setLightbox((lb) => {
      if (!lb || lb.images.length <= 1) return lb;
      const index = (lb.index + 1) % lb.images.length;
      return { ...lb, index, src: lb.images[index].src, alt: lb.images[index].alt };
    });
  }, []);

  useEffect(() => {
    if (!lightbox) return;
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
      else if (e.key === "ArrowLeft") prev();
      else if (e.key === "ArrowRight") next();
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [lightbox, close, prev, next]);

  return (
    <>
      <article
        ref={ref}
        className="post-content"
        dangerouslySetInnerHTML={{ __html: html }}
      />
      {lightbox && (
        <div className="lightbox-overlay active" onClick={close}>
          {lightbox.images.length > 1 && (
            <button
              className="lightbox-nav lightbox-prev"
              onClick={(e) => { e.stopPropagation(); prev(); }}
              aria-label="Immagine precedente"
            >
              &#8592;
            </button>
          )}
          <img
            src={lightbox.src}
            alt={lightbox.alt}
            className="lightbox-img"
            onClick={(e) => e.stopPropagation()}
          />
          {lightbox.images.length > 1 && (
            <button
              className="lightbox-nav lightbox-next"
              onClick={(e) => { e.stopPropagation(); next(); }}
              aria-label="Immagine successiva"
            >
              &#8594;
            </button>
          )}
          <button className="lightbox-close" onClick={close}>
            &times;
          </button>
          {lightbox.images.length > 1 && (
            <div className="lightbox-footer">
              <span className="lightbox-counter">
                {lightbox.index + 1}&thinsp;/&thinsp;{lightbox.images.length}
              </span>
            </div>
          )}
        </div>
      )}
    </>
  );
}
