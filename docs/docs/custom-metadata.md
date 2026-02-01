---
---

# Custom Metadata & Templating

Codex allows you to store and display custom metadata for your series beyond the standard fields (title, genre, tags, etc.). This guide covers how to use custom metadata and configure its display using Handlebars templates.

## Overview

Custom metadata is a flexible JSON object that can contain any data you want to associate with a series. It's displayed on the series detail page using a configurable Handlebars template that renders to Markdown.

Templates have access to two data sources:
- **`customMetadata`** - Your custom JSON data stored on the series
- **`metadata`** - Built-in series metadata (title, genres, publisher, ratings, etc.)

**Use cases:**
- Track reading progress and personal ratings
- Link to external databases (MyAnimeList, AniList, etc.)
- Store collection details (edition, condition, purchase info)
- Add personal notes and reviews
- Track technical information (resolution, audio, subtitles)

![Custom Metadata on Series](../screenshots/settings/server-custom-metadata.png)

## Storing Custom Metadata

Custom metadata is stored as a JSON object on each series. You can set it via the API or through the series metadata editor.

### JSON Structure

Custom metadata can be any valid JSON object:

```json
{
  "status": "In Progress",
  "rating": 8.5,
  "priority": 5,
  "startedDate": "2024-01-15",
  "notes": "Great series, highly recommended!",
  "links": [
    { "name": "MyAnimeList", "url": "https://myanimelist.net/manga/1" },
    { "name": "AniList", "url": "https://anilist.co/manga/1" }
  ],
  "tags": ["favorite", "action", "must-read"]
}
```

### API Endpoints

Update custom metadata via the API:

```bash
# Update series custom metadata
PATCH /api/v1/series/{id}
Content-Type: application/json

{
  "customMetadata": {
    "status": "Completed",
    "rating": 9
  }
}
```

## Display Templates

Custom metadata is rendered on the series detail page using a Handlebars template. The template is configured in **Settings > Server Settings > Custom Metadata Template**.

### How Templates Work

1. Your custom metadata is passed to the template as `customMetadata`
2. Built-in series metadata is passed as `metadata`
3. The template is rendered using Handlebars syntax
4. The output (Markdown) is displayed using styled components

### Default Template

The default template displays genres from built-in metadata and custom fields as a bullet list:

```handlebars
{{#if metadata.genres}}
**Genres:** {{join metadata.genres " • "}}
{{/if}}

{{#if customMetadata}}
## Additional Information

{{#each customMetadata}}
- **{{@key}}**: {{this}}
{{/each}}
{{/if}}
```

### Configuring Templates

1. Navigate to **Settings > Server Settings**
2. Find the **Custom Metadata Template** section
3. Edit the template or select a pre-built example
4. Use the live preview to test your changes
5. Save your changes

![Custom Metadata Settings](../screenshots/settings/server-custom-metadata.png)

![Custom Metadata Template Editor](../screenshots/settings/server-custom-metadata-templates.png)

## Handlebars Syntax

