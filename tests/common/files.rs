use image::{ImageBuffer, Rgb};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;
use zip::ZipWriter;
use zip::write::FileOptions;

// Re-export for PDF creation
use lopdf::dictionary;
pub use lopdf::{Dictionary, Document, Object, Stream};

/// Create a simple test PNG image using the image crate
pub fn create_test_png(width: u32, height: u32) -> Vec<u8> {
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(width, height, |x, y| {
        // Create a simple pattern
        if (x + y) % 2 == 0 {
            Rgb([255, 0, 0]) // Red
        } else {
            Rgb([0, 0, 255]) // Blue
        }
    });

    let mut buffer = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut buffer),
        image::ImageFormat::Png,
    )
    .unwrap();

    buffer
}

/// Create a simple test JPEG image using the image crate
pub fn create_test_jpeg(width: u32, height: u32) -> Vec<u8> {
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(width, height, |x, y| {
        // Create a simple pattern
        if (x + y) % 2 == 0 {
            Rgb([255, 0, 0]) // Red
        } else {
            Rgb([0, 0, 255]) // Blue
        }
    });

    let mut buffer = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut buffer),
        image::ImageFormat::Jpeg,
    )
    .unwrap();

    buffer
}

/// Create a test CBZ file with the specified number of pages
pub fn create_test_cbz(temp_dir: &TempDir, num_pages: usize, with_comic_info: bool) -> PathBuf {
    let cbz_path = temp_dir.path().join("test_comic.cbz");
    let file = File::create(&cbz_path).unwrap();
    let mut zip = ZipWriter::new(file);

    let options: FileOptions<'_, ()> =
        FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    // Add pages
    for i in 1..=num_pages {
        let page_data = create_test_png(10, 10);
        let filename = format!("page{:03}.png", i);
        zip.start_file(&filename, options).unwrap();
        zip.write_all(&page_data).unwrap();
    }

    // Add ComicInfo.xml if requested
    if with_comic_info {
        let comic_info_xml = r#"<?xml version="1.0"?>
<ComicInfo>
    <Title>Test Comic</Title>
    <Series>Test Series</Series>
    <Number>1</Number>
    <Volume>1</Volume>
    <Writer>Test Writer</Writer>
    <Publisher>Test Publisher</Publisher>
    <Year>2024</Year>
    <PageCount>3</PageCount>
</ComicInfo>"#;

        zip.start_file("ComicInfo.xml", options).unwrap();
        zip.write_all(comic_info_xml.as_bytes()).unwrap();
    }

    zip.finish().unwrap();
    cbz_path
}

/// Create a test EPUB file with the specified number of chapters and images
pub fn create_test_epub(temp_dir: &TempDir, num_chapters: usize, num_images: usize) -> PathBuf {
    let epub_path = temp_dir.path().join("test_book.epub");
    let file = File::create(&epub_path).unwrap();
    let mut zip = ZipWriter::new(file);

    let options: FileOptions<'_, ()> =
        FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    // Add mimetype file (must be first and uncompressed)
    zip.start_file("mimetype", options).unwrap();
    zip.write_all(b"application/epub+zip").unwrap();

    // Add META-INF/container.xml
    let container_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;

    zip.start_file("META-INF/container.xml", options).unwrap();
    zip.write_all(container_xml.as_bytes()).unwrap();

    // Build manifest items and spine items
    let mut manifest_items = String::new();
    let mut spine_items = String::new();

    // Add chapters to manifest and spine
    for i in 1..=num_chapters {
        manifest_items.push_str(&format!(
            r#"    <item id="chapter{}" href="chapter{}.xhtml" media-type="application/xhtml+xml"/>"#,
            i, i
        ));
        manifest_items.push('\n');

        spine_items.push_str(&format!(r#"    <itemref idref="chapter{}"/>"#, i));
        spine_items.push('\n');
    }

    // Add images to manifest
    for i in 1..=num_images {
        manifest_items.push_str(&format!(
            r#"    <item id="img{}" href="images/image{}.png" media-type="image/png"/>"#,
            i, i
        ));
        manifest_items.push('\n');
    }

    // Add content.opf
    let content_opf = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="bookid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Test EPUB Book</dc:title>
    <dc:creator>Test Author</dc:creator>
    <dc:identifier id="bookid">test-epub-123</dc:identifier>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
{}  </manifest>
  <spine>
{}  </spine>
</package>"#,
        manifest_items, spine_items
    );

    zip.start_file("OEBPS/content.opf", options).unwrap();
    zip.write_all(content_opf.as_bytes()).unwrap();

    // Add chapter files
    for i in 1..=num_chapters {
        let chapter_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
  <title>Chapter {}</title>
</head>
<body>
  <h1>Chapter {}</h1>
  <p>This is the content of chapter {}.</p>
</body>
</html>"#,
            i, i, i
        );

        zip.start_file(format!("OEBPS/chapter{}.xhtml", i), options)
            .unwrap();
        zip.write_all(chapter_content.as_bytes()).unwrap();
    }

    // Add images
    for i in 1..=num_images {
        let image_data = create_test_png(10, 10);
        zip.start_file(format!("OEBPS/images/image{}.png", i), options)
            .unwrap();
        zip.write_all(&image_data).unwrap();
    }

    zip.finish().unwrap();
    epub_path
}

