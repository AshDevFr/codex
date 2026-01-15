# Custom Metadata Test Fixtures

This directory contains JSON fixtures for testing the custom metadata feature.

## Files

- `reading_progress.json` - Example metadata for tracking reading progress
- `external_links.json` - Example metadata with external database links
- `collection_info.json` - Example metadata for physical collection tracking
- `nested_structure.json` - Complex nested JSON structure for testing deep parsing

## Usage

These fixtures can be loaded in tests to verify:
1. JSON parsing/serialization works correctly
2. API accepts and returns custom_metadata properly
3. Size validation (64KB limit) works as expected
4. Template rendering handles various data structures
