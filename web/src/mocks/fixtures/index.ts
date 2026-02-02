/**
 * Static mock fixture files for frontend testing.
 * These are imported as raw URLs via Vite's ?url suffix.
 */

import coverSvgUrl from "./cover.svg?url";
import pageSvgUrl from "./page.svg?url";
import sampleCbzUrl from "./sample.cbz?url";
import sampleEpubUrl from "./sample.epub?url";
import samplePdfUrl from "./sample.pdf?url";

export const fixtures = {
  cbz: sampleCbzUrl,
  epub: sampleEpubUrl,
  pdf: samplePdfUrl,
  cover: coverSvgUrl,
  page: pageSvgUrl,
};

export { sampleCbzUrl, sampleEpubUrl, samplePdfUrl, coverSvgUrl, pageSvgUrl };
