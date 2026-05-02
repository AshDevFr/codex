use crate::parsers::ComicInfo;
use quick_xml::de::from_str;
use serde::{Deserialize, Serialize};

/// ComicInfo.xml structure for deserialization
#[derive(Debug, Deserialize)]
#[serde(rename = "ComicInfo")]
struct ComicInfoXml {
    #[serde(rename = "Title")]
    title: Option<String>,
    #[serde(rename = "Series")]
    series: Option<String>,
    #[serde(rename = "Number")]
    number: Option<String>,
    #[serde(rename = "Count")]
    count: Option<i32>,
    #[serde(rename = "Volume")]
    volume: Option<i32>,
    #[serde(rename = "Summary")]
    summary: Option<String>,
    #[serde(rename = "Year")]
    year: Option<i32>,
    #[serde(rename = "Month")]
    month: Option<i32>,
    #[serde(rename = "Day")]
    day: Option<i32>,
    #[serde(rename = "Writer")]
    writer: Option<String>,
    #[serde(rename = "Penciller")]
    penciller: Option<String>,
    #[serde(rename = "Inker")]
    inker: Option<String>,
    #[serde(rename = "Colorist")]
    colorist: Option<String>,
    #[serde(rename = "Letterer")]
    letterer: Option<String>,
    #[serde(rename = "CoverArtist")]
    cover_artist: Option<String>,
    #[serde(rename = "Editor")]
    editor: Option<String>,
    #[serde(rename = "Publisher")]
    publisher: Option<String>,
    #[serde(rename = "Imprint")]
    imprint: Option<String>,
    #[serde(rename = "Genre")]
    genre: Option<String>,
    #[serde(rename = "Web")]
    web: Option<String>,
    #[serde(rename = "PageCount")]
    page_count: Option<i32>,
    #[serde(rename = "LanguageISO")]
    language_iso: Option<String>,
    #[serde(rename = "Format")]
    format: Option<String>,
    #[serde(rename = "BlackAndWhite")]
    black_and_white: Option<String>,
    #[serde(rename = "Manga")]
    manga: Option<String>,
}

/// A single author entry for the structured `authors_json` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuthorEntry {
    name: String,
    role: String,
}

/// Build a JSON string from individual ComicInfo role fields.
///
/// For each non-None field, splits the value on `,`, trims whitespace from each
/// name, and creates an `AuthorEntry` with the field name as the role.
/// Returns `None` if no authors are found across any field.
fn build_authors_json(
    writer: &Option<String>,
    penciller: &Option<String>,
    inker: &Option<String>,
    colorist: &Option<String>,
    letterer: &Option<String>,
    cover_artist: &Option<String>,
    editor: &Option<String>,
) -> Option<String> {
    let fields: &[(&Option<String>, &str)] = &[
        (writer, "writer"),
        (penciller, "penciller"),
        (inker, "inker"),
        (colorist, "colorist"),
        (letterer, "letterer"),
        (cover_artist, "cover_artist"),
        (editor, "editor"),
    ];

    let mut entries = Vec::new();

    for (field, role) in fields {
        if let Some(value) = field {
            for name in value.split(',') {
                let trimmed = name.trim();
                if !trimmed.is_empty() {
                    entries.push(AuthorEntry {
                        name: trimmed.to_string(),
                        role: role.to_string(),
                    });
                }
            }
        }
    }

    if entries.is_empty() {
        None
    } else {
        // serde_json::to_string should not fail for this simple structure
        serde_json::to_string(&entries).ok()
    }
}

