---
---

# Preprocessing Examples

This page provides ready-to-use configurations for common preprocessing scenarios. Copy these examples and adapt them to your needs.

## Manga Library

Common patterns for manga directories that often include metadata in folder names.

### Complete Manga Setup

**Title Preprocessing Rules:**

```json
[
  {
    "pattern": "\\s*\\(Digital\\)$",
    "replacement": "",
    "description": "Remove (Digital) suffix",
    "enabled": true
  },
  {
    "pattern": "\\s*\\[English\\]$",
    "replacement": "",
    "description": "Remove [English] suffix",
    "enabled": true
  },
  {
    "pattern": "^\\[[^\\]]+\\]\\s*",
    "replacement": "",
    "description": "Remove [Scanlator] prefix",
    "enabled": true
  },
  {
    "pattern": "\\s*\\(\\d{4}\\)$",
    "replacement": "",
    "description": "Remove year in parentheses",
    "enabled": true
  },
  {
    "pattern": "\\s+v\\d+.*$",
    "replacement": "",
    "description": "Remove volume information",
    "enabled": true
  },
  {
    "pattern": "\\s{2,}",
    "replacement": " ",
    "description": "Normalize multiple spaces",
    "enabled": true
  }
]
```

**Example Transformations:**

| Before | After |
|--------|-------|
| `One Piece (Digital)` | `One Piece` |
| `[Viz] Naruto [English]` | `Naruto` |
| `Attack on Titan (2013)` | `Attack on Titan` |
| `My Hero Academia v01-v30` | `My Hero Academia` |

### Auto-Match Conditions (Library Level)

Only auto-match series that:
- Have at least 1 book
- Don't have metadata locked

```json
{
  "mode": "all",
  "rules": [
    {
      "field": "book_count",
      "operator": "gte",
      "value": 1
    },
    {
      "field": "metadata.title_lock",
      "operator": "not_equals",
      "value": true
    }
  ]
}
```

## Comics Library

Common patterns for western comics with publisher and volume information.

### Complete Comics Setup

**Title Preprocessing Rules:**

```json
[
  {
    "pattern": "\\s*\\(\\d{4}\\)$",
    "replacement": "",
    "description": "Remove year suffix",
    "enabled": true
  },
  {
    "pattern": "\\s*\\(DC Comics\\)$",
    "replacement": "",
    "description": "Remove DC publisher suffix",
    "enabled": true
  },
  {
    "pattern": "\\s*\\(Marvel\\)$",
    "replacement": "",
    "description": "Remove Marvel publisher suffix",
    "enabled": true
  },
  {
    "pattern": "\\s*[Vv]ol\\.?\\s*\\d+.*$",
    "replacement": "",
    "description": "Remove volume suffix",
    "enabled": true
  },
  {
    "pattern": "\\s*-\\s*The Complete.*$",
    "replacement": "",
    "description": "Remove 'The Complete...' suffix",
    "enabled": true
  },
  {
    "pattern": "\\s*\\(TPB\\)$",
    "replacement": "",
    "description": "Remove TPB indicator",
    "enabled": true
  }
]
```

**Example Transformations:**

| Before | After |
|--------|-------|
| `Batman (2016)` | `Batman` |
| `Spider-Man (Marvel)` | `Spider-Man` |
| `X-Men Vol. 1 - Revolution` | `X-Men` |
| `Saga - The Complete Collection (TPB)` | `Saga` |

## Ebooks Library

Common patterns for ebooks with author names and series information.

### Complete Ebooks Setup

**Title Preprocessing Rules:**

```json
[
  {
    "pattern": "^[^-]+\\s*-\\s*",
    "replacement": "",
    "description": "Remove 'Author - ' prefix",
    "enabled": true
  },
  {
    "pattern": "\\s*\\(.*?Series.*?\\)$",
    "replacement": "",
    "description": "Remove series indicator",
    "enabled": true
  },
  {
    "pattern": "\\s*#\\d+$",
    "replacement": "",
    "description": "Remove book number",
    "enabled": true
  },
  {
    "pattern": "\\s*\\[\\d+\\]$",
    "replacement": "",
    "description": "Remove book number in brackets",
    "enabled": true
  }
]
```

**Example Transformations:**

| Before | After |
|--------|-------|
| `Brandon Sanderson - Mistborn` | `Mistborn` |
| `The Way of Kings (Stormlight Archive Series)` | `The Way of Kings` |
| `Harry Potter #1` | `Harry Potter` |
| `Dune [1]` | `Dune` |

## Plugin-Specific Configurations

### MangaBaka Plugin

Configuration for the MangaBaka metadata provider.

**Search Query Template:**

```handlebars
{{metadata.title}}
```

**Search Preprocessing Rules:**

```json
[
  {
    "pattern": "^The\\s+",
    "replacement": "",
    "description": "Remove leading 'The'",
    "enabled": true
  },
  {
    "pattern": "\\s*:\\s*",
    "replacement": " ",
    "description": "Replace colons with spaces",
    "enabled": true
  }
]
```

