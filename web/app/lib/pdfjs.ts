export interface PdfPageRender {
  pageNumber: number;
  totalPages?: number;
  width: number;
  height: number;
  rotation: number;
  dataUrl: string;
}

let pdfjsPromise: Promise<any> | null = null;

async function getPdfJs() {
  if (!pdfjsPromise) {
    const path = "/pdfjs/pdf.mjs";
    pdfjsPromise = import(/* @vite-ignore */ path).then((mod) => {
      mod.GlobalWorkerOptions.workerSrc = "/pdfjs/pdf.worker.mjs";
      return mod;
    });
  }
  return pdfjsPromise;
}

export async function renderPdfPages(
  file: File,
  maxPages = 24,
  targetWidth = 220,
): Promise<{ totalPages: number; pages: PdfPageRender[] }> {
  const pdfjs = await getPdfJs();
  const bytes = await file.arrayBuffer();
  const task = pdfjs.getDocument({
    data: bytes,
    isEvalSupported: false,
    enableXfa: false,
    disableFontFace: false,
    useWorkerFetch: false,
  });
  const doc = await task.promise;
  try {
    const totalPages = doc.numPages as number;
    const count = Math.min(totalPages, maxPages);
    const pages: PdfPageRender[] = [];
    for (let pageNumber = 1; pageNumber <= count; pageNumber++) {
      const page = await doc.getPage(pageNumber);
      const baseVp = page.getViewport({ scale: 1 });
      const scale = Math.min(targetWidth / baseVp.width, 1.8);
      const vp = page.getViewport({ scale });
      const canvas = document.createElement("canvas");
      canvas.width = Math.max(1, Math.ceil(vp.width));
      canvas.height = Math.max(1, Math.ceil(vp.height));
      const ctx = canvas.getContext("2d");
      if (!ctx) continue;
      ctx.fillStyle = "#ffffff";
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      await page.render({ canvasContext: ctx, viewport: vp }).promise;
      pages.push({
        pageNumber,
        width: baseVp.width,
        height: baseVp.height,
        rotation: baseVp.rotation || 0,
        dataUrl: canvas.toDataURL("image/png"),
      });
    }
    return { totalPages, pages };
  } finally {
    await doc.destroy();
  }
}

export async function renderSinglePage(
  file: File,
  pageNumber = 1,
  targetWidth = 560,
): Promise<PdfPageRender | null> {
  const pdfjs = await getPdfJs();
  const bytes = await file.arrayBuffer();
  const task = pdfjs.getDocument({
    data: bytes,
    isEvalSupported: false,
    enableXfa: false,
    disableFontFace: false,
    useWorkerFetch: false,
  });
  const doc = await task.promise;
  try {
    const num = Math.max(1, Math.min(pageNumber, doc.numPages));
    const page = await doc.getPage(num);
    const baseVp = page.getViewport({ scale: 1 });
    const scale = Math.min(targetWidth / baseVp.width, 2.2);
    const vp = page.getViewport({ scale });
    const canvas = document.createElement("canvas");
    canvas.width = Math.max(1, Math.ceil(vp.width));
    canvas.height = Math.max(1, Math.ceil(vp.height));
    const ctx = canvas.getContext("2d");
    if (!ctx) return null;
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    await page.render({ canvasContext: ctx, viewport: vp }).promise;
    return {
      pageNumber: num,
      totalPages: doc.numPages as number,
      width: baseVp.width,
      height: baseVp.height,
      rotation: baseVp.rotation || 0,
      dataUrl: canvas.toDataURL("image/png"),
    };
  } finally {
    await doc.destroy();
  }
}
