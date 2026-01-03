# Test Fixtures

This directory contains test files for integration tests.

## CBR Test Files

Due to UnRAR license restrictions (extraction only, no creation), CBR test files must be created manually.

### Source Files Location

The `cbr_source/` directory contains the source files needed to create CBR test archives:
- `test_comic/` - Contains 3 test PNG images (page001.png, page002.png, page003.png)
- `test_comic_with_info/` - Contains 3 test PNG images + ComicInfo.xml

### Creating the CBR Files

**Prerequisites:** Install WinRAR or another RAR archiver that supports creating RAR archives

#### 1. Create test_comic.cbr
1. Navigate to `cbr_source/test_comic/` directory
2. Select all PNG files (page001.png, page002.png, page003.png)
3. Create a RAR archive using your RAR archiver
4. Rename the archive to `test_comic.cbr`
5. Move `test_comic.cbr` to `tests/fixtures/` (parent directory)

#### 2. Create test_comic_with_info.cbr
1. Navigate to `cbr_source/test_comic_with_info/` directory
2. Select all files (page001.png, page002.png, page003.png, ComicInfo.xml)
3. Create a RAR archive using your RAR archiver
4. Rename the archive to `test_comic_with_info.cbr`
5. Move `test_comic_with_info.cbr` to `tests/fixtures/` (parent directory)

The ComicInfo.xml file already contains:
```xml
<?xml version="1.0"?>
<ComicInfo>
    <Title>Test Comic</Title>
    <Series>Test Series</Series>
    <Number>1</Number>
    <Volume>1</Volume>
    <Writer>Test Writer</Writer>
    <Publisher>Test Publisher</Publisher>
    <Year>2024</Year>
    <PageCount>3</PageCount>
</ComicInfo>
```

### Running CBR Integration Tests

Once test files are created:

```bash
# Run all tests including ignored ones
cargo test --features rar -- --ignored --test-threads=1

# Run only CBR tests
cargo test --features rar cbr_parser -- --ignored --test-threads=1
```

## Note on Licensing

The UnRAR library used for CBR support has a proprietary license that permits:
- ✅ Extraction and decompression of RAR archives
- ❌ Creation of RAR archives

This is why we cannot programmatically create test CBR files in our test suite.