**Auto-Match Conditions:**

Only match if not already matched by this plugin:

```json
{
  "mode": "all",
  "rules": [
    {
      "field": "external_ids.plugin:mangabaka",
      "operator": "is_null"
    }
  ]
}
```

### ComicVine Plugin (Hypothetical)

Example configuration for a ComicVine-style plugin.

**Search Query Template:**

Include year for better matching:

```handlebars
{{metadata.title}}{{#if metadata.year}} ({{metadata.year}}){{/if}}
```

**Search Preprocessing Rules:**

```json
[
  {
    "pattern": "\\s*'s\\b",
    "replacement": "",
    "description": "Remove possessive apostrophe-s",
    "enabled": true
  },
  {
    "pattern": "&",
    "replacement": "and",
    "description": "Replace ampersand with 'and'",
    "enabled": true
  }
]
```

## Advanced Scenarios

### Multi-Language Library

For libraries with content in multiple languages:

**Auto-Match Conditions:**

Only match Japanese and English content:

```json
{
  "mode": "all",
  "rules": [
    {
      "field": "metadata.language",
      "operator": "in",
      "value": ["en", "ja", "ja-JP"]
    }
  ]
}
```

### Conservative Matching

For users who prefer manual matching for most content:

**Auto-Match Conditions:**

Only auto-match when confident (many books, no existing match):

```json
{
  "mode": "all",
  "rules": [
    {
      "field": "book_count",
      "operator": "gte",
      "value": 5
    },
    {
      "field": "external_ids.count",
      "operator": "equals",
      "value": 0
    }
  ]
}
```

### Publisher-Specific Matching

Different conditions for different publishers:

**Example: Only match Dark Horse comics automatically**

```json
{
  "mode": "all",
  "rules": [
    {
      "field": "metadata.publisher",
      "operator": "contains",
      "value": "Dark Horse"
    }
  ]
}
```

### Skip Completed Series

Don't re-match series that are already marked as completed:

```json
{
  "mode": "any",
  "rules": [
    {
      "field": "metadata.status",
      "operator": "not_equals",
      "value": "completed"
    },
    {
      "field": "metadata.status",
      "operator": "is_null"
    }
  ]
}
```

## Regex Reference

### Common Regex Patterns

| Pattern | Matches | Example |
|---------|---------|---------|
| `\\s*` | Zero or more whitespace | Handles varying spacing |
| `$` | End of string | `foo$` matches "foo" at end |
| `^` | Start of string | `^foo` matches "foo" at start |
| `\\d+` | One or more digits | Matches "2024" |
| `\\(.*?\\)` | Text in parentheses | Matches "(Digital)" |
| `\\[.*?\\]` | Text in brackets | Matches "[English]" |
| `[^\\]]+` | Any chars except `]` | Non-greedy bracket content |

### Capture Groups

Use `$1`, `$2`, etc. to reference captured groups:

```json
{
  "pattern": "^(.*?)\\s*\\(\\d{4}\\)$",
  "replacement": "$1",
  "description": "Keep text before year"
}
```

**Before:** `Batman (2016)` → **After:** `Batman`

### Case-Insensitive Matching

Rust regex supports case-insensitive mode:

```json
{
  "pattern": "(?i)\\s*\\(digital\\)$",
  "replacement": "",
  "description": "Remove (Digital) case-insensitive"
}
```

This matches `(Digital)`, `(DIGITAL)`, `(digital)`, etc.

## Testing Your Rules

### Web UI Testing

1. Go to Library Settings → Preprocessing
2. Enter a test title in the preview field
3. See the transformed result in real-time
4. Adjust rules until the output is correct

### API Testing

Use the API to test preprocessing without scanning:

```bash
curl -X POST "http://localhost:8080/api/v1/admin/test-preprocessing" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "title": "One Piece (Digital)",
    "rules": [
      {"pattern": "\\s*\\(Digital\\)$", "replacement": ""}
    ]
  }'
```

## Troubleshooting

### Rule Not Working

1. **Check order**: Rules are applied sequentially
2. **Escape backslashes**: JSON requires `\\` for `\`
3. **Test pattern**: Use a regex tester first
4. **Check enabled**: Ensure `enabled` is `true` or omitted

### Unexpected Results

1. **Check previous rules**: Earlier rules may have modified the string
2. **Check anchors**: Use `^` and `$` for start/end matching
3. **Check greedy vs lazy**: Use `.*?` for non-greedy matching

### Performance Issues

1. **Limit complex patterns**: Avoid excessive backtracking
2. **Order by frequency**: Put most common patterns first
3. **Disable unused rules**: Set `enabled: false` instead of deleting

## Next Steps

- [Preprocessing Rules Guide](../preprocessing-rules.md) - Full reference
- [Libraries & Scanning](../libraries.md) - Library setup
- [Plugin Overview](/dev/plugins/overview.md) - Plugin system