/// Create a test EPUB file with rich OPF metadata for testing metadata extraction
pub fn create_test_epub_with_metadata(temp_dir: &TempDir) -> PathBuf {
    let epub_path = temp_dir.path().join("test_book_with_metadata.epub");
    let file = File::create(&epub_path).unwrap();
    let mut zip = ZipWriter::new(file);

    let options: FileOptions<'_, ()> =
        FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    // Add mimetype file (must be first and uncompressed)
    zip.start_file("mimetype", options).unwrap();
    zip.write_all(b"application/epub+zip").unwrap();

    // Add META-INF/container.xml
    let container_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;

    zip.start_file("META-INF/container.xml", options).unwrap();
    zip.write_all(container_xml.as_bytes()).unwrap();

    // Add content.opf with rich metadata
    let content_opf = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="bookid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>The Great Adventure</dc:title>
    <dc:creator opf:role="aut">Jane Doe</dc:creator>
    <dc:publisher>Acme Publishing</dc:publisher>
    <dc:date>2023-06-15</dc:date>
    <dc:language>en</dc:language>
    <dc:description>An epic tale of adventure and discovery.</dc:description>
    <dc:subject>Fiction</dc:subject>
    <dc:subject>Adventure</dc:subject>
    <dc:identifier id="bookid">urn:isbn:978-1-23456-789-0</dc:identifier>
    <dc:identifier opf:scheme="ISBN">978-1-23456-789-0</dc:identifier>
    <meta name="calibre:series" content="Adventure Chronicles"/>
    <meta name="calibre:series_index" content="2.0"/>
  </metadata>
  <manifest>
    <item id="chapter1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
    <item id="img1" href="images/image1.png" media-type="image/png"/>
  </manifest>
  <spine>
    <itemref idref="chapter1"/>
  </spine>
</package>"#;

    zip.start_file("OEBPS/content.opf", options).unwrap();
    zip.write_all(content_opf.as_bytes()).unwrap();

    // Add a chapter file
    let chapter_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
  <title>Chapter 1</title>
</head>
<body>
  <h1>Chapter 1</h1>
  <p>This is the content of chapter 1.</p>
</body>
</html>"#;

    zip.start_file("OEBPS/chapter1.xhtml", options).unwrap();
    zip.write_all(chapter_content.as_bytes()).unwrap();

    // Add an image
    let image_data = create_test_png(10, 10);
    zip.start_file("OEBPS/images/image1.png", options).unwrap();
    zip.write_all(&image_data).unwrap();

    zip.finish().unwrap();
    epub_path
}

