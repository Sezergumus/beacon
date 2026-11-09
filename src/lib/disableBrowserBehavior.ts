/**
 * Tarayıcıya özgü davranışları kapatır, böylece uygulama native bir masaüstü
 * uygulaması gibi hissettirir: yenileme, sağ tık menüsü, yazdırma, kaynak
 * görüntüleme, sayfada arama vb.
 *
 * - Yenileme / yazdırma / kaynak görüntüleme / sayfada arama kısayolları her
 *   zaman engellenir.
 * - DevTools kısayolları (F12, Ctrl+Shift+I/J/C) sadece geliştirme modunda açık
 *   bırakılır.
 * - Metin alanlarında (input/textarea/contenteditable) sağ tık menüsü açık kalır;
 *   kopyala/yapıştır için gerekli.
 */
export function disableBrowserBehavior(): void {
  const isDev = import.meta.env.DEV;

  const isEditable = (target: EventTarget | null): boolean => {
    if (!(target instanceof HTMLElement)) return false;
    return (
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement ||
      target.isContentEditable
    );
  };

  window.addEventListener(
    "contextmenu",
    (e) => {
      if (!isEditable(e.target)) e.preventDefault();
    },
    true,
  );

  window.addEventListener(
    "keydown",
    (e) => {
      const key = e.key.toLowerCase();
      const ctrl = e.ctrlKey || e.metaKey;

      const blocked =
        // Yenileme
        key === "f5" ||
        (ctrl && key === "r") ||
        // Yazdır, kaynağı görüntüle, kaydet, sayfada ara, git
        (ctrl && ["p", "u", "s", "f", "g", "j"].includes(key) && !e.shiftKey) ||
        // Geri / ileri
        (e.altKey && (key === "arrowleft" || key === "arrowright")) ||
        // Yakınlaştırma
        (ctrl && ["+", "-", "=", "0"].includes(key)) ||
        // DevTools (sadece production'da)
        (!isDev &&
          (key === "f12" ||
            (ctrl && e.shiftKey && ["i", "j", "c"].includes(key))));

      if (blocked) {
        e.preventDefault();
        e.stopPropagation();
      }
    },
    true,
  );

  // Ctrl + fare tekerleği ile yakınlaştırmayı engelle
  window.addEventListener(
    "wheel",
    (e) => {
      if (e.ctrlKey) e.preventDefault();
    },
    { passive: false, capture: true },
  );
}