/// Parse ComicInfo.xml content
pub fn parse_comic_info(xml_content: &str) -> Result<ComicInfo, quick_xml::DeError> {
    let xml_info: ComicInfoXml = from_str(xml_content)?;

    let authors_json = build_authors_json(
        &xml_info.writer,
        &xml_info.penciller,
        &xml_info.inker,
        &xml_info.colorist,
        &xml_info.letterer,
        &xml_info.cover_artist,
        &xml_info.editor,
    );

    // Phase 12 of metadata-count-split: derive a structured `chapter` from
    // `<Number>`. ComicInfo's `<Number>` field is overloaded — issue, chapter,
    // or part depending on the producer. v1: read it as a chapter; users whose
    // files use it for issues can lock `chapter` after manual fix.
    let chapter = xml_info
        .number
        .as_deref()
        .and_then(|n| n.trim().parse::<f32>().ok());

    Ok(ComicInfo {
        title: xml_info.title,
        series: xml_info.series,
        number: xml_info.number,
        count: xml_info.count,
        volume: xml_info.volume,
        chapter,
        summary: xml_info.summary,
        year: xml_info.year,
        month: xml_info.month,
        day: xml_info.day,
        writer: xml_info.writer,
        penciller: xml_info.penciller,
        inker: xml_info.inker,
        colorist: xml_info.colorist,
        letterer: xml_info.letterer,
        cover_artist: xml_info.cover_artist,
        editor: xml_info.editor,
        authors_json,
        publisher: xml_info.publisher,
        imprint: xml_info.imprint,
        genre: xml_info.genre,
        web: xml_info.web,
        page_count: xml_info.page_count,
        language_iso: xml_info.language_iso,
        format: xml_info.format,
        black_and_white: xml_info.black_and_white,
        manga: xml_info.manga,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_comic_info_full() {
        let xml = r#"<?xml version="1.0"?>
<ComicInfo>
    <Title>Amazing Comic</Title>
    <Series>Amazing Series</Series>
    <Number>42</Number>
    <Count>100</Count>
    <Volume>2</Volume>
    <Summary>An amazing comic book story</Summary>
    <Year>2024</Year>
    <Month>6</Month>
    <Day>15</Day>
    <Writer>John Doe</Writer>
    <Penciller>Jane Smith</Penciller>
    <Inker>Bob Johnson</Inker>
    <Colorist>Alice Brown</Colorist>
    <Letterer>Charlie Wilson</Letterer>
    <CoverArtist>Diana Prince</CoverArtist>
    <Editor>Eve Davis</Editor>
    <Publisher>Great Comics</Publisher>
    <Imprint>GC Imprint</Imprint>
    <Genre>Superhero</Genre>
    <Web>https://example.com</Web>
    <PageCount>24</PageCount>
    <LanguageISO>en</LanguageISO>
    <Format>Standard</Format>
    <BlackAndWhite>No</BlackAndWhite>
    <Manga>No</Manga>
</ComicInfo>"#;

        let result = parse_comic_info(xml).unwrap();

        assert_eq!(result.title, Some("Amazing Comic".to_string()));
        assert_eq!(result.series, Some("Amazing Series".to_string()));
        assert_eq!(result.number, Some("42".to_string()));
        assert_eq!(result.count, Some(100));
        assert_eq!(result.volume, Some(2));
        assert_eq!(
            result.summary,
            Some("An amazing comic book story".to_string())
        );
        assert_eq!(result.year, Some(2024));
        assert_eq!(result.month, Some(6));
        assert_eq!(result.day, Some(15));
        assert_eq!(result.writer, Some("John Doe".to_string()));
        assert_eq!(result.penciller, Some("Jane Smith".to_string()));
        assert_eq!(result.inker, Some("Bob Johnson".to_string()));
        assert_eq!(result.colorist, Some("Alice Brown".to_string()));
        assert_eq!(result.letterer, Some("Charlie Wilson".to_string()));
        assert_eq!(result.cover_artist, Some("Diana Prince".to_string()));
        assert_eq!(result.editor, Some("Eve Davis".to_string()));
        assert_eq!(result.publisher, Some("Great Comics".to_string()));
        assert_eq!(result.imprint, Some("GC Imprint".to_string()));
        assert_eq!(result.genre, Some("Superhero".to_string()));
        assert_eq!(result.web, Some("https://example.com".to_string()));
        assert_eq!(result.page_count, Some(24));
        assert_eq!(result.language_iso, Some("en".to_string()));
        assert_eq!(result.format, Some("Standard".to_string()));
        assert_eq!(result.black_and_white, Some("No".to_string()));
        assert_eq!(result.manga, Some("No".to_string()));
    }

    #[test]
    fn test_parse_comic_info_minimal() {
        let xml = r#"<?xml version="1.0"?>
<ComicInfo>
    <Title>Minimal Comic</Title>
</ComicInfo>"#;

        let result = parse_comic_info(xml).unwrap();

        assert_eq!(result.title, Some("Minimal Comic".to_string()));
        assert_eq!(result.series, None);
        assert_eq!(result.writer, None);
        assert_eq!(result.publisher, None);
    }

    #[test]
    fn test_parse_comic_info_partial() {
        let xml = r#"<?xml version="1.0"?>
<ComicInfo>
    <Title>Test Comic</Title>
    <Series>Test Series</Series>
    <Number>1</Number>
    <Writer>Test Writer</Writer>
    <Publisher>Test Publisher</Publisher>
    <Year>2023</Year>
</ComicInfo>"#;

        let result = parse_comic_info(xml).unwrap();

        assert_eq!(result.title, Some("Test Comic".to_string()));
        assert_eq!(result.series, Some("Test Series".to_string()));
        assert_eq!(result.number, Some("1".to_string()));
        assert_eq!(result.writer, Some("Test Writer".to_string()));
        assert_eq!(result.publisher, Some("Test Publisher".to_string()));
        assert_eq!(result.year, Some(2023));

        // Fields not present should be None
        assert_eq!(result.penciller, None);
        assert_eq!(result.inker, None);
        assert_eq!(result.month, None);
    }

    #[test]
    fn test_parse_comic_info_empty_fields() {
        let xml = r#"<?xml version="1.0"?>
<ComicInfo>
    <Title></Title>
    <Series>Valid Series</Series>
</ComicInfo>"#;

        let result = parse_comic_info(xml).unwrap();

        assert_eq!(result.title, Some("".to_string()));
        assert_eq!(result.series, Some("Valid Series".to_string()));
    }

    #[test]
    fn test_parse_comic_info_manga() {
        let xml = r#"<?xml version="1.0"?>
<ComicInfo>
    <Title>Manga Title</Title>
    <Manga>YesAndRightToLeft</Manga>
    <LanguageISO>ja</LanguageISO>
</ComicInfo>"#;

        let result = parse_comic_info(xml).unwrap();

        assert_eq!(result.title, Some("Manga Title".to_string()));
        assert_eq!(result.manga, Some("YesAndRightToLeft".to_string()));
        assert_eq!(result.language_iso, Some("ja".to_string()));
    }

    #[test]
    fn test_parse_comic_info_with_special_characters() {
        let xml = r#"<?xml version="1.0"?>
<ComicInfo>
    <Title>Comic &amp; Story</Title>
    <Summary>A story with "quotes" and &lt;tags&gt;</Summary>
</ComicInfo>"#;

        let result = parse_comic_info(xml).unwrap();

        assert_eq!(result.title, Some("Comic & Story".to_string()));
        assert_eq!(
            result.summary,
            Some("A story with \"quotes\" and <tags>".to_string())
        );
    }

    #[test]
    fn test_parse_comic_info_invalid_xml() {
        let xml = "This is not valid XML";
        let result = parse_comic_info(xml);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_comic_info_malformed_xml() {
        let xml = r#"<?xml version="1.0"?>
<ComicInfo>
    <Title>Unclosed tag
</ComicInfo>"#;

        let result = parse_comic_info(xml);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_comic_info_wrong_root_element() {
        // XML parser with serde is lenient with element names
        // It will successfully parse even with wrong root element
        let xml = r#"<?xml version="1.0"?>
<WrongElement>
    <Title>Test</Title>
</WrongElement>"#;

        let result = parse_comic_info(xml);
        // The parser is lenient and will succeed, returning parsed fields
        assert!(result.is_ok());
        if let Ok(info) = result {
            assert_eq!(info.title, Some("Test".to_string()));
        }
    }

    #[test]
    fn test_parse_comic_info_derives_chapter_from_number() {
        // Phase 12 of metadata-count-split: ComicInfo `<Number>` is the chapter
        // axis on the parsed struct. Integer parses cleanly; fractional preserved.
        let xml = r#"<?xml version="1.0"?>
<ComicInfo>
    <Number>42</Number>
</ComicInfo>"#;
        let result = parse_comic_info(xml).unwrap();
        assert_eq!(result.number.as_deref(), Some("42"));
        assert_eq!(result.chapter, Some(42.0));

        // Fractional chapter (e.g. side stories at 47.5).
        let xml_frac = r#"<?xml version="1.0"?>
<ComicInfo>
    <Number>47.5</Number>
</ComicInfo>"#;
        let result_frac = parse_comic_info(xml_frac).unwrap();
        assert_eq!(result_frac.chapter, Some(47.5));

        // No `<Number>` at all -> chapter stays None.
        let xml_none = r#"<?xml version="1.0"?>
<ComicInfo>
    <Title>X</Title>
</ComicInfo>"#;
        let result_none = parse_comic_info(xml_none).unwrap();
        assert_eq!(result_none.chapter, None);

        // Non-numeric `<Number>` (rare but possible) -> chapter None, raw stays.
        let xml_bad = r#"<?xml version="1.0"?>
<ComicInfo>
    <Number>part-1</Number>
</ComicInfo>"#;
        let result_bad = parse_comic_info(xml_bad).unwrap();
        assert_eq!(result_bad.number.as_deref(), Some("part-1"));
        assert_eq!(result_bad.chapter, None);
    }

    #[test]
    fn test_parse_comic_info_numeric_fields() {
        let xml = r#"<?xml version="1.0"?>
<ComicInfo>
    <Number>5.5</Number>
    <Count>50</Count>
    <Volume>3</Volume>
    <Year>2024</Year>
    <Month>12</Month>
    <Day>25</Day>
    <PageCount>32</PageCount>
</ComicInfo>"#;

        let result = parse_comic_info(xml).unwrap();

        assert_eq!(result.number, Some("5.5".to_string()));
        assert_eq!(result.count, Some(50));
        assert_eq!(result.volume, Some(3));
        assert_eq!(result.year, Some(2024));
        assert_eq!(result.month, Some(12));
        assert_eq!(result.day, Some(25));
        assert_eq!(result.page_count, Some(32));
    }

    #[test]
    fn test_parse_comic_info_whitespace() {
        let xml = r#"<?xml version="1.0"?>
<ComicInfo>
    <Title>  Title with spaces  </Title>
    <Writer>
        Writer Name
    </Writer>
</ComicInfo>"#;

        let result = parse_comic_info(xml).unwrap();

        // XML parsing typically preserves whitespace
        assert!(result.title.is_some());
        assert!(result.writer.is_some());
    }
}