/// Create a test PDF file with the specified number of pages and images
pub fn create_test_pdf(
    temp_dir: &TempDir,
    num_pages: usize,
    num_images_per_page: usize,
) -> PathBuf {
    let pdf_path = temp_dir.path().join("test_document.pdf");

    // Create a new PDF document
    let mut doc = Document::with_version("1.5");

    // Create catalog and pages objects
    let pages_id = doc.new_object_id();
    let mut page_ids = Vec::new();

    for page_num in 0..num_pages {
        // Create a page
        let page_id = doc.new_object_id();
        page_ids.push(page_id);

        // Create content stream for the page
        let content_id = doc.new_object_id();

        // Create simple content (text)
        let content_text = format!("BT /F1 24 Tf 100 700 Td (Page {}) Tj ET", page_num + 1);

        let content = Stream::new(dictionary! {}, content_text.as_bytes().to_vec());
        doc.objects.insert(content_id, Object::Stream(content));

        // Create XObject dictionary for images if needed
        let mut resources_dict = dictionary! {
            "Font" => dictionary! {
                "F1" => dictionary! {
                    "Type" => "Font",
                    "Subtype" => "Type1",
                    "BaseFont" => "Helvetica",
                }
            }
        };

        if num_images_per_page > 0 {
            let mut xobject_dict = Dictionary::new();

            for img_num in 0..num_images_per_page {
                // Create a simple test image (10x10 JPEG)
                // We use JPEG with DCTDecode filter because:
                // - DCTDecode means the stream contains raw JPEG data
                // - This is directly readable by image decoders without conversion
                // - FlateDecode would mean raw pixel data, not a file format
                let image_data = create_test_jpeg(10, 10);

                let image_id = doc.new_object_id();
                let image_name = format!("Im{}", img_num + 1);

                // Create image XObject with DCTDecode (JPEG)
                let image_dict = dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 10,
                    "Height" => 10,
                    "ColorSpace" => "DeviceRGB",
                    "BitsPerComponent" => 8,
                    "Filter" => "DCTDecode",
                };

                let image_stream = Stream::new(image_dict.clone(), image_data);
                doc.objects.insert(image_id, Object::Stream(image_stream));

                xobject_dict.set(image_name.as_bytes(), Object::Reference(image_id));
            }

            resources_dict.set("XObject", Object::Dictionary(xobject_dict));
        }

        // Create the page object
        let page_dict = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_dict,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        };

        doc.objects.insert(page_id, Object::Dictionary(page_dict));
    }

    // Create the Pages object
    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => page_ids.iter().map(|id| Object::Reference(*id)).collect::<Vec<_>>(),
        "Count" => page_ids.len() as u32,
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    // Create the catalog
    let catalog_id = doc.new_object_id();
    let catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    };
    doc.objects.insert(catalog_id, Object::Dictionary(catalog));

    // Set the document trailer
    doc.trailer.set("Root", Object::Reference(catalog_id));

    // Save the PDF
    doc.save(&pdf_path).unwrap();

    pdf_path
}

/// Create test CBZ files in a directory for scanning
pub fn create_test_cbz_files_in_dir(dir: &std::path::Path) {
    use std::fs;

    // Create a series directory
    let series_dir = dir.join("Test Series");
    fs::create_dir_all(&series_dir).unwrap();

    // Create temp dir for creating CBZ files
    let temp_dir = TempDir::new().unwrap();

    // Create a few test CBZ files in the series directory
    for i in 1..=3 {
        let cbz_path = create_test_cbz(&temp_dir, 5, true);
        let target_path = series_dir.join(format!("Issue {:03}.cbz", i));
        fs::copy(cbz_path, target_path).unwrap();
    }
}

/// Create a test PDF with only text content (no embedded images)
///
/// This creates a PDF that requires PDFium rendering to extract page images.
/// It cannot be processed by the fast path (embedded image extraction).
pub fn create_text_only_pdf(temp_dir: &TempDir, num_pages: usize) -> PathBuf {
    let pdf_path = temp_dir.path().join("text_only.pdf");

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_ids = Vec::new();

    for page_num in 0..num_pages {
        let page_id = doc.new_object_id();
        page_ids.push(page_id);

        // Create content stream with text only (no images)
        let content_id = doc.new_object_id();
        let content_text = format!(
            "BT /F1 24 Tf 100 700 Td (Text-only Page {}) Tj 0 -30 Td (This page has no embedded images) Tj 0 -30 Td (It requires PDFium rendering) Tj ET",
            page_num + 1
        );

        let content = Stream::new(dictionary! {}, content_text.as_bytes().to_vec());
        doc.objects.insert(content_id, Object::Stream(content));

        // Resources with only a font (no XObject/images)
        let resources_dict = dictionary! {
            "Font" => dictionary! {
                "F1" => dictionary! {
                    "Type" => "Font",
                    "Subtype" => "Type1",
                    "BaseFont" => "Helvetica",
                }
            }
        };

        let page_dict = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_dict,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        };

        doc.objects.insert(page_id, Object::Dictionary(page_dict));
    }

    // Create Pages object
    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => page_ids.iter().map(|id| Object::Reference(*id)).collect::<Vec<_>>(),
        "Count" => page_ids.len() as u32,
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    // Create catalog
    let catalog_id = doc.new_object_id();
    let catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    };
    doc.objects.insert(catalog_id, Object::Dictionary(catalog));
    doc.trailer.set("Root", Object::Reference(catalog_id));

    doc.save(&pdf_path).unwrap();
    pdf_path
}