Templates use [Handlebars](https://handlebarsjs.com/) syntax with additional custom helpers.

### Basic Syntax

```handlebars
{{!-- Output a value --}}
{{customMetadata.title}}

{{!-- Conditional block --}}
{{#if customMetadata.rating}}
Rating: {{customMetadata.rating}}/10
{{/if}}

{{!-- Iterate over arrays --}}
{{#each customMetadata.tags}}
- {{this}}
{{/each}}

{{!-- Iterate over objects --}}
{{#each customMetadata}}
- **{{@key}}**: {{this}}
{{/each}}
```

### Available Helpers

Codex provides these custom helpers:

| Helper | Description | Usage |
|--------|-------------|-------|
| `formatDate` | Format a date string | `{{formatDate value "MMM d, yyyy"}}` |
| `ifEquals` | Check if two values are equal | `{{#ifEquals value1 value2}}...{{/ifEquals}}` |
| `ifNotEquals` | Check if two values are not equal | `{{#ifNotEquals value1 value2}}...{{/ifNotEquals}}` |
| `json` | Output JSON representation | `{{json value}}` |
| `truncate` | Truncate string to length | `{{truncate value 100 "..."}}` |
| `lowercase` | Convert to lowercase | `{{lowercase value}}` |
| `uppercase` | Convert to uppercase | `{{uppercase value}}` |
| `capitalize` | Capitalize first letter | `{{capitalize value}}` |
| `first` | Get first N items of array | `{{#first items 3}}...{{/first}}` |
| `join` | Join array with separator | `{{join array ", "}}` |
| `exists` | Check if value exists | `{{#exists value}}...{{/exists}}` |
| `length` | Get length of array/string | `{{length array}}` |
| `gt` | Greater than comparison | `{{#gt value1 value2}}...{{/gt}}` |
| `lt` | Less than comparison | `{{#lt value1 value2}}...{{/lt}}` |
| `and` | Logical AND | `{{#and cond1 cond2}}...{{/and}}` |
| `or` | Logical OR | `{{#or cond1 cond2}}...{{/or}}` |
| `lookup` | Dynamic property access | `{{lookup object key}}` |
| `default` | Provide default value | `{{default value "fallback"}}` |

### Helper Examples

**Date Formatting:**

```handlebars
{{#if customMetadata.startedDate}}
Started: {{formatDate customMetadata.startedDate "MMMM d, yyyy"}}
{{/if}}
```

Date formats use [date-fns format strings](https://date-fns.org/docs/format):
- `yyyy-MM-dd` → 2024-01-15
- `MMM d, yyyy` → Jan 15, 2024
- `MMMM d, yyyy` → January 15, 2024
- `MMMM d, yyyy 'at' h:mm a` → January 15, 2024 at 2:30 PM

**Conditional Display:**

```handlebars
{{#gt customMetadata.rating 8}}
🔥 Highly Rated!
{{else}}
{{#gt customMetadata.rating 5}}
👍 Worth Reading
{{else}}
🤔 Mixed Reviews
{{/gt}}
{{/gt}}
```

**Array Operations:**

```handlebars
{{!-- Join array items --}}
**Tags:** {{join customMetadata.tags ", "}}

{{!-- Show only first 3 items --}}
{{#first customMetadata.characters 3}}
- {{this.name}}: {{this.role}}
{{/first}}

{{!-- Show count --}}
Total: {{length customMetadata.items}} items
```

**Default Values:**

```handlebars
Status: {{default customMetadata.status "Not started"}}
Rating: {{default customMetadata.rating "—"}}/10
```

## Built-in Metadata Fields

In addition to `customMetadata`, templates have access to the series' built-in metadata via the `metadata` object. This allows you to combine your custom tracking data with standard series information.

### Available Fields

| Field | Type | Description |
|-------|------|-------------|
| `metadata.title` | string | Series title |
| `metadata.summary` | string | Series description/summary |
| `metadata.publisher` | string | Publisher name |
| `metadata.imprint` | string | Publisher imprint |
| `metadata.year` | number | Publication year |
| `metadata.status` | string | Series status (e.g., "ongoing", "completed") |
| `metadata.totalBookCount` | number | Total number of books in the series |
| `metadata.ageRating` | number | Age rating (e.g., 13, 18) |
| `metadata.language` | string | Primary language |
| `metadata.genres` | string[] | List of genres |
| `metadata.tags` | string[] | List of tags |
| `metadata.externalRatings` | array | External ratings (source, rating, votes) |
| `metadata.externalLinks` | array | External links (source, url) |
| `metadata.alternateTitles` | array | Alternate titles (title, label) |

### Metadata Examples

**Display genres and publisher:**

```handlebars
{{#if metadata.genres}}
**Genres:** {{join metadata.genres " • "}}
{{/if}}

{{#if metadata.publisher}}
**Publisher:** {{metadata.publisher}}{{#if metadata.year}} ({{metadata.year}}){{/if}}
{{/if}}
```

**Show series status with capitalization:**

```handlebars
{{#if metadata.status}}
**Status:** {{capitalize metadata.status}}
{{/if}}
```

**Display external ratings:**

```handlebars
{{#if metadata.externalRatings}}
### Community Ratings
{{#each metadata.externalRatings}}
- **{{this.source}}**: {{this.rating}}{{#if this.votes}} ({{this.votes}} votes){{/if}}
{{/each}}
{{/if}}
```

**Combine custom and built-in metadata:**

```handlebars
{{#if metadata}}
## {{metadata.title}}

{{#if metadata.summary}}
{{metadata.summary}}
{{/if}}

{{#if metadata.genres}}
**Genres:** {{join metadata.genres " • "}}
{{/if}}
{{/if}}

{{#if customMetadata}}
---
## My Progress

**Status:** {{default customMetadata.status "Not started"}}
{{#if customMetadata.currentVolume}}
**Currently on:** Volume {{customMetadata.currentVolume}}{{#if metadata.totalBookCount}} of {{metadata.totalBookCount}}{{/if}}
{{/if}}
{{/if}}
```

## Supported Markdown

The template output is rendered as Markdown with support for:

### Headings

```markdown
# Large Heading
## Section Heading
### Subsection
```

### Lists

```markdown
- Item 1
- Item 2
- Item 3
```

List items with `**label**: value` pattern are styled as key-value rows:

```markdown
- **Status**: Completed
- **Rating**: 9/10
- **Priority**: High
```

### Tables

```markdown
| Column 1 | Column 2 | Column 3 |
|----------|----------|----------|
| Value 1  | Value 2  | Value 3  |
| Value 4  | Value 5  | Value 6  |
```

### Links

```markdown
[Link Text](https://example.com)
```

External links (starting with `http`) open in a new tab.

### Code

Inline code:
```markdown
File ID: `ABC123`
```

Code blocks:
````markdown
```
Resolution: 1920x1080
Audio: Japanese 5.1
```
````

### Blockquotes

```markdown
> This is a blockquote for important notes or warnings.
```

### Text Formatting

```markdown
**Bold text**
*Italic text*
~~Strikethrough text~~
```

### Horizontal Rules

```markdown
---
```

## Example Templates

Codex includes several pre-built templates to get you started:

### Simple List

Basic key-value display with optional genres from built-in metadata:

```handlebars
{{#if metadata.genres}}
**Genres:** {{join metadata.genres " • "}}
{{/if}}

{{#if customMetadata}}
## Additional Information

{{#each customMetadata}}
- **{{@key}}**: {{this}}
{{/each}}
{{/if}}
```

### Reading List

Track reading progress and ratings:

```handlebars
{{#if customMetadata}}
## Reading Info

{{#if customMetadata.status}}
**Status:** {{customMetadata.status}}
{{/if}}

{{#if customMetadata.rating}}
**My Rating:** {{customMetadata.rating}}/10
{{/if}}

{{#if customMetadata.startedDate}}
**Started:** {{formatDate customMetadata.startedDate "MMM d, yyyy"}}
{{/if}}

{{#if customMetadata.completedDate}}
**Completed:** {{formatDate customMetadata.completedDate "MMM d, yyyy"}}
{{/if}}

{{#if customMetadata.notes}}
### Notes
{{customMetadata.notes}}
{{/if}}
{{/if}}
```

### External Links

Link to external databases:

```handlebars
{{#if customMetadata}}
{{#if customMetadata.links}}
## External Links

{{#each customMetadata.links}}
- [{{this.name}}]({{this.url}})
{{/each}}
{{/if}}

{{#if customMetadata.ids}}
### Database IDs
{{#each customMetadata.ids}}
- **{{@key}}**: `{{this}}`
{{/each}}
{{/if}}
{{/if}}
```

### Collection Info

Track physical collection details:

```handlebars
{{#if customMetadata}}
## Collection Details

{{#if customMetadata.format}}
**Format:** {{customMetadata.format}}
{{/if}}

{{#if customMetadata.edition}}
**Edition:** {{customMetadata.edition}}
{{/if}}

{{#if customMetadata.condition}}
**Condition:** {{customMetadata.condition}}
{{/if}}

{{#if customMetadata.purchaseDate}}
**Purchased:** {{formatDate customMetadata.purchaseDate "MMM d, yyyy"}}
{{/if}}

{{#if customMetadata.location}}
**Location:** {{customMetadata.location}}
{{/if}}
{{/if}}
```

### With Tables

Display data in tables:

```handlebars
{{#if customMetadata}}
{{#and customMetadata.status customMetadata.rating}}
## Reading Status

| Status | Rating | Priority |
|--------|--------|----------|
| {{default customMetadata.status "Not started"}} | {{default customMetadata.rating "—"}}/10 | {{default customMetadata.priority "—"}} |

{{/and}}
{{/if}}
```

### Series Info (Built-in Metadata)

Display only built-in series metadata:

```handlebars
{{#if metadata}}
## Series Info

{{#if metadata.publisher}}
**Publisher:** {{metadata.publisher}}{{#if metadata.imprint}} ({{metadata.imprint}}){{/if}}
{{/if}}

{{#if metadata.year}}
**Year:** {{metadata.year}}
{{/if}}

{{#if metadata.status}}
**Status:** {{capitalize metadata.status}}
{{/if}}

{{#if metadata.genres}}
### Genres
{{join metadata.genres " • "}}
{{/if}}

{{#if metadata.externalRatings}}
### Ratings
{{#each metadata.externalRatings}}
- **{{this.source}}**: {{this.rating}}{{#if this.votes}} ({{this.votes}} votes){{/if}}
{{/each}}
{{/if}}

{{#if metadata.externalLinks}}
### Links
{{#each metadata.externalLinks}}
- [{{this.source}}]({{this.url}})
{{/each}}
{{/if}}
{{/if}}
```

### Complete Overview (Combined)

Combine custom tracking data with built-in series metadata:

```handlebars
{{#if metadata}}
## {{metadata.title}}

{{#if metadata.summary}}
{{metadata.summary}}
{{/if}}

{{#and metadata.publisher metadata.year}}
*Published by {{metadata.publisher}} in {{metadata.year}}*
{{/and}}
{{/if}}

{{#if customMetadata}}
---

## My Progress

{{#if customMetadata.status}}
**Status:** {{customMetadata.status}}
{{/if}}

{{#if customMetadata.rating}}
**My Rating:** {{customMetadata.rating}}/10
{{/if}}

{{#if customMetadata.currentVolume}}
**Currently on:** Volume {{customMetadata.currentVolume}}{{#if metadata.totalBookCount}} of {{metadata.totalBookCount}}{{/if}}
{{/if}}

{{#if customMetadata.notes}}
### Notes
{{customMetadata.notes}}
{{/if}}
{{/if}}

{{#if metadata}}
{{#if metadata.genres}}
---
**Genres:** {{join metadata.genres " • "}}
{{/if}}
{{/if}}
```

## Best Practices

### Template Design

1. **Use conditional blocks**: Always wrap sections in `{{#if}}` to handle missing data gracefully
2. **Provide defaults**: Use `{{default value "fallback"}}` for optional fields
3. **Keep it readable**: Use headings and sections to organize information
4. **Test with sample data**: Use the live preview in settings to test your template

### Data Structure

1. **Use consistent keys**: Stick to a naming convention (camelCase recommended)
2. **Use ISO dates**: Store dates as ISO strings (e.g., `2024-01-15`) for reliable formatting
3. **Group related data**: Use nested objects for related information
4. **Use arrays for lists**: Store multiple items as arrays for easy iteration

### Performance

1. **Keep templates simple**: Complex nested loops may slow rendering
2. **Limit output size**: Templates have a 100KB output limit
3. **Use efficient helpers**: Prefer `exists` over `if` for null checks

## Troubleshooting

### Template Errors

If your template shows an error:

1. Check for unclosed blocks (`{{#if}}` needs `{{/if}}`)
2. Verify helper names are spelled correctly
3. Check for mismatched quotes in strings
4. Use the live preview to see error messages

### Data Not Displaying

If custom metadata isn't showing:

1. Verify the series has custom metadata set
2. Check that your template handles the data structure correctly
3. Ensure conditional blocks match your data (e.g., `customMetadata.field` vs `customMetadata.nested.field`)

### Styling Issues

If the output doesn't look right:

1. Verify your Markdown syntax is correct
2. Check that tables have proper header rows
3. Ensure code blocks use triple backticks

## Next Steps

- [Filtering & Search](./filtering) - Filter series by custom metadata
- [API Documentation](./api) - Full API reference
- [Libraries](./libraries) - Library management
