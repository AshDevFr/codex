#!/usr/bin/env node
/**
 * Generate static mock fixture files (CBZ, EPUB, PDF) for frontend testing.
 * Run with: node scripts/generate-fixtures.mjs
 */

import JSZip from "jszip";
import { writeFileSync, mkdirSync } from "fs";
import { dirname, join } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const FIXTURES_DIR = join(__dirname, "../src/mocks/fixtures");

const LOREM_IPSUM = `Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.

Curabitur pretium tincidunt lacus. Nulla gravida orci a odio. Nullam varius, turpis et commodo pharetra, est eros bibendum elit, nec luctus magna felis sollicitudin mauris. Integer in mauris eu nibh euismod gravida. Duis ac tellus et risus vulputate vehicula. Donec lobortis risus a elit.`;

// Ensure fixtures directory exists
mkdirSync(FIXTURES_DIR, { recursive: true });

/**
 * Generate SVG page content
 */
function generateSvgPage(pageNumber, title) {
  const colors = ["#2c3e50", "#34495e", "#1a252f", "#2d3436", "#0d1117"];
  const bgColor = colors[pageNumber % colors.length];

  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="800" height="1200" viewBox="0 0 800 1200">
  <rect width="100%" height="100%" fill="${bgColor}"/>
  <text x="400" y="100" text-anchor="middle" fill="#ffffff" font-family="Arial, sans-serif" font-size="32" font-weight="bold">${title}</text>
  <text x="400" y="150" text-anchor="middle" fill="#cccccc" font-family="Arial, sans-serif" font-size="24">Page ${pageNumber}</text>
  <text x="50" y="220" fill="#aaaaaa" font-family="Georgia, serif" font-size="14">
    <tspan x="50" dy="0">${LOREM_IPSUM.substring(0, 80)}</tspan>
    <tspan x="50" dy="20">${LOREM_IPSUM.substring(80, 160)}</tspan>
    <tspan x="50" dy="20">${LOREM_IPSUM.substring(160, 240)}</tspan>
    <tspan x="50" dy="20">${LOREM_IPSUM.substring(240, 320)}</tspan>
    <tspan x="50" dy="20">${LOREM_IPSUM.substring(320, 400)}</tspan>
  </text>
  <text x="400" y="1150" text-anchor="middle" fill="#666666" font-family="Arial, sans-serif" font-size="18">${pageNumber}</text>
</svg>`;
}

/**
 * Generate CBZ file (ZIP with comic pages)
 */
async function generateCbz() {
  const zip = new JSZip();
  const title = "Sample Comic";
  const pageCount = 20;

  // ComicInfo.xml
  zip.file(
    "ComicInfo.xml",
    `<?xml version="1.0" encoding="UTF-8"?>
<ComicInfo>
  <Title>${title}</Title>
  <Series>Mock Series</Series>
  <Number>1</Number>
  <Volume>1</Volume>
  <Writer>Lorem Ipsum Author</Writer>
  <Publisher>Mock Publisher</Publisher>
  <Year>2024</Year>
  <PageCount>${pageCount}</PageCount>
  <Summary>${LOREM_IPSUM.substring(0, 200)}</Summary>
</ComicInfo>`
  );

  // Add pages
  for (let i = 1; i <= pageCount; i++) {
    zip.file(
      `page${String(i).padStart(3, "0")}.svg`,
      generateSvgPage(i, title)
    );
  }

  const buffer = await zip.generateAsync({
    type: "nodebuffer",
    compression: "DEFLATE",
  });
  writeFileSync(join(FIXTURES_DIR, "sample.cbz"), buffer);
  console.log("✓ Generated sample.cbz");
}

/**
 * Generate EPUB file
 */
async function generateEpub() {
  const zip = new JSZip();
  const title = "Sample Ebook";
  const chapterCount = 10;

  // mimetype (must be first, uncompressed)
  zip.file("mimetype", "application/epub+zip", { compression: "STORE" });

  // META-INF/container.xml
  zip.file(
    "META-INF/container.xml",
    `<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>`
  );

  // content.opf
  const chapterIds = Array.from(
    { length: chapterCount },
    (_, i) => `chapter${i + 1}`
  );
  const manifestItems = chapterIds
    .map(
      (id) =>
        `    <item id="${id}" href="${id}.xhtml" media-type="application/xhtml+xml"/>`
    )
    .join("\n");
  const spineItems = chapterIds
    .map((id) => `    <itemref idref="${id}"/>`)
    .join("\n");

  zip.file(
    "OEBPS/content.opf",
    `<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="bookid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="bookid">urn:uuid:mock-epub-12345</dc:identifier>
    <dc:title>${title}</dc:title>
    <dc:creator>Lorem Ipsum Author</dc:creator>
    <dc:language>en</dc:language>
    <dc:publisher>Mock Publisher</dc:publisher>
    <meta property="dcterms:modified">2024-01-01T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
${manifestItems}
  </manifest>
  <spine>
${spineItems}
  </spine>
</package>`
  );

  // nav.xhtml
  const navItems = chapterIds
    .map((id, i) => `      <li><a href="${id}.xhtml">Chapter ${i + 1}</a></li>`)
    .join("\n");

  zip.file(
    "OEBPS/nav.xhtml",
    `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>Table of Contents</title></head>
<body>
  <nav epub:type="toc">
    <h1>Table of Contents</h1>
    <ol>
${navItems}
    </ol>
  </nav>
</body>
</html>`
  );

  // Chapters
  for (let i = 0; i < chapterCount; i++) {
    const content = Array(3).fill(`<p>${LOREM_IPSUM}</p>`).join("\n      ");
    zip.file(
      `OEBPS/${chapterIds[i]}.xhtml`,
      `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
  <title>Chapter ${i + 1}</title>
  <style>body { font-family: Georgia, serif; line-height: 1.6; margin: 2em; } h1 { color: #2c3e50; } p { text-align: justify; }</style>
</head>
<body>
  <h1>Chapter ${i + 1}</h1>
  <section>
      ${content}
  </section>
</body>
</html>`
    );
  }

  const buffer = await zip.generateAsync({
    type: "nodebuffer",
    compression: "DEFLATE",
  });
  writeFileSync(join(FIXTURES_DIR, "sample.epub"), buffer);
  console.log("✓ Generated sample.epub");
}

/**
 * Generate minimal PDF file
 */
function generatePdf() {
  const title = "Sample PDF";
  const pageCount = 20;

  // Wrap text for PDF
  function wrapText(text, maxLen = 70) {
    const words = text.split(" ");
    const lines = [];
    let line = "";
    for (const word of words) {
      if (line.length + word.length > maxLen) {
        lines.push(line.trim());
        line = word + " ";
      } else {
        line += word + " ";
      }
    }
    if (line.trim()) lines.push(line.trim());
    return lines;
  }

  // Escape PDF string
  function escPdf(s) {
    return s.replace(/\\/g, "\\\\").replace(/\(/g, "\\(").replace(/\)/g, "\\)");
  }

  const objects = [];
  let objNum = 0;
  const addObj = (content) => {
    objNum++;
    objects.push(`${objNum} 0 obj\n${content}\nendobj\n`);
    return objNum;
  };

  // Catalog
  const catalogRef = addObj("<< /Type /Catalog /Pages 2 0 R >>");

  // Pages placeholder
  const pagesIdx = objects.length;
  addObj("PLACEHOLDER");

  // Font
  const fontRef = addObj(
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"
  );

  // Create pages
  const pageRefs = [];
  for (let p = 1; p <= pageCount; p++) {
    const lines = wrapText(LOREM_IPSUM);
    let stream = "BT\n/F1 24 Tf\n50 750 Td\n";
    stream += `(${escPdf(title)} - Page ${p}) Tj\n0 -40 Td\n/F1 12 Tf\n`;
    for (const line of lines.slice(0, 20)) {
      stream += `(${escPdf(line)}) Tj\n0 -16 Td\n`;
    }
    stream += "ET";

    const contentRef = addObj(
      `<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`
    );
    const pageRef = addObj(
      `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents ${contentRef} 0 R /Resources << /Font << /F1 ${fontRef} 0 R >> >> >>`
    );
    pageRefs.push(pageRef);
  }

  // Update pages object
  objects[pagesIdx] = `2 0 obj\n<< /Type /Pages /Kids [${pageRefs
    .map((r) => `${r} 0 R`)
    .join(" ")}] /Count ${pageCount} >>\nendobj\n`;

  // Build PDF
  let pdf = "%PDF-1.4\n%\xE2\xE3\xCF\xD3\n";
  const xrefOffsets = [0];
  for (const obj of objects) {
    xrefOffsets.push(pdf.length);
    pdf += obj;
  }

  const xrefOffset = pdf.length;
  pdf += `xref\n0 ${objNum + 1}\n0000000000 65535 f \n`;
  for (let i = 1; i <= objNum; i++) {
    pdf += `${String(xrefOffsets[i]).padStart(10, "0")} 00000 n \n`;
  }
  pdf += `trailer\n<< /Size ${
    objNum + 1
  } /Root ${catalogRef} 0 R >>\nstartxref\n${xrefOffset}\n%%EOF`;

  writeFileSync(join(FIXTURES_DIR, "sample.pdf"), pdf);
  console.log("✓ Generated sample.pdf");
}

/**
 * Generate cover SVG
 */
function generateCover() {
  const svg = `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="300" height="450" viewBox="0 0 300 450">
  <defs>
    <linearGradient id="bg" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" style="stop-color:#2c3e50"/>
      <stop offset="100%" style="stop-color:#1a252f"/>
    </linearGradient>
  </defs>
  <rect width="100%" height="100%" fill="url(#bg)"/>
  <text x="150" y="200" text-anchor="middle" fill="#ffffff" font-family="Arial, sans-serif" font-size="20" font-weight="bold">Sample Cover</text>
  <text x="150" y="240" text-anchor="middle" fill="#888888" font-family="Arial, sans-serif" font-size="14">Mock Book</text>
</svg>`;

  writeFileSync(join(FIXTURES_DIR, "cover.svg"), svg);
  console.log("✓ Generated cover.svg");
}

/**
 * Generate page SVG
 */
function generatePage() {
  writeFileSync(
    join(FIXTURES_DIR, "page.svg"),
    generateSvgPage(1, "Sample Page")
  );
  console.log("✓ Generated page.svg");
}

// Run all generators
console.log("Generating mock fixtures...\n");

await generateCbz();
await generateEpub();
generatePdf();
generateCover();
generatePage();

console.log("\n✓ All fixtures generated in src/mocks/fixtures/");