/// Create a test PDF with vector graphics content (paths/shapes, no images)
///
/// This creates a PDF with vector drawing commands that requires PDFium rendering.
pub fn create_vector_graphics_pdf(temp_dir: &TempDir, num_pages: usize) -> PathBuf {
    let pdf_path = temp_dir.path().join("vector_graphics.pdf");

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_ids = Vec::new();

    for _page_num in 0..num_pages {
        let page_id = doc.new_object_id();
        page_ids.push(page_id);

        // Create content stream with vector graphics (paths)
        let content_id = doc.new_object_id();
        // PDF path drawing commands: rectangle, circle approximation, lines
        let content_text = r#"
            % Draw a red rectangle
            1 0 0 RG
            50 600 200 100 re S
            % Draw a blue filled rectangle
            0 0 1 rg
            300 600 150 80 re f
            % Draw some lines
            0.5 0.5 0.5 RG
            100 400 m 300 500 l 500 400 l S
            % Draw a triangle
            0 1 0 RG
            200 200 m 300 350 l 400 200 l h S
        "#;

        let content = Stream::new(dictionary! {}, content_text.as_bytes().to_vec());
        doc.objects.insert(content_id, Object::Stream(content));

        let resources_dict = dictionary! {};

        let page_dict = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_dict,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        };

        doc.objects.insert(page_id, Object::Dictionary(page_dict));
    }

    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => page_ids.iter().map(|id| Object::Reference(*id)).collect::<Vec<_>>(),
        "Count" => page_ids.len() as u32,
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    let catalog_id = doc.new_object_id();
    let catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    };
    doc.objects.insert(catalog_id, Object::Dictionary(catalog));
    doc.trailer.set("Root", Object::Reference(catalog_id));

    doc.save(&pdf_path).unwrap();
    pdf_path
}

/// Create a test PDF with mixed content (text, vector graphics, and embedded images)
pub fn create_mixed_content_pdf(
    temp_dir: &TempDir,
    num_pages: usize,
    num_images_per_page: usize,
) -> PathBuf {
    let pdf_path = temp_dir.path().join("mixed_content.pdf");

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_ids = Vec::new();

    for page_num in 0..num_pages {
        let page_id = doc.new_object_id();
        page_ids.push(page_id);

        let content_id = doc.new_object_id();
        // Mixed content: text + vector paths + image references
        let mut content_text = format!(
            "BT /F1 18 Tf 50 750 Td (Mixed Content Page {}) Tj ET\n",
            page_num + 1
        );
        content_text.push_str("% Vector graphics\n");
        content_text.push_str("0.8 0 0 RG 100 600 100 50 re S\n");

        // Add image references
        for i in 0..num_images_per_page {
            content_text.push_str(&format!(
                "q 50 0 0 50 {} 400 cm /Im{} Do Q\n",
                100 + i * 60,
                i + 1
            ));
        }

        let content = Stream::new(dictionary! {}, content_text.as_bytes().to_vec());
        doc.objects.insert(content_id, Object::Stream(content));

        let mut resources_dict = dictionary! {
            "Font" => dictionary! {
                "F1" => dictionary! {
                    "Type" => "Font",
                    "Subtype" => "Type1",
                    "BaseFont" => "Helvetica",
                }
            }
        };

        // Add images to XObject dictionary
        if num_images_per_page > 0 {
            let mut xobject_dict = Dictionary::new();

            for img_num in 0..num_images_per_page {
                let image_data = create_test_png(10, 10);
                let image_id = doc.new_object_id();
                let image_name = format!("Im{}", img_num + 1);

                let image_dict = dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 10,
                    "Height" => 10,
                    "ColorSpace" => "DeviceRGB",
                    "BitsPerComponent" => 8,
                    "Filter" => "FlateDecode",
                };

                let image_stream = Stream::new(image_dict.clone(), image_data);
                doc.objects.insert(image_id, Object::Stream(image_stream));
                xobject_dict.set(image_name.as_bytes(), Object::Reference(image_id));
            }

            resources_dict.set("XObject", Object::Dictionary(xobject_dict));
        }

        let page_dict = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_dict,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        };

        doc.objects.insert(page_id, Object::Dictionary(page_dict));
    }

    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => page_ids.iter().map(|id| Object::Reference(*id)).collect::<Vec<_>>(),
        "Count" => page_ids.len() as u32,
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    let catalog_id = doc.new_object_id();
    let catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    };
    doc.objects.insert(catalog_id, Object::Dictionary(catalog));
    doc.trailer.set("Root", Object::Reference(catalog_id));

    doc.save(&pdf_path).unwrap();
    pdf_path
}

/// Create a multi-page PDF for performance/pagination testing
pub fn create_multi_page_pdf(temp_dir: &TempDir, num_pages: usize) -> PathBuf {
    // Reuse the text-only PDF creation but with a different filename
    let pdf_path = temp_dir.path().join("multi_page.pdf");

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_ids = Vec::new();

    for page_num in 0..num_pages {
        let page_id = doc.new_object_id();
        page_ids.push(page_id);

        let content_id = doc.new_object_id();
        let content_text = format!("BT /F1 48 Tf 200 400 Td (Page {}) Tj ET", page_num + 1);

        let content = Stream::new(dictionary! {}, content_text.as_bytes().to_vec());
        doc.objects.insert(content_id, Object::Stream(content));

        let resources_dict = dictionary! {
            "Font" => dictionary! {
                "F1" => dictionary! {
                    "Type" => "Font",
                    "Subtype" => "Type1",
                    "BaseFont" => "Helvetica",
                }
            }
        };

        let page_dict = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_dict,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        };

        doc.objects.insert(page_id, Object::Dictionary(page_dict));
    }

    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => page_ids.iter().map(|id| Object::Reference(*id)).collect::<Vec<_>>(),
        "Count" => page_ids.len() as u32,
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    let catalog_id = doc.new_object_id();
    let catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    };
    doc.objects.insert(catalog_id, Object::Dictionary(catalog));
    doc.trailer.set("Root", Object::Reference(catalog_id));

    doc.save(&pdf_path).unwrap();
    pdf_path
}

/// Create a test CBZ file with rich ComicInfo.xml metadata
pub fn create_test_cbz_with_metadata(temp_dir: &TempDir, filename: &str) -> std::path::PathBuf {
    use std::fs;

    let file_path = temp_dir.path().join(filename);
    let file = fs::File::create(&file_path).unwrap();
    let mut zip = ZipWriter::new(file);

    let options: FileOptions<'_, ()> =
        FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    // Add ComicInfo.xml with rich metadata
    let comic_info_xml = r#"<?xml version="1.0"?>
<ComicInfo xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <Title>Test Comic Title</Title>
  <Series>Test Series</Series>
  <Number>1</Number>
  <Count>12</Count>
  <Volume>1</Volume>
  <Summary>This is a test comic book summary with detailed description.</Summary>
  <Year>2024</Year>
  <Month>1</Month>
  <Day>15</Day>
  <Writer>Test Writer</Writer>
  <Penciller>Test Penciller</Penciller>
  <Inker>Test Inker</Inker>
  <Colorist>Test Colorist</Colorist>
  <Letterer>Test Letterer</Letterer>
  <CoverArtist>Test Cover Artist</CoverArtist>
  <Editor>Test Editor</Editor>
  <Publisher>Test Publisher</Publisher>
  <Imprint>Test Imprint</Imprint>
  <Genre>Action, Adventure</Genre>
  <Web>https://example.com/comic</Web>
  <PageCount>3</PageCount>
  <LanguageISO>en</LanguageISO>
  <Format>Comic</Format>
  <BlackAndWhite>No</BlackAndWhite>
  <Manga>No</Manga>
</ComicInfo>"#;

    zip.start_file("ComicInfo.xml", options).unwrap();
    zip.write_all(comic_info_xml.as_bytes()).unwrap();

    // Add some test image pages (simple PNG files)
    for i in 1..=3 {
        let page_name = format!("page{:03}.png", i);
        zip.start_file(&page_name, options).unwrap();

        // Create a minimal valid PNG file
        let png_data = create_test_png(10, 10);
        zip.write_all(&png_data).unwrap();
    }

    zip.finish().unwrap();
    file_path
}
